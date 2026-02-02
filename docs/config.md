# Config (`tracy.toml`)

## Discovery

- Default: search for `tracy.toml` from CWD upward
- Override: `--config path/to/tracy.toml`
- Disable: `--no-config`

CLI overrides config.

## Example

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

## Keys

Top-level:

- `root` (string): scan root (relative paths resolved vs config dir)
- `format` (`json|jsonl|csv|sarif`)
- `output` (string)
- `quiet` (bool)
- `fail_on_empty` (bool)
- `include_git_meta` (bool)
- `include_blame` (bool)

`[scan]`:

- `slug` (string array)

`[filter]`:

- `include_vendored` (bool)
- `include_generated` (bool)
- `include_submodules` (bool)
- `include` (string array, glob)
- `exclude` (string array, glob)

