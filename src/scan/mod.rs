pub mod args;
mod error;

pub use args::ScanArgs;
pub use error::ScanError;

use ast_grep_core::language::Language;
use ast_grep_language::{LanguageExt, SupportLang};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct Entry {
    pub file: PathBuf,
    pub line: usize,
}

pub type ScanResult = BTreeMap<String, Vec<Entry>>;

pub fn scan_files(
    root: &Path,
    paths: &[PathBuf],
    args: &ScanArgs,
) -> Result<ScanResult, ScanError> {
    let pattern = Regex::new(&format!(r"({})-(\d+)", regex::escape(&args.slug)))?;
    let mut results: ScanResult = BTreeMap::new();

    for path in paths {
        scan_file(root, path, &pattern, &mut results)?;
    }

    Ok(results)
}

fn scan_file(
    root: &Path,
    path: &Path,
    pattern: &Regex,
    results: &mut ScanResult,
) -> Result<(), ScanError> {
    let Some(lang) = SupportLang::from_path(path) else {
        return Ok(());
    };

    let source = fs::read_to_string(path).map_err(|e| ScanError::ReadFile {
        path: path.to_path_buf(),
        source: e,
    })?;

    let relative = path.strip_prefix(root).unwrap_or(path);
    let ast_root = lang.ast_grep(&source);
    let mut seen: HashSet<(String, usize)> = HashSet::new();

    for node in ast_root.root().dfs() {
        let kind = node.kind();
        if !is_comment(&kind) {
            continue;
        }

        let line = node.start_pos().line() + 1;
        let text = node.text();

        for cap in pattern.captures_iter(&text) {
            let slug = format!(
                "{}-{}",
                cap.get(1).map(|m| m.as_str()).unwrap_or_default(),
                cap.get(2).map(|m| m.as_str()).unwrap_or_default()
            );

            if !seen.insert((slug.clone(), line)) {
                continue;
            }

            results.entry(slug).or_default().push(Entry {
                file: relative.to_path_buf(),
                line,
            });
        }
    }

    Ok(())
}

