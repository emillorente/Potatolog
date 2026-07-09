---
name: logviewer-tauri-desktop
description: >-
  Build and migrate LogViewer to a Windows portable desktop app with Tauri 2.
  Use when working on src-tauri/, desktop IPC, WebView2, package-windows.ps1,
  query engine extraction, or replacing warp HTTP with Tauri commands.
---

# LogViewer — Desktop Windows (Tauri 2)

## When to use

- Implementing or planning the Tauri desktop app
- Extracting logic from `src/web.rs` into `src/query.rs`
- Wiring `static/frontend.html` to Tauri `invoke` instead of `fetch`
- Packaging portable Windows builds
- Reviewing Rust/Tauri dependency versions

## Project constraints

- **Windows only** for desktop (portable folder + ZIP)
- **Reuse** existing HTML/CSS/JS in `static/frontend.html` — no React/npm unless strictly needed
- **Reuse** Rust core: `readers.rs`, `filters.rs`, `process.rs`, rayon cache
- **Keep** CLI `process` subcommand unchanged
- **Feature `web`** (warp) is dev/legacy only after desktop ships

## Architecture rules

1. Business logic lives in the **library** (`src/query.rs`), not in `src-tauri/main.rs`
2. Tauri commands are thin wrappers: validate input ? call `query::*` ? return JSON
3. Desktop opens files via **`tauri-plugin-dialog`**, not HTTP upload to `%TEMP%`
4. Views are embedded via `include_str!` in query module (standalone exe)
5. Frontend uses `static/api.js` abstraction — one code path for Tauri and web

## Key files

| Path | Role |
|------|------|
| `docs/desktop-tauri.md` | Plan and phases |
| `docs/dependencies.md` | Version audit |
| `rust-toolchain.toml` | Pinned Rust |
| `src/web.rs` | Legacy warp server (shrink to adapter) |
| `src/query.rs` | Target: shared query engine (create in phase 1) |
| `static/frontend.html` | UI |
| `static/api.js` | IPC/fetch bridge (create in phase 3) |
| `src-tauri/` | Tauri app (create in phase 2) |
| `scripts/package-windows.ps1` | Portable packaging |

## Tauri command template

```rust
#[tauri::command]
fn query_logs(state: State<AppState>, params: QueryParams) -> Result<QueryResponse, String> {
    logviewer::query::query_records(&state, params).map_err(|e| e.to_string())
}
```

## Frontend invoke template

```javascript
async function apiQuery(params) {
  if (window.__TAURI__) {
    const { invoke } = window.__TAURI__.core;
    return invoke('query_logs', { params: Object.fromEntries(params) });
  }
  const res = await fetch('/api/query?' + params.toString());
  return res.json();
}
```

## Dependencies (desktop target)

```toml
# src-tauri/Cargo.toml
tauri = { version = "2.11", features = [] }
tauri-plugin-dialog = "2"
logviewer = { path = ".." }
```

Do **not** add warp/tokio/bytes to the desktop binary.

## Windows prerequisites

- Rust ? 1.77.2 (project pins 1.90 in `rust-toolchain.toml`)
- VS Build Tools 2022 (C++)
- WebView2 Runtime
- `cargo install tauri-cli --version "^2.0"`

## Build commands

```powershell
# Dev
cargo tauri dev

# Release portable (no installer)
cargo tauri build --bundles none

# Package
.\scripts\package-windows.ps1
```

## Testing checklist

- [ ] Open CORE.OUT via native dialog — columns populated
- [ ] Open reu.out — SQL highlighting works
- [ ] Filters, pagination, dark theme in WebView2
- [ ] Multi-user filter (comma-separated OR)
- [ ] Large file (~100MB+) — cache + rayon filter < 1s on repeat queries
- [ ] Portable: copy `dist/LogViewer/` to another path — still works
- [ ] CLI `process` still works: `cargo run -- process view_core.json file.OUT`

## Common mistakes

- Putting filter/cache logic inside `src-tauri/main.rs` — keep in lib
- Using `fetch('/api/...')` only — breaks desktop mode
- Bundling warp in desktop release — bloat + port conflicts
- Forgetting WebView2 CSS quirks — test `color-scheme` and datetime inputs
- Removing `include_str!` views — breaks single-exe portability

## Version policy

Read `docs/dependencies.md` before bumping crates. Prefer:

- `tauri = "2.11"`
- `regex = "1.12"` (upgrade from locked 1.4.2 when touching manifest)
- `rayon = "1.12"`

Do not upgrade warp to 0.4 if removing web mode.
