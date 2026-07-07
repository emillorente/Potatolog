# LogViewer

Web-based structured log viewer. Parses CORE.OUT (tilde-delimited) and reu.out (SQL trace) formats, displays all original columns, and provides per-column filters with pagination.

## Features

- **Auto-detect** CORE.OUT (`~@_~` delimited) and reu.out (SQL trace with CONTEXT headers)
- **11 original columns**: VM, Objeto, Procedimiento, Usuario, Fecha, Thr, Texto, Fuente, Línea, Fuente C, Línea C
- **Per-column filters**: text inputs, multi-select dropdown for Usuario, date range pickers for Fecha
- **Global search** across all fields (space-separated AND logic)
- **Server-side pagination** (2000 records per page)
- **Dark/light theme** toggle with localStorage persistence
- **Column resizing** via drag handles, widths persist in localStorage
- **Trigger toggle** to hide records whose Objeto starts with `TRIGGER`
- **XML formatting** in CORE.OUT messages (syntax-highlighted modal)
- **SQL syntax highlighting** in reu.out messages (colored operation bars + keyword highlighting)
- **MacOS .app bundle** runs as background agent, opens browser automatically

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   readers   │ ──▶ │   process    │ ──▶ │  web (warp) │
│ (file I/O)  │     │ (filtering)  │     │  (server)   │
└─────────────┘     └──────────────┘     └─────────────┘
                        │                       │
                   ┌────▼────┐           ┌───────▼──────┐
                   │ filters │           │  Frontend    │
                   │ (regex) │           │ (HTML/CSS/JS)│
                   └─────────┘           └──────────────┘
```

### Components

| Module | File | Description |
|--------|------|-------------|
| `lib` | `src/lib.rs` | Core types: `Record`, `Color` |
| `filters` | `src/filters.rs` | Regex view definitions, pattern matching, view DSL |
| `readers` | `src/readers.rs` | File readers: `LogFile`, `LogCoreReader`, `LogQueryReader` |
| `process` | `src/process.rs` | Filter pipeline: applies view operations to records |
| `web` | `src/web.rs` | HTTP server (warp) + embedded frontend (HTML/CSS/JS) |
| `cli` | `src/cli.rs` | CLI entry point: `process` and `web` subcommands |

### Readers

- **LogFile**: Generic line-by-line reader. Reads one line per record.
- **LogCoreReader**: Byte-by-byte state machine for `~@_~`-delimited records. Handles multi-line XML messages spanning many physical lines.
- **LogQueryReader**: Merges SQL trace headers + SQL text into one record for reu.out format.

**Auto-detection**: `detect_reader()` reads the first 512 bytes:
- Starts with `/***` → `LogQueryReader`
- Contains `~@_~` → `LogCoreReader`
- Otherwise → `LogFile`

### Views

View files (JSON) define regex patterns to extract fields from log records:

**view_core.json** (`CORE.OUT`):
```regex
^~(?P<level>[^~]*)~(?P<component>[^~]*)~(?P<proc>[^~]*)~(?P<thread>[^~]*)~
(?P<timestamp>[^~]*)~(?P<user>[^~]*)~(?P<message>[^~]*)~(?P<source>[^~]*)~
(?P<empty>[^~]*)~(?P<line>[^~]*)~(?P<sourceC>[^~]*)~(?P<lineC>[^~]*)~@_~
```

**view_reu.json** (reu.out):
```regex
^/\*\*\* Query\s+\((?P<ms>[^)]+)\)\s+\*\*\*\*\*\*CONTEXT@(?P<level>[^,]+),
(?P<component>[^,]+),(?P<proc>[^,]*),(?P<empty1>[^,]*),(?P<hora>[^,]*),
(?P<ms2>[^,]*),(?P<source>[^,]*),(?P<line>[^;]+).*\*\*\*/~
(?P<message>.*)
```

View files are resolved in this order:
1. Next to the log file
2. Relative to the executable
3. In `Resources/` (macOS .app bundle)
4. Current working directory

### Server

The web server (warp on Tokio runtime):
- **Cache**: In-memory `Arc<RwLock<HashMap>>`, pre-computes ISO timestamps on first load
- **Parallel filtering**: Rayon `par_iter()` for CPU-bound filter operations
- **Upload**: Files uploaded via UI stored in temp directory, auto-cleaned after 3600s
- **Query**: Server-side filtering with case-insensitive substring match, date range, global search

## Build

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- Git

### Build & Run

```bash
git clone <repo> && cd LogViewer

# Build release binary
cargo build --release --all-features

# Run web server (starts empty, upload files via UI)
./target/release/logviewer web

# Process a file with a view (CLI mode)
./target/release/logviewer process view_core.json examples/CORE.OUT
```

### Build macOS .app

```bash
cargo build --release --all-features
mkdir -p target/release/LogViewer.app/Contents/{MacOS,Resources}
cp target/release/logviewer target/release/LogViewer.app/Contents/MacOS/lv-core

# Info.plist
cat > target/release/LogViewer.app/Contents/Info.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>lv-core</string>
    <key>CFBundleIdentifier</key>
    <string>com.remirampin.logviewer</string>
    <key>CFBundleName</key>
    <string>LogViewer</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>logviewer</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>
EOF

# View files + icon
cp view_core.json view_reu.json target/release/LogViewer.app/Contents/Resources/
# (add logviewer.icns to Resources/)

# Sign
rm -rf target/release/LogViewer.app/Contents/_CodeSignature
codesign --force --deep --sign - target/release/LogViewer.app
```

Double-click `LogViewer.app` in Finder. Runs as background agent (no dock icon), opens browser automatically. Upload files via the "Open file…" button.

### Build on Windows

```powershell
# Install Rust from https://rustup.rs/
# Open PowerShell or cmd

git clone <repo> && cd LogViewer
cargo build --release --all-features
# Binary: target\release\logviewer.exe

# Run
.\target\release\logviewer web
```

### Add icon to Windows .exe

1. Create `logo.ico` (32×32 + 256×256 at least)
2. Create `logo.rc` in project root:
   ```rc
   1 ICON "logo.ico"
   ```
3. Add to `Cargo.toml`:
   ```toml
   [package]
   build = "build.rs"

   [build-dependencies]
   embed-resource = "2"
   ```
4. Create `build.rs`:
   ```rust
   fn main() {
       println!("cargo:rerun-if-changed=logo.rc");
       println!("cargo:rerun-if-changed=logo.ico");
       embed_resource::compile("logo.rc", embed_resource::NONE);
   }
   ```
5. Build again — `logviewer.exe` shows your icon in Explorer.

### Cross-compile for Windows from macOS/Linux

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --all-features --target x86_64-pc-windows-gnu
```

## Run

```bash
# Web server (auto-opens http://127.0.0.1:8000)
./target/release/logviewer web

# CLI processing
./target/release/logviewer process view_core.json examples/CORE.OUT
```

## Tests

```bash
cargo test
```

## File Structure

```
src/
├── cli.rs         # CLI entry point
├── filters.rs     # View/pattern engine
├── lib.rs         # Core types
├── process.rs     # Filter pipeline
├── readers.rs     # File readers
├── tests.rs       # Unit tests
└── web.rs         # HTTP server + frontend
view_core.json     # CORE.OUT regex view
view_reu.json      # reu.out regex view
Cargo.toml         # Rust project config
AGENTS.md          # Dev notes
```
