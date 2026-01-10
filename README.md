# desktop-indexer

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

Fast index/search for Linux `.desktop` entries, with an optional local IPC daemon for low-latency typeahead search.

This project is designed to be used as a “launcher backend”: build an in-memory index of applications, search it quickly, and launch by `desktop-id`.

## Features

- Scans XDG application roots (`XDG_DATA_HOME` + `XDG_DATA_DIRS`) and parses `.desktop` files.
- Launcher-grade fields: Name/GenericName/Comment/Categories/Keywords/MimeType, plus `[Desktop Action ...]` entries.
- Incremental on-disk cache to avoid re-parsing unchanged files.
- Optional IPC daemon (Unix socket) with JSON-line protocol:
	- `search`, `list`, `launch`, `status`, `warmup`, `shutdown`
- Transparent fallback to local execution when the daemon is unavailable.
- Observability:
	- `--trace` prints whether a command ran via daemon or local fallback.
	- `DESKTOP_INDEXER_TIMING=1` prints client timings to stderr.
- Personalized ranking: persistent frequency + recency boosts based on successful launches.
- Optional filtering of entries with `TryExec` missing (`--respect-try-exec`).

## Install

### From source

```bash
cargo build --release
./target/release/desktop-indexer --help
```

## Quick start

Search apps (human output):

```bash
desktop-indexer search "code"
```

Empty query (show recent launches):

```bash
desktop-indexer search "" --limit 10
```

Empty query (show most frequent launches):

```bash
desktop-indexer search "" --empty-mode frequency --limit 10
```

Search apps (JSON):

```bash
desktop-indexer search "code" --json
```

Launch an app:

```bash
desktop-indexer launch code
```

Launch a specific Desktop Action:

```bash
desktop-indexer launch org.gnome.Terminal --action new-window
```

List all apps:

```bash
desktop-indexer list
```

Scan and parse (debug/tooling):

```bash
desktop-indexer scan --parse
desktop-indexer scan --parse --json
```

## Daemon mode (recommended for launchers)

Start daemon in background:

```bash
desktop-indexer daemon start
```

Check status:

```bash
desktop-indexer daemon status
desktop-indexer daemon status --json
```

Stop daemon:

```bash
desktop-indexer daemon stop
```

Restart daemon (useful after upgrading the binary):

```bash
desktop-indexer daemon restart
```

Legacy commands (still supported):

```bash
desktop-indexer start-daemon
desktop-indexer stop-daemon
desktop-indexer status
```

Notes:

- The client commands (`search`, `list`, `launch`) try the daemon first, then fall back to local execution.
- `daemon start` will also send a `warmup` request (unless `--no-daemon` is set) to avoid a first-search spike.
- After upgrading/reinstalling the binary, restart the daemon so it uses the new version:
	- `desktop-indexer daemon restart`

## IPC protocol (for QuickShell / custom clients)

Transport:

- Unix domain socket (path resolution):
	- `$XDG_RUNTIME_DIR/desktop-indexer.sock` if `XDG_RUNTIME_DIR` is set
	- else `/tmp/desktop-indexer-$USER.sock`

Framing:

- One JSON object per line (`\n`). One request line → one response line.

Request examples:

```json
{"cmd":"status"}
```

```json
{"cmd":"warmup","roots":["/home/me/.local/share/applications","/usr/share/applications"],"respect_try_exec":false}
```

```json
{"cmd":"search","roots":["/home/me/.local/share/applications"],"query":"code","limit":20,"respect_try_exec":false}
```

Empty query (recency vs frequency):

```json
{"cmd":"search","roots":["/home/me/.local/share/applications"],"query":"","limit":20,"empty_mode":"recency","respect_try_exec":false}
```

Where `empty_mode` is optional and can be:

- `"recency"` (default)
- `"frequency"`

```json
{"cmd":"launch","roots":["/home/me/.local/share/applications"],"desktop_id":"code.desktop","action":null,"respect_try_exec":false}
```

Response examples:

```json
{"type":"ok"}
```

```json
{"type":"status","has_index_count":1}
```

```json
{"type":"entries","entries":[{"id":"code","name":"Visual Studio Code", ...}]}
```

Important integration detail:

- The daemon caches indexes *by the exact `roots` list* (order matters). If you build your own client, keep the roots list consistent with the tool’s XDG logic to avoid building multiple indexes.
- The daemon also keys indexes by `respect_try_exec` (so clients should keep it consistent too).

## Configuration

### Scan roots

By default, roots are derived from XDG:

- `XDG_DATA_HOME/applications` (default: `~/.local/share/applications`)
- for each entry in `XDG_DATA_DIRS`: `<dir>/applications` (default: `/usr/local/share:/usr/share`)

You can add extra scan roots with `-p/--path` (repeatable).

### Environment variables

- `DESKTOP_INDEXER_TIMING=1|true|yes`: print end-to-end client timing to stderr.

### Flags

- `--trace`: prints daemon vs local mode (stderr).
- `--no-daemon`: forces local execution and skips daemon warmup.
- `--respect-try-exec`: hide entries whose `.desktop` has `TryExec` but the executable is not available.

## Development

```bash
cargo fmt
cargo clippy
cargo test
```

## License

Licensed under either of:

- MIT License: see [LICENSE-MIT](LICENSE-MIT)
- Apache License 2.0: see [LICENSE-APACHE](LICENSE-APACHE)

You may choose either license.
