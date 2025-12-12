# Tracy

Scans codebases for requirement references in doc comments and outputs JSON.

## Usage

```bash
tracy --slug REQ --root ./src
```

Finds `{SLUG}-{NUMBER}` formatted references in comments across your codebase, returning JSON objects containing a list of appearances and their locations:

```json
{
  "REQ-123": [
    {
      "file": "src/lib.rs",
      "line": 42
    }
  ],
  "REQ-456": [
    {
      "file": "src/lib.rs",
      "line": 22
    },
    {
      "file": "src/main.rs",
      "line": 10
    }
  ]
}
```

## Options

| Flag                   | Description                                    |
| ---------------------- | ---------------------------------------------- |
| `--slug`, `-s`         | Slug pattern to match (e.g., `REQ`, `LIN`)     |
| `--root`               | Root directory to scan (default: `.`)          |
| `--output`, `-o`       | Write output to file                           |
| `--quiet`, `-q`        | Suppress stdout output                         |
| `--include-vendored`   | Include vendored files (per `.gitattributes`)  |
| `--include-generated`  | Include generated files (per `.gitattributes`) |
| `--include-submodules` | Include git submodules                         |

## Supported Languages

All languages supported by [ast-grep](https://ast-grep.github.io/guide/introduction.html#supported-languages), including Rust, TypeScript, JavaScript, Python, Go, Java, C, C++, and more.

## License

MIT
