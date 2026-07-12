# PotatoLog

Web-based structured log viewer. Parses CORE.OUT (tilde-delimited) and reu.out (SQL trace) formats, displays all original columns, and provides per-column filters with pagination.

## Features

- **Auto-detect** CORE.OUT (`~@_~` delimited) and reu.out (SQL trace with CONTEXT headers)
- **11 original columns**: VM, Objeto, Procedimiento, Usuario, Fecha, Thr, Texto, Fuente, Línea, Fuente C, Línea C
- **Per-column filters**: text inputs, multi-select dropdown for Usuario, date range pickers for Fecha
- **Server-side pagination** (2000 records per page) + server-side filtering (all filters applied server-side)
- **Dark/light theme** toggle with localStorage persistence
- **Column resizing** via drag handles, widths persist in localStorage
- **Trigger toggle** — hides records whose Objeto starts with `TRIGGER`; persists across files
- **XML formatting** in CORE.OUT messages (syntax-highlighted modal)
- **SQL syntax highlighting** in reu.out messages (colored operation bars + keyword highlighting in modal)
- **JSON formatting** in any message starting with `{` or `[` (pretty-printed with syntax highlighting in modal)
- **Optimized for large files**: handles 382K-record (156 MB) CORE.OUT and 3.8K-query (13 MB) reu.out
- **macOS .app bundle** (Tauri) with embedded web server

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   readers    │ ──▶ │   process    │ ──▶ │  web (warp)  │
│ (file I/O)   │     │ (filtering)  │     │   (server)   │
└──────────────┘     └──────────────┘     └──────┬───────┘
                        │                        │
                   ┌────▼────┐            ┌──────▼───────┐
                   │ filters │            │  Frontend    │
                   │ (regex) │            │ (HTML/CSS/JS)│
                   └─────────┘            └──────────────┘
                                                    │
                                           ┌────────▼───────┐
                                           │  Tauri (macOS) │
                                           │   (WebView)    │
                                           └────────────────┘
```

## Quick Start

### Desktop (macOS)

```bash
cd src-tauri && cargo tauri build --bundles "app,dmg"

# Output:
#   target/release/bundle/macos/PotatoLog.app
#   target/release/bundle/dmg/PotatoLog_0.1.0_aarch64.dmg
```

Open `PotatoLog.app` — upload a log file via the "Open file…" button.

### CLI + Web Server

```bash
# Build CLI
cargo build --release

# Run web server (uploads via UI at http://127.0.0.1:8000)
./target/release/potatolog web

# Process log with view (JSON lines to stdout)
./target/release/potatolog process view_core.json examples/CORE.OUT
```

### Desktop binary only (no bundle)

```bash
cargo build --release -p potatolog-desktop
# Binary: target/release/potatolog-desktop
```

### Tests

```bash
cargo test    # 14 tests pass
```

## Components

| Module | File | Description |
|--------|------|-------------|
| `lib` | `src/lib.rs` | Core types: `Record`, `Color` |
| `filters` | `src/filters.rs` | Regex view definitions, pattern matching, view DSL |
| `readers` | `src/readers.rs` | File readers: `LogFile`, `LogCoreReader`, `LogQueryReader` |
| `process` | `src/process.rs` | Filter pipeline: applies view operations to records |
| `web` | `src/web.rs` | HTTP server (warp) + embedded frontend |
| `query` | `src/query.rs` | Query engine: caching, filtering, pagination, JSON response |
| `cli` | `src/cli.rs` | CLI entry: `process` and `web` subcommands |
| `desktop` | `src-tauri/` | Tauri desktop app with embedded warp server |

## Readers

| Reader | Format | Detection |
|--------|--------|-----------|
| **LogFile** | Line-by-line (any text) | Fallback |
| **LogCoreReader** | `~@_~`-delimited (CORE.OUT) | First 512B contain `~@_~` |
| **LogQueryReader** | CONTEXT headers + SQL (reu.out) | First 512B start with `/***` |

## Views

View files (JSON) define regex patterns to extract fields:

- **view_core.json** — 15 `~`-delimited fields, 11 captured (CORE.OUT)
- **view_reu.json** — CONTEXT fields + SQL query (reu.out)

Resolution order: next to log file → next to executable → `Resources/` (macOS bundle) → cwd → embedded in binary.

## Optimizations

### I/O + CPU
- Batch I/O in LogCoreReader (`fill_buf()`/`consume()`) — eliminates ~156M byte reads
- Pre-computed lowercase filters outside loop — no per-record allocs
- `Cow<str>` in evaluate() — avoids cloning record text
- `spawn_blocking` — queries in thread pool, doesn't block tokio

### Memory + Frontend
- `search_texts` eliminated (~248 MB), `text_lower` eliminated (~157 MB)
- Single-pass field extraction in `record_matches` (O(12) loop vs 11 linear searches)
- Binary search for date break (O(log n) vs O(n))
- `detect_op_class()` without allocs (byte comparisons)
- `esc()` after truncation — no HTML-escape on 100KB XML, only on 200-char slice
- `into_owned()` eliminated in Match branch — `Cow<str>` passed directly
- `getComputedStyle` cached — one lookup, invalidated on theme toggle
- User dropdown cached per file — only rebuilt when file changes
- Byte comparisons in LogCoreReader — no `byte as char` cast

### Filter logic
- `has_non_triggers` in `CachedDataSet` — if all records are triggers (reu.out), skip trigger filter entirely; fast path always active

## File Structure

```
├── src/
│   ├── cli.rs         CLI entry point
│   ├── filters.rs     View/pattern engine
│   ├── lib.rs         Core types
│   ├── process.rs     Filter pipeline
│   ├── query.rs       Query engine
│   ├── readers.rs     File readers
│   └── web.rs         HTTP server + frontend
├── src-tauri/
│   ├── src/
│   │   ├── main.rs    Desktop entry
│   │   └── lib.rs     Desktop runtime
│   ├── tauri.conf.json
│   └── Cargo.toml
├── static/
│   ├── index.html     Frontend
│   └── api.js         API client
├── view_core.json     CORE.OUT regex view
├── view_reu.json      reu.out regex view
├── Cargo.toml         Workspace root
└── AGENTS.md          Dev notes
```
