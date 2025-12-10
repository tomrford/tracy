pub mod args;
mod error;

pub use args::FilterArgs;
pub use error::FilterError;

use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

struct Excludes {
    vendored: Vec<glob::Pattern>,
    generated: Vec<glob::Pattern>,
}

pub fn collect_files(root: &Path, args: &FilterArgs) -> Result<Vec<PathBuf>, FilterError> {
    let excludes = parse_gitattributes(root);
    let mut files = Vec::new();

    for entry in WalkBuilder::new(root)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .require_git(!args.include_submodules)
        .build()
    {
        let entry = entry?;

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();

        if is_excluded(path, root, &excludes, args) {
            continue;
        }

        files.push(path.to_path_buf());
    }

    Ok(files)
}

fn parse_gitattributes(root: &Path) -> Excludes {
    let attr_path = root.join(".gitattributes");
    let Ok(content) = fs::read_to_string(&attr_path) else {
        return Excludes {
            vendored: Vec::new(),
            generated: Vec::new(),
        };
    };

    let mut vendored = Vec::new();
    let mut generated = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some(pattern_str) = line.split_whitespace().next() else {
            continue;
        };

        let Ok(pattern) = glob::Pattern::new(pattern_str) else {
            continue;
        };

        if line.contains("linguist-vendored") {
            vendored.push(pattern.clone());
        }
        if line.contains("linguist-generated") {
            generated.push(pattern);
        }
    }

    Excludes {
        vendored,
        generated,
    }
}

fn is_excluded(path: &Path, root: &Path, excludes: &Excludes, args: &FilterArgs) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative_str = relative.to_string_lossy();

    if !args.include_vendored && excludes.vendored.iter().any(|p| p.matches(&relative_str)) {
        return true;
    }

    if !args.include_generated && excludes.generated.iter().any(|p| p.matches(&relative_str)) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn parse_gitattributes_finds_vendored_patterns() {
        let excludes = parse_gitattributes(&fixture_root());
        assert_eq!(excludes.vendored.len(), 2);
        assert!(excludes.vendored[0].matches("vendor/foo/bar.rs"));
        assert!(excludes.vendored[1].matches("third_party/lib.c"));
    }

    #[test]
    fn parse_gitattributes_finds_generated_patterns() {
        let excludes = parse_gitattributes(&fixture_root());
        assert_eq!(excludes.generated.len(), 2);
        assert!(excludes.generated[0].matches("foo.generated.rs"));
        assert!(excludes.generated[1].matches("src/gen/types.rs"));
    }

    #[test]
    fn is_excluded_respects_vendored_flag() {
        let excludes = parse_gitattributes(&fixture_root());
        let root = Path::new("/repo");
        let path = Path::new("/repo/vendor/dep/lib.rs");

        let args_exclude = FilterArgs::default();
        assert!(is_excluded(path, root, &excludes, &args_exclude));

        let args_include = FilterArgs {
            include_vendored: true,
            ..Default::default()
        };
        assert!(!is_excluded(path, root, &excludes, &args_include));
    }

    #[test]
    fn is_excluded_respects_generated_flag() {
        let excludes = parse_gitattributes(&fixture_root());
        let root = Path::new("/repo");
        let path = Path::new("/repo/types.generated.rs");

        let args_exclude = FilterArgs::default();
        assert!(is_excluded(path, root, &excludes, &args_exclude));

        let args_include = FilterArgs {
            include_generated: true,
            ..Default::default()
        };
        assert!(!is_excluded(path, root, &excludes, &args_include));
    }
}
