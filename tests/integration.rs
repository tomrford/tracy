use std::collections::BTreeMap;
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn run_scan(
    include_vendored: bool,
    include_generated: bool,
) -> BTreeMap<String, Vec<tracy::scan::Entry>> {
    let root = fixture_root();
    let filter_args = tracy::filter::FilterArgs {
        include_vendored,
        include_generated,
        include_submodules: false,
        include: Vec::new(),
        exclude: Vec::new(),
    };
    let scan_args = tracy::scan::ScanArgs {
        slug: vec!["REQ".to_string()],
    };

    let files = tracy::filter::collect_files(&root, &filter_args).unwrap();
    tracy::scan::scan_files(&root, &files, &scan_args).unwrap()
}

#[test]
fn finds_rust_doc_comments() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-1"), "inner doc //!");
    assert!(results.contains_key("REQ-2"), "outer doc ///");
    assert!(results.contains_key("REQ-3"), "block doc /**");
}

#[test]
fn finds_typescript_jsdoc() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-10"));
    assert!(results.contains_key("REQ-11"));
    assert!(results.contains_key("REQ-12"));
}

#[test]
fn finds_javascript_jsdoc() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-20"));
    assert!(results.contains_key("REQ-21"));
    assert!(results.contains_key("REQ-22"));
}

#[test]
fn finds_java_javadoc() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-40"));
    assert!(results.contains_key("REQ-41"));
}

#[test]
fn finds_cpp_doc_comments() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-50"));
    assert!(results.contains_key("REQ-51"));
}

#[test]
fn finds_go_comments() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-30"), "Go // comment");
    assert!(results.contains_key("REQ-31"), "Go /* comment");
}

#[test]
fn finds_regular_comments_in_all_languages() {
    let results = run_scan(false, false);

    assert!(
        results.contains_key("REQ-999"),
        "regular comments should be found"
    );
}

#[test]
fn excludes_vendored_by_default() {
    let results = run_scan(false, false);

    assert!(
        !results.contains_key("REQ-100"),
        "vendor/ should be excluded"
    );
    assert!(
        !results.contains_key("REQ-101"),
        "third_party/ should be excluded"
    );
}

#[test]
fn includes_vendored_when_flag_set() {
    let results = run_scan(true, false);

    assert!(
        results.contains_key("REQ-100"),
        "vendor/ included with flag"
    );
    assert!(
        results.contains_key("REQ-101"),
        "third_party/ included with flag"
    );
}

#[test]
fn excludes_generated_by_default() {
    let results = run_scan(false, false);

    assert!(
        !results.contains_key("REQ-102"),
        "src/gen/ should be excluded"
    );
    assert!(
        !results.contains_key("REQ-103"),
        "*.generated.rs should be excluded"
    );
}

#[test]
fn includes_generated_when_flag_set() {
    let results = run_scan(false, true);

    assert!(
        results.contains_key("REQ-102"),
        "src/gen/ included with flag"
    );
    assert!(
        results.contains_key("REQ-103"),
        "*.generated.rs included with flag"
    );
}

#[test]
fn excludes_gitignored_files() {
    let results = run_scan(true, true);

    assert!(!results.contains_key("REQ-200"), "ignored/ is gitignored");
    assert!(!results.contains_key("REQ-201"), "build/ is gitignored");
}

#[test]
fn aggregates_same_slug_across_files() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-2"));
    assert!(
        results["REQ-2"].len() >= 2,
        "REQ-2 appears in main.rs and lib.rs"
    );
}

#[test]
fn handles_multiple_slugs_on_one_line() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-60"));
    assert!(results.contains_key("REQ-61"));
    assert!(results.contains_key("REQ-62"));
}

#[test]
fn same_slug_different_lines_creates_multiple_entries() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-63"));
    assert_eq!(results["REQ-63"].len(), 2, "REQ-63 on two different lines");
}

#[test]
fn dedupes_same_slug_same_line() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-64"));
    assert_eq!(results["REQ-64"].len(), 1, "REQ-64 repeated on same line");
}

#[test]
fn finds_inner_block_doc() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-65"));
}

#[test]
fn finds_slugs_with_special_chars_around() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-66"), "[REQ-66]");
    assert!(results.contains_key("REQ-67"), "(REQ-67)");
    assert!(results.contains_key("REQ-68"), "{{REQ-68}}");
}

#[test]
fn finds_trailing_slugs() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-69"));
}

#[test]
fn finds_slugs_in_urls() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-70"));
}

#[test]
fn preserves_leading_zeros() {
    let results = run_scan(false, false);

    assert!(results.contains_key("REQ-007"));
}

#[test]
fn file_paths_are_relative() {
    let results = run_scan(false, false);

    let entry = &results["REQ-1"][0];
    assert!(
        !entry.file.is_absolute(),
        "file path should be relative: {:?}",
        entry.file
    );
    assert!(
        entry.file.starts_with("src/"),
        "should start with src/: {:?}",
        entry.file
    );
}

#[test]
fn skips_unsupported_file_types() {
    let results = run_scan(false, false);

    assert!(!results.contains_key("REQ-300"), "markdown not supported");
    assert!(
        !results.contains_key("REQ-301"),
        "json not supported for comments"
    );
}
