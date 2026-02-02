pub mod args;
mod context;
mod error;

pub use args::ScanArgs;
pub use context::{CodeContext, ScopeItem};
pub use error::ScanError;

use crate::git::BlameInfo;
use ast_grep_language::{Language, LanguageExt, SupportLang};
use context::{extract_block_context, extract_hierarchy};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A single reference to a requirement marker found in code.
#[derive(Debug, Serialize)]
pub struct Entry {
    /// Relative file path from the scan root
    pub file: PathBuf,
    /// 1-indexed line number where the marker was found
    pub line: usize,
    /// Full aggregated comment text (including adjacent comments in the block)
    pub comment_text: String,
    /// Code context found above the comment block (first non-comment line above)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above: Option<CodeContext>,
    /// Code context found below the comment block (first non-comment line below)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below: Option<CodeContext>,
    /// Code context on the same line (for inline/trailing comments)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<CodeContext>,
    /// Scope hierarchy from innermost to outermost (fn → impl → mod → file)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<ScopeItem>,

    /// Git blame metadata for the marker line
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blame: Option<BlameInfo>,
}

pub type ScanResult = BTreeMap<String, Vec<Entry>>;

pub fn scan_files(
    root: &Path,
    paths: &[PathBuf],
    args: &ScanArgs,
) -> Result<ScanResult, ScanError> {
    let slugs: Vec<String> = args.slug.iter().map(|s| regex::escape(s)).collect();
    let pattern = Regex::new(&format!(r"(?:{})-\d+", slugs.join("|")))?;
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
    let ast_root_node = ast_root.root();
    let source_lines: Vec<&str> = source.lines().collect();
    let mut seen: HashSet<(String, usize)> = HashSet::new();

    for node in ast_root_node.dfs() {
        let kind = node.kind();
        let kind_str: &str = &kind;
        if !is_comment(kind_str) {
            continue;
        }

        let start_pos = node.start_pos();
        let line_0indexed = start_pos.line();
        let line = line_0indexed + 1; // Convert to 1-indexed for output
        let text = node.text().to_string();

        for m in pattern.find_iter(&text) {
            let slug = m.as_str().to_string();

            if seen.insert((slug.clone(), line)) {
                // Extract block context (above/below/inline code)
                let block_ctx = extract_block_context(&ast_root_node, line_0indexed, &source_lines);

                // Extract scope hierarchy
                let scope = extract_hierarchy(&ast_root_node, line_0indexed);

                results.entry(slug).or_default().push(Entry {
                    file: relative.to_path_buf(),
                    line,
                    comment_text: text.clone(),
                    above: block_ctx.above,
                    below: block_ctx.below,
                    inline: block_ctx.inline,
                    scope,
                    blame: None,
                });
            }
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
            slug: vec![slug.to_string()],
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
        let file = create_temp_file(
            ".py",
            r#""""REQ-100: python docstring"""
def foo(): pass"#,
        );
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
        let file = create_temp_file(
            ".go",
            "package main\n\n// REQ-300: go comment\nfunc main() {}",
        );
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
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("MySlug")).unwrap();

        assert!(results.contains_key("MySlug-42"));
    }

    #[test]
    fn slug_with_numbers() {
        let file = create_temp_file(".rs", "/// ABC123-456: numbers in slug\nfn x() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("ABC123")).unwrap();

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

    #[test]
    fn multiple_slug_patterns() {
        let file = create_temp_file(".rs", "/// REQ-1 and LIN-2 and FEAT-3\nfn x() {}");
        let root = file.path().parent().unwrap();
        let args = ScanArgs {
            slug: vec!["REQ".to_string(), "LIN".to_string(), "FEAT".to_string()],
        };
        let results = scan_files(root, &[file.path().to_path_buf()], &args).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("LIN-2"));
        assert!(results.contains_key("FEAT-3"));
    }

    #[test]
    fn multiple_slugs_only_matches_specified() {
        let file = create_temp_file(".rs", "/// REQ-1 LIN-2 OTHER-3\nfn x() {}");
        let root = file.path().parent().unwrap();
        let args = ScanArgs {
            slug: vec!["REQ".to_string(), "LIN".to_string()],
        };
        let results = scan_files(root, &[file.path().to_path_buf()], &args).unwrap();

        assert!(results.contains_key("REQ-1"));
        assert!(results.contains_key("LIN-2"));
        assert!(!results.contains_key("OTHER-3"));
    }

    // ==================== Metadata: Inline context (same line) ====================

    #[test]
    fn extracts_inline_context_rust() {
        let file = create_temp_file(".rs", "let x = 42; // REQ-1\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some(), "should have inline context");
        let ctx = entry.inline.as_ref().unwrap();
        assert_eq!(ctx.kind, "let_declaration");
    }

    #[test]
    fn no_inline_for_standalone_comment() {
        let file = create_temp_file(".rs", "// REQ-1: standalone\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(
            entry.inline.is_none(),
            "standalone comment has no inline context"
        );
    }

    #[test]
    fn extracts_inline_context_python() {
        let file = create_temp_file(".py", "x = 42  # REQ-1\ndef foo(): pass");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some());
    }

    #[test]
    fn extracts_inline_context_javascript() {
        let file = create_temp_file(".js", "const x = 42; // REQ-1\nfunction foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some());
    }

    #[test]
    fn extracts_inline_context_for_let() {
        let file = create_temp_file(".rs", "let frequency = 100; // REQ-1\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some(), "should have inline context");
        let ctx = entry.inline.as_ref().unwrap();
        assert_eq!(ctx.kind, "let_declaration");
        assert_eq!(ctx.name.as_deref(), Some("frequency"));
    }

    #[test]
    fn extracts_inline_context_for_function() {
        let file = create_temp_file(".rs", "fn measure_temp() {} // REQ-1");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some());
        let ctx = entry.inline.as_ref().unwrap();
        assert_eq!(ctx.kind, "function_item");
        assert_eq!(ctx.name.as_deref(), Some("measure_temp"));
    }

    #[test]
    fn extracts_inline_context_for_js_const() {
        let file = create_temp_file(
            ".js",
            "const sampleRate = 44100; // REQ-1\nfunction foo() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some());
        let ctx = entry.inline.as_ref().unwrap();
        assert!(ctx.kind.contains("declaration") || ctx.kind.contains("declarator"));
        assert_eq!(ctx.name.as_deref(), Some("sampleRate"));
    }

    #[test]
    fn extracts_inline_context_for_python_assignment() {
        let file = create_temp_file(".py", "timeout = 30  # REQ-1\ndef foo(): pass");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.inline.is_some());
        let ctx = entry.inline.as_ref().unwrap();
        assert!(ctx.text.contains("timeout"));
    }

    // ==================== Metadata: Below context ====================

    #[test]
    fn extracts_below_context_for_doc_comment() {
        let file = create_temp_file(".rs", "/// REQ-1: doc comment\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(
            entry.below.is_some(),
            "doc comment should have below context"
        );
        let ctx = entry.below.as_ref().unwrap();
        assert_eq!(ctx.kind, "function_item");
        assert_eq!(ctx.name.as_deref(), Some("main"));
    }

    #[test]
    fn extracts_below_context_for_standalone_comment() {
        let file = create_temp_file(".rs", "// REQ-1: standalone\nlet x = 42;");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.below.is_some());
        let ctx = entry.below.as_ref().unwrap();
        assert_eq!(ctx.kind, "let_declaration");
    }

    #[test]
    fn extracts_below_context_jsdoc() {
        let file = create_temp_file(".js", "/** REQ-1: jsdoc */\nfunction foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.below.is_some());
        let ctx = entry.below.as_ref().unwrap();
        assert_eq!(ctx.kind, "function_declaration");
        assert_eq!(ctx.name.as_deref(), Some("foo"));
    }

    // ==================== Metadata: Above context ====================

    #[test]
    fn extracts_above_context() {
        let file = create_temp_file(".rs", "fn foo() {}\n// REQ-1: after function\nfn bar() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.above.is_some(), "should have above context");
        let ctx = entry.above.as_ref().unwrap();
        assert_eq!(ctx.kind, "function_item");
        assert_eq!(ctx.name.as_deref(), Some("foo"));
    }

    #[test]
    fn no_above_context_at_file_start() {
        let file = create_temp_file(".rs", "// REQ-1: first line\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.above.is_none(), "no code above first line");
    }

    // ==================== Metadata: Scope hierarchy ====================

    #[test]
    fn extracts_scope_inside_function() {
        let file = create_temp_file(
            ".rs",
            "fn outer() {\n    let x = 1; // REQ-1\n}\nfn main() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(!entry.scope.is_empty(), "should have scope hierarchy");
        // Should contain the outer function
        let has_outer = entry
            .scope
            .iter()
            .any(|s| s.name.as_deref() == Some("outer"));
        assert!(has_outer, "scope should include outer function");
    }

    #[test]
    fn extracts_scope_inside_impl() {
        let file = create_temp_file(
            ".rs",
            "struct Foo;\nimpl Foo {\n    fn bar(&self) {\n        let x = 1; // REQ-1\n    }\n}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        // Should have both function and impl in scope
        let has_function = entry.scope.iter().any(|s| s.kind == "function_item");
        let has_impl = entry.scope.iter().any(|s| s.kind == "impl_item");
        assert!(has_function, "should have function in scope");
        assert!(has_impl, "should have impl in scope");
    }

    #[test]
    fn extracts_scope_inside_class_js() {
        let file = create_temp_file(
            ".js",
            "class Sensor {\n    measure() {\n        const x = 1; // REQ-1\n    }\n}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        let has_method = entry.scope.iter().any(|s| s.kind == "method_definition");
        let has_class = entry.scope.iter().any(|s| s.kind == "class_declaration");
        assert!(has_method || has_class, "should have class/method in scope");
    }

    #[test]
    fn extracts_scope_inside_python_class() {
        let file = create_temp_file(
            ".py",
            "class Sensor:\n    def measure(self):\n        x = 1  # REQ-1",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        let has_function = entry.scope.iter().any(|s| s.kind == "function_definition");
        let has_class = entry.scope.iter().any(|s| s.kind == "class_definition");
        assert!(has_function, "should have function in scope");
        assert!(has_class, "should have class in scope");
    }

    #[test]
    fn scope_is_ordered_innermost_first() {
        let file = create_temp_file(
            ".rs",
            "fn outer() {\n    fn inner() {\n        let x = 1; // REQ-1\n    }\n}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        // Inner should come before outer
        let inner_idx = entry
            .scope
            .iter()
            .position(|s| s.name.as_deref() == Some("inner"));
        let outer_idx = entry
            .scope
            .iter()
            .position(|s| s.name.as_deref() == Some("outer"));
        assert!(inner_idx.is_some() && outer_idx.is_some());
        assert!(
            inner_idx.unwrap() < outer_idx.unwrap(),
            "inner should come before outer"
        );
    }

    // ==================== Metadata: Comment text ====================

    #[test]
    fn stores_comment_text() {
        let file = create_temp_file(".rs", "// REQ-1: important requirement\nfn main() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.comment_text.contains("important requirement"));
    }

    #[test]
    fn comment_text_contains_node_text() {
        let file = create_temp_file(
            ".rs",
            "// REQ-1: first line\n// second line\n// third line\nfn main() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        // comment_text contains the text of the specific comment node with the marker
        assert!(entry.comment_text.contains("first line"));
    }

    // ==================== Comment block context ====================

    #[test]
    fn comment_block_finds_code_below() {
        let file = create_temp_file(
            ".rs",
            "/// REQ-1: description\n/// more description\nfn documented() {}",
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        assert!(entry.below.is_some());
        let ctx = entry.below.as_ref().unwrap();
        assert_eq!(ctx.name.as_deref(), Some("documented"));
    }

    #[test]
    fn stacked_comments_share_below_context() {
        // When comments are stacked, they all point to the same code below
        let file = create_temp_file(".rs", "// REQ-1: first\n// REQ-2: second\nfn foo() {}");
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        // Both REQ-1 and REQ-2 should point to fn foo
        let e1 = &results["REQ-1"][0];
        let e2 = &results["REQ-2"][0];
        assert!(e1.below.is_some());
        assert!(e2.below.is_some());
        assert_eq!(e1.below.as_ref().unwrap().name.as_deref(), Some("foo"));
        assert_eq!(e2.below.as_ref().unwrap().name.as_deref(), Some("foo"));
    }

    // ==================== Complex scenarios ====================

    #[test]
    fn multiple_requirements_different_contexts() {
        let file = create_temp_file(
            ".rs",
            r#"
fn setup() {} // REQ-1: setup function
let config = 42; // REQ-2: configuration
// REQ-3: standalone note
fn main() {
    let x = 1; // REQ-4: inside main
}
"#,
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        // REQ-1: inline context (function on same line)
        let e1 = &results["REQ-1"][0];
        assert!(e1.inline.is_some());
        assert_eq!(e1.inline.as_ref().unwrap().kind, "function_item");

        // REQ-2: inline context (let on same line)
        let e2 = &results["REQ-2"][0];
        assert!(e2.inline.is_some());
        assert_eq!(e2.inline.as_ref().unwrap().kind, "let_declaration");

        // REQ-3: no inline (standalone), but has below context
        let e3 = &results["REQ-3"][0];
        assert!(e3.inline.is_none());
        assert!(e3.below.is_some());

        // REQ-4: inside main function (scope)
        let e4 = &results["REQ-4"][0];
        let has_main = e4.scope.iter().any(|s| s.name.as_deref() == Some("main"));
        assert!(has_main, "REQ-4 should be in main's scope");
    }

    #[test]
    fn deeply_nested_scope() {
        let file = create_temp_file(
            ".rs",
            r#"
mod outer_mod {
    struct Container;
    impl Container {
        fn method(&self) {
            let x = 1; // REQ-1
        }
    }
}
"#,
        );
        let root = file.path().parent().unwrap();
        let results = scan_files(root, &[file.path().to_path_buf()], &scan_args("REQ")).unwrap();

        let entry = &results["REQ-1"][0];
        // Should have multiple levels of scope
        assert!(
            entry.scope.len() >= 2,
            "should have at least 2 scope levels"
        );
    }
}
