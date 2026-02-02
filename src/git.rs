use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

use crate::scan::ScanResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitMeta {
    pub repo_root: PathBuf,
    pub head_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_ref: Option<String>,
    pub is_dirty: bool,
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    Run(#[from] std::io::Error),

    #[error("git command failed ({cmd}): {stderr}")]
    CommandFailed { cmd: String, stderr: String },

    #[error("git output was not valid utf-8: {0}")]
    OutputUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlameInfo {
    pub commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_mail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

pub fn collect_git_meta(scan_root: &Path) -> Result<GitMeta, GitError> {
    let repo_root = PathBuf::from(git(scan_root, &["rev-parse", "--show-toplevel"])?);

    let head_sha = git(scan_root, &["rev-parse", "HEAD"])?;

    let head_ref = match git(scan_root, &["rev-parse", "--abbrev-ref", "HEAD"])? {
        s if s == "HEAD" => None,
        s if s.is_empty() => None,
        s => Some(s),
    };

    let is_dirty = !git(scan_root, &["status", "--porcelain"])?.is_empty();

    Ok(GitMeta {
        repo_root,
        head_sha,
        head_ref,
        is_dirty,
    })
}

pub fn add_blame(scan_root: &Path, results: &mut ScanResult) -> Result<(), GitError> {
    let _ = git(scan_root, &["rev-parse", "--is-inside-work-tree"])?;

    let mut by_file: BTreeMap<PathBuf, BTreeSet<usize>> = BTreeMap::new();
    for entries in results.values() {
        for entry in entries {
            by_file.entry(entry.file.clone()).or_default().insert(entry.line);
        }
    }

    let mut blame_by_file: BTreeMap<PathBuf, BTreeMap<usize, BlameInfo>> = BTreeMap::new();
    for (file, lines) in &by_file {
        let Some(start) = lines.iter().next().copied() else {
            continue;
        };
        let Some(end) = lines.iter().next_back().copied() else {
            continue;
        };

        match blame_range(scan_root, file, start, end) {
            Ok(map) => {
                blame_by_file.insert(file.clone(), map);
            }
            Err(_) => continue,
        }
    }

    for entries in results.values_mut() {
        for entry in entries {
            if let Some(map) = blame_by_file.get(&entry.file) {
                entry.blame = map.get(&entry.line).cloned();
            }
        }
    }

    Ok(())
}

fn git(scan_root: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(scan_root)
        .args(args)
        .output()?;

    if !output.status.success() {
        return Err(GitError::CommandFailed {
            cmd: format!("git {}", args.join(" ")),
            stderr: String::from_utf8(output.stderr)?.trim().to_string(),
        });
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn blame_range(
    scan_root: &Path,
    file: &Path,
    start_line: usize,
    end_line: usize,
) -> Result<BTreeMap<usize, BlameInfo>, GitError> {
    let range = format!("{start_line},{end_line}");
    let file = file.to_string_lossy().to_string();

    let args = vec![
        "blame".to_string(),
        "--line-porcelain".to_string(),
        "-L".to_string(),
        range,
        "--".to_string(),
        file,
    ];
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git(scan_root, &args_ref)?;

    Ok(parse_blame_porcelain(&output))
}

fn parse_blame_porcelain(output: &str) -> BTreeMap<usize, BlameInfo> {
    let mut result = BTreeMap::new();
    let mut iter = output.lines();

    while let Some(header) = iter.next() {
        if header.trim().is_empty() {
            continue;
        }

        let mut parts = header.split_whitespace();
        let Some(commit) = parts.next() else {
            continue;
        };
        let _orig_line = parts.next().and_then(|s| s.parse::<usize>().ok());
        let final_line = match parts.next().and_then(|s| s.parse::<usize>().ok()) {
            Some(n) => n,
            None => continue,
        };
        let group_len = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);

        let mut author = None;
        let mut author_mail = None;
        let mut author_time = None;
        let mut summary = None;

        while let Some(line) = iter.next() {
            if line.starts_with('\t') {
                break;
            }
            let Some((key, value)) = line.split_once(' ') else {
                continue;
            };
            match key {
                "author" => author = Some(value.to_string()),
                "author-mail" => {
                    author_mail = Some(
                        value
                            .trim()
                            .trim_start_matches('<')
                            .trim_end_matches('>')
                            .to_string(),
                    )
                }
                "author-time" => author_time = value.parse::<i64>().ok(),
                "summary" => summary = Some(value.to_string()),
                _ => {}
            }
        }

        let info = BlameInfo {
            commit: commit.to_string(),
            author,
            author_mail,
            author_time,
            summary,
        };

        for i in 0..group_len {
            result.insert(final_line + i, info.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn git_in(dir: &Path, args: &[&str]) -> Result<(), GitError> {
        let output = Command::new("git").arg("-C").arg(dir).args(args).output()?;
        if !output.status.success() {
            return Err(GitError::CommandFailed {
                cmd: format!("git {}", args.join(" ")),
                stderr: String::from_utf8(output.stderr)?.trim().to_string(),
            });
        }
        Ok(())
    }

    #[test]
    fn collects_head_and_dirty_state() {
        let dir = TempDir::new().unwrap();
        git_in(dir.path(), &["init", "-b", "main"]).unwrap();
        git_in(dir.path(), &["config", "user.email", "test@example.com"]).unwrap();
        git_in(dir.path(), &["config", "user.name", "Test"]).unwrap();

        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "hello\n").unwrap();
        git_in(dir.path(), &["add", "file.txt"]).unwrap();
        git_in(dir.path(), &["commit", "-m", "init"]).unwrap();

        let meta = collect_git_meta(dir.path()).unwrap();
        let expected_root = fs::canonicalize(dir.path()).unwrap();
        let actual_root = fs::canonicalize(&meta.repo_root).unwrap();
        assert_eq!(actual_root, expected_root);
        assert_eq!(meta.head_sha.len(), 40);
        assert_eq!(meta.head_ref.as_deref(), Some("main"));
        assert!(!meta.is_dirty);

        fs::write(&file_path, "changed\n").unwrap();
        let meta = collect_git_meta(dir.path()).unwrap();
        assert!(meta.is_dirty);
    }

    #[test]
    fn adds_blame_per_line() {
        let dir = TempDir::new().unwrap();
        git_in(dir.path(), &["init", "-b", "main"]).unwrap();
        git_in(dir.path(), &["config", "user.email", "test@example.com"]).unwrap();
        git_in(dir.path(), &["config", "user.name", "Test"]).unwrap();

        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "REQ-1 first\nREQ-2 second\n").unwrap();
        git_in(dir.path(), &["add", "file.txt"]).unwrap();
        git_in(
            dir.path(),
            &[
                "commit",
                "-m",
                "first",
                "--date",
                "2000-01-01T00:00:00Z",
            ],
        )
        .unwrap();
        let first_sha = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();

        fs::write(&file_path, "REQ-1 first\nREQ-2 second changed\n").unwrap();
        git_in(dir.path(), &["add", "file.txt"]).unwrap();
        git_in(
            dir.path(),
            &[
                "commit",
                "-m",
                "second",
                "--date",
                "2000-01-01T00:00:01Z",
            ],
        )
        .unwrap();
        let second_sha = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();

        let mut results: ScanResult = BTreeMap::new();
        results.insert(
            "REQ-1".to_string(),
            vec![crate::scan::Entry {
                file: PathBuf::from("file.txt"),
                line: 1,
                comment_text: "REQ-1 first".to_string(),
                above: None,
                below: None,
                inline: None,
                scope: Vec::new(),
                blame: None,
            }],
        );
        results.insert(
            "REQ-2".to_string(),
            vec![crate::scan::Entry {
                file: PathBuf::from("file.txt"),
                line: 2,
                comment_text: "REQ-2 second changed".to_string(),
                above: None,
                below: None,
                inline: None,
                scope: Vec::new(),
                blame: None,
            }],
        );

        add_blame(dir.path(), &mut results).unwrap();

        let blame_1 = results["REQ-1"][0].blame.as_ref().unwrap();
        assert_eq!(blame_1.commit, first_sha);
        assert_eq!(blame_1.author.as_deref(), Some("Test"));

        let blame_2 = results["REQ-2"][0].blame.as_ref().unwrap();
        assert_eq!(blame_2.commit, second_sha);
        assert_eq!(blame_2.author.as_deref(), Some("Test"));
    }
}
