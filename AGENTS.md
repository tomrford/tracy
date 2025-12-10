# Tracy

CI traceability scanner - scans codebases for requirement references in comments and outputs JSON.

## Stack

- Rust single binary
- `ast-grep-core` for multi-language AST parsing
- `ignore` crate for Git-aware file traversal
- `clap` for CLI

## Commands

```bash
cargo build          # build
cargo run            # run
cargo test           # test
cargo clippy         # lint
```

## Architecture

1. Git-aware file discovery (respects .gitignore, optionally includes submodules/vendored/generated)
2. AST parsing per file to find comments
3. Pattern matching for requirement refs (e.g., `REQ-123`)
