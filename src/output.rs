use crate::git::GitMeta;
use crate::scan::{Entry, ScanResult};
use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Jsonl,
}

pub fn format_output(
    format: OutputFormat,
    meta: Option<&GitMeta>,
    results: &ScanResult,
) -> Result<String, serde_json::Error> {
    match format {
        OutputFormat::Json => format_json(meta, results),
        OutputFormat::Jsonl => format_jsonl(meta, results),
    }
}

fn format_json(meta: Option<&GitMeta>, results: &ScanResult) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        meta: &'a GitMeta,
        results: &'a ScanResult,
    }

    match meta {
        Some(meta) => serde_json::to_string_pretty(&JsonReport { meta, results }),
        None => serde_json::to_string_pretty(results),
    }
}

fn format_jsonl(meta: Option<&GitMeta>, results: &ScanResult) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct JsonlMeta<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        meta: &'a GitMeta,
    }

    #[derive(Serialize)]
    struct JsonlMatch<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        requirement_id: &'a str,
        entry: &'a Entry,
    }

    let mut lines = Vec::new();

    if let Some(meta) = meta {
        lines.push(serde_json::to_string(&JsonlMeta { kind: "meta", meta })?);
    }

    for (requirement_id, entries) in results {
        for entry in entries {
            lines.push(serde_json::to_string(&JsonlMatch {
                kind: "match",
                requirement_id,
                entry,
            })?);
        }
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn one_result() -> ScanResult {
        let mut results: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        results.insert(
            "REQ-1".to_string(),
            vec![Entry {
                file: PathBuf::from("src/lib.rs"),
                line: 1,
                comment_text: "// REQ-1: validate input".to_string(),
                above: None,
                below: None,
                inline: None,
                scope: Vec::new(),
            }],
        );
        results
    }

    #[test]
    fn json_without_meta_is_plain_results() {
        let results = one_result();
        let out = format_output(OutputFormat::Json, None, &results).unwrap();
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(value.get("REQ-1").is_some());
        assert!(value.get("meta").is_none());
        assert!(value.get("results").is_none());
    }

    #[test]
    fn json_with_meta_wraps_results() {
        let results = one_result();
        let meta = GitMeta {
            repo_root: PathBuf::from("/repo"),
            head_sha: "a".repeat(40),
            head_ref: Some("main".to_string()),
            is_dirty: false,
        };

        let out = format_output(OutputFormat::Json, Some(&meta), &results).unwrap();
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(value.get("meta").is_some());
        assert!(value.get("results").is_some());
        assert!(value.get("REQ-1").is_none());
    }

    #[test]
    fn jsonl_emits_meta_then_matches() {
        let results = one_result();
        let meta = GitMeta {
            repo_root: PathBuf::from("/repo"),
            head_sha: "a".repeat(40),
            head_ref: None,
            is_dirty: true,
        };

        let out = format_output(OutputFormat::Jsonl, Some(&meta), &results).unwrap();
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(lines.len(), 2);

        let meta_line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(meta_line["type"], "meta");
        assert!(meta_line["meta"].is_object());

        let match_line: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(match_line["type"], "match");
        assert_eq!(match_line["requirement_id"], "REQ-1");
        assert!(match_line["entry"].is_object());
    }
}

