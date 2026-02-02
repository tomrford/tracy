use crate::git::GitMeta;
use crate::scan::{Entry, ScanResult};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Jsonl,
    Csv,
    Sarif,
}

pub fn format_output(
    format: OutputFormat,
    meta: Option<&GitMeta>,
    results: &ScanResult,
) -> Result<String, serde_json::Error> {
    match format {
        OutputFormat::Json => format_json(meta, results),
        OutputFormat::Jsonl => format_jsonl(meta, results),
        OutputFormat::Csv => Ok(format_csv(meta, results)),
        OutputFormat::Sarif => format_sarif(meta, results),
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

fn format_csv(meta: Option<&GitMeta>, results: &ScanResult) -> String {
    let mut lines = Vec::new();

    let mut header = vec![
        "requirement_id",
        "file",
        "line",
        "comment_text",
        "above",
        "below",
        "inline",
        "scope",
        "blame",
    ];
    if meta.is_some() {
        header.extend(["repo_root", "head_sha", "head_ref", "is_dirty"]);
    }
    lines.push(header.join(","));

    for (requirement_id, entries) in results {
        for entry in entries {
            let above = entry
                .above
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default())
                .unwrap_or_default();
            let below = entry
                .below
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default())
                .unwrap_or_default();
            let inline = entry
                .inline
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default())
                .unwrap_or_default();
            let scope = if entry.scope.is_empty() {
                String::new()
            } else {
                serde_json::to_string(&entry.scope).unwrap_or_default()
            };
            let blame = entry
                .blame
                .as_ref()
                .map(|b| serde_json::to_string(b).unwrap_or_default())
                .unwrap_or_default();

            let mut row = vec![
                requirement_id.to_string(),
                entry.file.display().to_string(),
                entry.line.to_string(),
                entry.comment_text.clone(),
                above,
                below,
                inline,
                scope,
                blame,
            ];

            if let Some(meta) = meta {
                row.push(meta.repo_root.display().to_string());
                row.push(meta.head_sha.clone());
                row.push(meta.head_ref.clone().unwrap_or_default());
                row.push(meta.is_dirty.to_string());
            }

            lines.push(row.iter().map(|v| csv_escape(v)).collect::<Vec<_>>().join(","));
        }
    }

    lines.join("\n")
}

fn csv_escape(value: &str) -> String {
    let needs_quotes = value
        .chars()
        .any(|c| matches!(c, ',' | '"' | '\n' | '\r'));
    if !needs_quotes {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn format_sarif(meta: Option<&GitMeta>, results: &ScanResult) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct SarifLog<'a> {
        #[serde(rename = "$schema")]
        schema: &'static str,
        version: &'static str,
        runs: Vec<SarifRun<'a>>,
    }

    #[derive(Serialize)]
    struct SarifRun<'a> {
        tool: SarifTool,
        results: Vec<SarifResult<'a>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        properties: Option<&'a GitMeta>,
    }

    #[derive(Serialize)]
    struct SarifTool {
        driver: SarifDriver,
    }

    #[derive(Serialize)]
    struct SarifDriver {
        name: &'static str,
        version: &'static str,
        rules: Vec<SarifRule>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SarifRule {
        id: &'static str,
        name: &'static str,
        short_description: SarifMessage,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SarifResult<'a> {
        rule_id: &'static str,
        level: &'static str,
        message: SarifMessage,
        locations: Vec<SarifLocation>,
        properties: SarifResultProperties<'a>,
    }

    #[derive(Serialize)]
    struct SarifResultProperties<'a> {
        requirement_id: &'a str,
        comment_text: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        blame: Option<&'a crate::git::BlameInfo>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SarifLocation {
        physical_location: SarifPhysicalLocation,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SarifPhysicalLocation {
        artifact_location: SarifArtifactLocation,
        region: SarifRegion,
    }

    #[derive(Serialize)]
    struct SarifArtifactLocation {
        uri: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SarifRegion {
        start_line: usize,
    }

    #[derive(Serialize)]
    struct SarifMessage {
        text: String,
    }

    let mut sarif_results = Vec::new();
    for (requirement_id, entries) in results {
        for entry in entries {
            sarif_results.push(SarifResult {
                rule_id: "traceability.requirement_ref",
                level: "note",
                message: SarifMessage {
                    text: format!("Requirement reference: {requirement_id}"),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: entry.file.to_string_lossy().replace('\\', "/"),
                        },
                        region: SarifRegion {
                            start_line: entry.line,
                        },
                    },
                }],
                properties: SarifResultProperties {
                    requirement_id,
                    comment_text: &entry.comment_text,
                    blame: entry.blame.as_ref(),
                },
            });
        }
    }

    let sarif = SarifLog {
        schema: "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0.json",
        version: "2.1.0",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "tracy",
                    version: env!("CARGO_PKG_VERSION"),
                    rules: vec![SarifRule {
                        id: "traceability.requirement_ref",
                        name: "Requirement reference",
                        short_description: SarifMessage {
                            text: "Requirement references found in comments".to_string(),
                        },
                    }],
                },
            },
            results: sarif_results,
            properties: meta,
        }],
    };

    serde_json::to_string_pretty(&sarif)
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
                blame: None,
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

    #[test]
    fn csv_escapes_commas_and_quotes() {
        let mut results = one_result();
        results.get_mut("REQ-1").unwrap()[0].comment_text = "// REQ-1, \"quoted\"".to_string();

        let out = format_output(OutputFormat::Csv, None, &results).unwrap();
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(
            lines[0],
            "requirement_id,file,line,comment_text,above,below,inline,scope,blame"
        );
        assert!(
            lines[1].contains("\"// REQ-1, \"\"quoted\"\"\""),
            "expected csv escaping, got: {}",
            lines[1]
        );
    }

    #[test]
    fn sarif_has_basic_structure() {
        let results = one_result();
        let out = format_output(OutputFormat::Sarif, None, &results).unwrap();
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["version"], "2.1.0");

        let run = &value["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "tracy");

        let result = &run["results"][0];
        assert_eq!(result["ruleId"], "traceability.requirement_ref");
        assert_eq!(result["properties"]["requirement_id"], "REQ-1");
        assert_eq!(
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/lib.rs"
        );
        assert_eq!(
            result["locations"][0]["physicalLocation"]["region"]["startLine"],
            1
        );
    }
}