fn is_comment(kind: &str) -> bool {
    kind.contains("comment")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(ext: &str, content: &str) -> NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    fn scan_args(slug: &str) -> ScanArgs {
        ScanArgs {
            slug: slug.to_string(),
        }
    }

    #[test]
    fn finds_requirement_in_doc_comment() {
        let file = create_temp_file(".rs", "/// REQ-123: validate input\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-123"));
        assert_eq!(results["REQ-123"].len(), 1);
        assert_eq!(results["REQ-123"][0].line, 1);
    }

    #[test]
    fn finds_multiple_ids_in_same_doc_comment() {
        let file = create_temp_file(".rs", "/// REQ-1 and REQ-2 are both needed\nfn foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
    }

    #[test]
    fn groups_multiple_entries_by_slug() {
        let file = create_temp_file(".rs", "/// REQ-1 first\n/// REQ-1 second\nfn foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results["REQ-1"].len(), 2);
    }

    #[test]
    fn uses_custom_slug_pattern() {
        let file = create_temp_file(".rs", "/// LIN-456: linear issue\nfn bar() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("LIN")).unwrap();

        assert!(results.contains_key("LIN-456"));
    }

    #[test]
    fn skips_unsupported_file_types() {
        let file = create_temp_file(".xyz", "/// REQ-999: won't be found");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn finds_doc_block_comments() {
        let file = create_temp_file(".rs", "/** REQ-789: block comment */\nfn baz() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-789"));
    }

    #[test]
    fn finds_regular_comments() {
        let file = create_temp_file(".rs", "// REQ-1: regular comment\n/* REQ-2 */\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
    }

    #[test]
    fn stores_relative_file_path() {
        let file = create_temp_file(".rs", "/// REQ-1\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results["REQ-1"][0].file, file.path().file_name().unwrap());
    }

    // ==================== Language-specific tests ====================

    #[test]
    fn python_docstring_not_parsed_as_comment() {
        // Python docstrings are string nodes, not comment nodes in tree-sitter
        // They won't be found by our comment-based scanner
        let file = create_temp_file(".py", r#""""REQ-100: python docstring"""
def foo(): pass"#);
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn python_finds_hash_comments() {
        let file = create_temp_file(".py", "# REQ-999: python comment\ndef x(): pass");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-999"));
    }

    #[test]
    fn javascript_jsdoc_comment() {
        let file = create_temp_file(".js", "/** REQ-200: jsdoc */\nfunction foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-200"));
    }

    #[test]
    fn javascript_finds_regular_comments() {
        let file = create_temp_file(".js", "// REQ-201\n/* REQ-202 */\nfunction x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-201"));
        assert!(results.contains_key("REQ-202"));
    }

    #[test]
    fn typescript_jsdoc_comment() {
        let file = create_temp_file(".ts", "/** REQ-210: ts jsdoc */\nconst x: number = 1;");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-210"));
    }

    #[test]
    fn go_comment() {
        let file = create_temp_file(".go", "package main\n\n// REQ-300: go comment\nfunc main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-300"));
    }

    #[test]
    fn java_javadoc_comment() {
        let file = create_temp_file(".java", "/** REQ-400: javadoc */\npublic class Foo {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-400"));
    }

    #[test]
    fn c_doxygen_block_comment() {
        let file = create_temp_file(".c", "/** REQ-500: doxygen */\nint main() { return 0; }");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-500"));
    }

    #[test]
    fn cpp_triple_slash_comment() {
        let file = create_temp_file(".cpp", "/// REQ-510: cpp doc\nint main() { return 0; }");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-510"));
    }

    #[test]
    fn rust_inner_doc_comment() {
        let file = create_temp_file(".rs", "//! REQ-600: inner doc\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-600"));
    }

    #[test]
    fn rust_inner_block_doc_comment() {
        let file = create_temp_file(".rs", "/*! REQ-610: inner block doc */\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-610"));
    }

    // ==================== Edge cases: duplicates ====================

    #[test]
    fn dedupes_same_slug_same_line() {
        let file = create_temp_file(".rs", "/// REQ-1 REQ-1 REQ-1\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results["REQ-1"].len(), 1);
    }

    #[test]
    fn multiple_different_slugs_same_line() {
        let file = create_temp_file(".rs", "/// REQ-1 REQ-2 REQ-3\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
        assert!(results.contains_key("REQ-3"));
    }

    #[test]
    fn same_slug_different_lines() {
        let file = create_temp_file(".rs", "/// REQ-1\n/// REQ-1\n/// REQ-1\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results["REQ-1"].len(), 3);
    }

    // ==================== Edge cases: malformed patterns ====================

    #[test]
    fn ignores_slug_without_number() {
        let file = create_temp_file(".rs", "/// REQ-abc: not a number\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn ignores_slug_without_hyphen() {
        let file = create_temp_file(".rs", "/// REQ123: no hyphen\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn ignores_partial_slug_match() {
        let file = create_temp_file(".rs", "/// MYREQ-123: prefix mismatch\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        // "REQ-123" should still match within "MYREQ-123"
        assert!(results.contains_key("REQ-123"));
    }

    #[test]
    fn matches_slug_with_leading_zeros() {
        let file = create_temp_file(".rs", "/// REQ-007: leading zeros\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-007"));
    }

    #[test]
    fn matches_very_large_number() {
        let file = create_temp_file(".rs", "/// REQ-999999999: large\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-999999999"));
    }

    #[test]
    fn ignores_empty_number() {
        let file = create_temp_file(".rs", "/// REQ-: empty number\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn case_sensitive_slug() {
        let file = create_temp_file(".rs", "/// req-1 Req-2 REQ-3\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains_key("REQ-3"));
    }

    // ==================== Edge cases: stacked/multiline ====================

    #[test]
    fn stacked_doc_comments() {
        let file = create_temp_file(
            ".rs",
            "/// REQ-1: first\n/// REQ-2: second\n/// REQ-3: third\nfn x() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results["REQ-1"][0].line, 1);
        assert_eq!(results["REQ-2"][0].line, 2);
        assert_eq!(results["REQ-3"][0].line, 3);
    }

    #[test]
    fn multiline_block_doc_comment() {
        let file = create_temp_file(
            ".rs",
            "/**\n * REQ-1: first line\n * REQ-2: second line\n */\nfn x() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
    }

    #[test]
    fn finds_all_comment_styles() {
        let file = create_temp_file(
            ".rs",
            "// REQ-1: line\n/// REQ-2: doc\n/* REQ-3 */\n/** REQ-4 */\nfn x() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
        assert!(results.contains_key("REQ-3"));
        assert!(results.contains_key("REQ-4"));
    }

    // ==================== Edge cases: empty/minimal ====================

    #[test]
    fn empty_file() {
        let file = create_temp_file(".rs", "");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn file_with_no_comments() {
        let file = create_temp_file(".rs", "fn main() {\n    println!(\"hello\");\n}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn doc_comment_with_only_whitespace() {
        let file = create_temp_file(".rs", "///    \nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn slug_surrounded_by_special_chars() {
        let file = create_temp_file(".rs", "/// [REQ-1] (REQ-2) {REQ-3} <REQ-4>\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert_eq!(results.len(), 4);
    }

    #[test]
    fn slug_at_end_of_line() {
        let file = create_temp_file(".rs", "/// implements REQ-1\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
    }

    #[test]
    fn slug_in_url_like_context() {
        let file = create_temp_file(".rs", "/// see https://tracker.com/REQ-123\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-123"));
    }

    // ==================== Multiple files ====================

    #[test]
    fn scans_multiple_files() {
        let file1 = create_temp_file(".rs", "/// REQ-1\nfn a() {}");
        let file2 = create_temp_file(".rs", "/// REQ-2\nfn b() {}");
        let root = file1.path().parent().unwrap();
        let results = scan_files(
            root,
            &[file1.path().to_path_buf(), file2.path().to_path_buf()],
            &scan_args("REQ"),
        )
        .unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("REQ-2"));
    }

    #[test]
    fn aggregates_same_slug_across_files() {
        let file1 = create_temp_file(".rs", "/// REQ-1: in file1\nfn a() {}");
        let file2 = create_temp_file(".rs", "/// REQ-1: in file2\nfn b() {}");
        let root = file1.path().parent().unwrap();
        let results = scan_files(
            root,
            &[file1.path().to_path_buf(), file2.path().to_path_buf()],
            &scan_args("REQ"),
        )
        .unwrap();

        assert_eq!(results["REQ-1"].len(), 2);
    }

    // ==================== Special slug patterns ====================

    #[test]
    fn slug_with_lowercase() {
        let file = create_temp_file(".rs", "/// feat-123: lowercase slug\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("feat")).unwrap();

        assert!(results.contains_key("feat-123"));
    }

    #[test]
    fn slug_with_mixed_case() {
        let file = create_temp_file(".rs", "/// MySlug-42: mixed case\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results =
            scan_files(root, &[file.path().to_path_buf()], &scan_args("MySlug")).unwrap();

        assert!(results.contains_key("MySlug-42"));
    }

    #[test]
    fn slug_with_numbers() {
        let file = create_temp_file(".rs", "/// ABC123-456: numbers in slug\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results =
            scan_files(root, &[file.path().to_path_buf()], &scan_args("ABC123")).unwrap();

        assert!(results.contains_key("ABC123-456"));
    }

    #[test]
    fn single_digit_number() {
        let file = create_temp_file(".rs", "/// REQ-1\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-1"));
    }

    #[test]
    fn zero_as_number() {
        let file = create_temp_file(".rs", "/// REQ-0: zero is valid\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        assert!(results.contains_key("REQ-0"));
    }
}
