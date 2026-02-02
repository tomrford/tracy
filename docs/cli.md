# CLI

## Basics

```bash
tracy --slug REQ --root .
```

## Output formats

- `--format json` (default): JSON object keyed by requirement id
- `--format jsonl`: JSON Lines stream (`type=meta` then `type=match`)
- `--format csv`: CSV rows (one match per row)
- `--format sarif`: SARIF 2.1.0 (for GitHub code scanning, editors)

## Common flags

- `--slug/-s <SLUG>` (repeatable): requirement prefixes, e.g. `REQ`, `LIN`
- `--root <DIR>`: scan root (default: config dir or `.`)
- `--output/-o <PATH>`: write output file (still prints unless `--quiet`)
- `--quiet/-q`: suppress stdout
- `--fail-on-empty`: exit non-zero if no matches found

## Git metadata (optional)

- `--include-git-meta`: top-level `meta` in JSON; extra columns in CSV; run-level properties in SARIF
- `--include-blame`: per-match `blame` object (commit/author/time/summary)

## Filtering

- `--include <GLOB>` (repeatable): allowlist
- `--exclude <GLOB>` (repeatable): blocklist
- `--include-vendored`: include `.gitattributes` `linguist-vendored`
- `--include-generated`: include `.gitattributes` `linguist-generated`
- `--include-submodules`: include submodules

## Examples

SARIF for PR annotations:

```bash
tracy -s REQ --format sarif --output tracy.sarif
```

JSONL for streaming ingestion:

```bash
tracy -s REQ --format jsonl --include-git-meta
```

