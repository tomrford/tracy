use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

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
}
