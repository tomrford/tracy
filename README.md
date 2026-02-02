# Tracy

Scans codebases for requirement references in comments and outputs JSON.

## Usage

```bash
tracy --slug REQ --root .
```

If `tracy.toml` is present (searched from the current directory upwards), Tracy will load it by default. CLI flags override config.

Finds `{SLUG}-{NUMBER}` formatted references in comments across your codebase, returning JSON keyed by requirement id. Repeat `--slug` to match multiple prefixes.

Example (single hit):

```rust
// src/lib.rs
// REQ-1: validate input
```

```json
{
  "REQ-1": [
    {
      "file": "src/lib.rs",
      "line": 1,
      "comment_text": "// REQ-1: validate input"
    }
  ]
}
```

Each entry may also include `above`, `below`, `inline`, and `scope` context fields when available.

## Options

| Flag                   | Description                                    |
| ---------------------- | ---------------------------------------------- |
| `--slug`, `-s`         | Slug pattern to match (e.g., `REQ`, `LIN`)     |
| `--root`               | Root directory to scan (default: config dir or `.`) |
| `--format`             | Output format (`json`, `jsonl`, `csv`, `sarif`) |
| `--config`             | Path to config file (default: search for `tracy.toml`) |
| `--no-config`          | Disable config file loading                    |
| `--output`, `-o`       | Write output to file                           |
| `--quiet`, `-q`        | Suppress stdout output                         |
| `--fail-on-empty`      | Exit with error if no matches found            |
| `--include-git-meta`   | Include git repository metadata in output      |
| `--include-blame`      | Include git blame metadata for each match      |
| `--include-vendored`   | Include vendored files (per `.gitattributes`)  |
| `--include-generated`  | Include generated files (per `.gitattributes`) |
| `--include-submodules` | Include git submodules                         |
| `--include`            | Only include paths matching this glob (repeatable) |
| `--exclude`            | Exclude paths matching this glob (repeatable)  |

## Config

Create a `tracy.toml` at your repo root (or pass `--config path`):

```toml
format = "sarif"
include_git_meta = true
include_blame = true

[scan]
slug = ["REQ"]

[filter]
include = ["src/**"]
exclude = ["**/generated/**"]
```

## Supported Languages

All languages supported by [ast-grep](https://ast-grep.github.io/guide/introduction.html#supported-languages), including Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, and more.

## License

MIT
