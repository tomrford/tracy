# Tracy

Scans codebases for requirement references in comments and outputs JSON.

## Usage

```bash
tracy --slug REQ --root .
```

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
| `--root`               | Root directory to scan (default: `.`)          |
| `--output`, `-o`       | Write output to file                           |
| `--quiet`, `-q`        | Suppress stdout output                         |
| `--fail-on-empty`      | Exit with error if no matches found            |
| `--include-git-meta`   | Include git repository metadata in output      |
| `--include-vendored`   | Include vendored files (per `.gitattributes`)  |
| `--include-generated`  | Include generated files (per `.gitattributes`) |
| `--include-submodules` | Include git submodules                         |

## Supported Languages

All languages supported by [ast-grep](https://ast-grep.github.io/guide/introduction.html#supported-languages), including Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, and more.

## License

MIT
