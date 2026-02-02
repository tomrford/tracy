use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    dir
}

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-m", message]);
}

fn write_file(repo: &Path, rel: &str, content: &str) -> PathBuf {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn run_tracy(cwd: &Path, args: &[&str]) -> Output {
    let bin = env!("CARGO_BIN_EXE_tracy");
    Command::new(bin)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn include_git_meta_wraps_json_output() {
    let repo = init_repo();
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one\n");
    commit_all(repo.path(), "init");

    let out = run_tracy(
        repo.path(),
        &[
            "--no-config",
            "--root",
            repo.path().to_str().unwrap(),
            "--slug",
            "REQ",
            "--include-git-meta",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value.get("meta").is_some());
    assert!(value.get("results").is_some());
    assert_eq!(value["meta"]["head_sha"].as_str().unwrap().len(), 40);
    assert_eq!(value["results"]["REQ-1"][0]["file"], "src/lib.rs");
}

#[test]
fn jsonl_emits_meta_then_match_lines() {
    let repo = init_repo();
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one\n");
    commit_all(repo.path(), "init");

    let out = run_tracy(
        repo.path(),
        &[
            "--no-config",
            "--root",
            repo.path().to_str().unwrap(),
            "--slug",
            "REQ",
            "--include-git-meta",
            "--format",
            "jsonl",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim_end().split('\n').collect();
    assert!(lines.len() >= 2);

    let meta: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(meta["type"], "meta");

    let m: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(m["type"], "match");
    assert_eq!(m["requirement_id"], "REQ-1");
    assert_eq!(m["entry"]["file"], "src/lib.rs");
}

#[test]
fn csv_includes_header_and_rows() {
    let repo = init_repo();
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one\n");
    commit_all(repo.path(), "init");

    let out = run_tracy(
        repo.path(),
        &[
            "--no-config",
            "--root",
            repo.path().to_str().unwrap(),
            "--slug",
            "REQ",
            "--include-git-meta",
            "--format",
            "csv",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.trim_end().lines();
    let header = lines.next().unwrap();
    assert!(header.contains("requirement_id"));
    assert!(header.contains("repo_root"));
    assert!(header.contains("head_sha"));
    assert!(header.contains("blame"));

    let row = lines.next().unwrap();
    assert!(row.contains("REQ-1"));
    assert!(row.contains("src/lib.rs"));
}

#[test]
fn sarif_has_expected_shape() {
    let repo = init_repo();
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one\n");
    commit_all(repo.path(), "init");

    let out = run_tracy(
        repo.path(),
        &[
            "--no-config",
            "--root",
            repo.path().to_str().unwrap(),
            "--slug",
            "REQ",
            "--format",
            "sarif",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "tracy");
    assert_eq!(
        value["runs"][0]["results"][0]["ruleId"],
        "traceability.requirement_ref"
    );
    assert_eq!(
        value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/lib.rs"
    );
}

#[test]
fn config_autodiscovery_sets_slug_and_filters() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "tracy.toml",
        r#"
[scan]
slug = ["REQ"]

[filter]
include = ["src/**"]
exclude = ["src/gen/**"]
"#,
    );
    write_file(repo.path(), "src/lib.rs", "// REQ-1: keep\n");
    write_file(repo.path(), "src/gen/types.rs", "// REQ-2: drop\n");

    let nested = repo.path().join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();

    let out = run_tracy(&nested, &[]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value.get("REQ-1").is_some());
    assert!(value.get("REQ-2").is_none());
}

#[test]
fn cli_overrides_config_slug() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "tracy.toml",
        r#"
[scan]
slug = ["REQ"]
"#,
    );
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one LIN-1\n");

    let out = run_tracy(repo.path(), &["--slug", "LIN"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value.get("LIN-1").is_some());
    assert!(value.get("REQ-1").is_none());
}

#[test]
fn include_blame_populates_commit_ids() {
    let repo = init_repo();
    write_file(repo.path(), "src/lib.rs", "// REQ-1: one\n// REQ-2: two\n");
    commit_all(repo.path(), "first");
    let first_sha = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let first_sha = String::from_utf8(first_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    write_file(
        repo.path(),
        "src/lib.rs",
        "// REQ-1: one\n// REQ-2: two changed\n",
    );
    commit_all(repo.path(), "second");
    let second_sha = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let second_sha = String::from_utf8(second_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    let out = run_tracy(
        repo.path(),
        &[
            "--no-config",
            "--root",
            repo.path().to_str().unwrap(),
            "--slug",
            "REQ",
            "--include-blame",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let blame_1 = value["REQ-1"][0]["blame"]["commit"].as_str().unwrap();
    let blame_2 = value["REQ-2"][0]["blame"]["commit"].as_str().unwrap();
    assert_eq!(blame_1, first_sha);
    assert_eq!(blame_2, second_sha);
}
