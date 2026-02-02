use crate::output::OutputFormat;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub root: Option<PathBuf>,
    pub format: Option<OutputFormat>,
    pub output: Option<PathBuf>,
    pub quiet: Option<bool>,
    pub fail_on_empty: Option<bool>,
    pub include_git_meta: Option<bool>,
    pub include_blame: Option<bool>,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub filter: FilterConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScanConfig {
    pub slug: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct FilterConfig {
    pub include_vendored: Option<bool>,
    pub include_generated: Option<bool>,
    pub include_submodules: Option<bool>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = fs::read_to_string(path).map_err(|e| ConfigError::Read {
        path: path.to_path_buf(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        source: e,
    })
}

pub fn find_config(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = dir.join("tracy.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_config_walks_up() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        let nested = repo.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(repo.join("tracy.toml"), "quiet = true\n").unwrap();

        let found = find_config(&nested).unwrap();
        assert_eq!(found, repo.join("tracy.toml"));
    }

    #[test]
    fn parses_config_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tracy.toml");
        fs::write(
            &path,
            r#"
format = "jsonl"
quiet = true
[scan]
slug = ["REQ"]
[filter]
include = ["src/**"]
"#,
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.format, Some(OutputFormat::Jsonl));
        assert_eq!(config.quiet, Some(true));
        assert_eq!(config.scan.slug.as_deref(), Some(&["REQ".to_string()][..]));
        assert_eq!(
            config.filter.include.as_deref(),
            Some(&["src/**".to_string()][..])
        );
    }
}
