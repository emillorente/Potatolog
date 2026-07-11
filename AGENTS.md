## Goal
- Build a web-based log viewer (Rust + JavaScript) that auto-detects structured log formats (CORE.OUT ~-delimited and reu.out SQL trace), displays all original columns, and provides per-column filters with pagination.

## Constraints & Preferences
- 11 original system columns: VM, Objeto, Procedimiento, Usuario, Hoa (Fecha), Thr, Texto, Fuente, Línea, Fuente C, Línea C.
- Columns resizable via drag handles, widths persist in localStorage.
- Dark + light theme toggle (icons show target: ☀ in dark, ☾ in light), persisted in localStorage.
- Filters: text inputs for most columns, multi-select dropdown for Usuario, date range pickers (from/to) for Fecha. All applied server-side.
- Column filters combine with AND. Pagination with Prev/Next, 2000 records per page, server-side.
- Server returns only a page of records, not the full file.
- User filter dropdown must be left-aligned and full column width.
- Must auto-detect CORE.OUT format (tilde-delimited) and reu.out format (SQL trace with CONTEXT headers).
- Must auto-select the correct view file based on filename (view_core.json for .OUT, view_reu.json for reu).
- No pre-loaded files — app starts empty, only loads via "Open file…" upload button.
- Trigger toggle (`☐ triggers`) in toolbar: unchecked → triggers ocultos, checked → triggers visibles. Persiste al cambiar de archivo.
- XML detection in CORE.OUT messages: if text starts with `<` and contains `>` and either `</` or `/>`, format as XML in modal with syntax highlighting.
- SQL operation detection (any file): first keyword (SELECT, INSERT, etc.) detected, colored bar shown in Texto column, border + colored keyword in modal. Botón "📋 Copy" en modal para copiar texto formateado.

## Security & Reliability Fixes

### 🔴 Path traversal (query.rs:85-92)
- `file` param en `/api/query` se validaba contra cualquier ruta del sistema.
- **Fix**: solo se permiten archivos dentro de `{temp_dir}/logviewer/` o el `default_file` del CLI.

### 🔴 Race condition en fetchPage (index.html + api.js)
- Clics rápidos en Prev/Next podían mostrar datos de página equivocada (respuestas fuera de orden).
- **Fix**: `AbortController` cancela cualquier query previa antes de lanzar una nueva.

### 🔴 LogQueryReader GO handling (readers.rs)
- `"GO"` en mayúscula no se detectaba como terminador; líneas `/***` se trataban como headers.
- **Fix**: `eq_ignore_ascii_case("go")`, skip de líneas `/***`, skip condicional de trailing.

### 🟠 no_active_filters optimization (query.rs)
- El fast path nunca se activaba por defecto porque `f_show_triggers=false` bloqueaba `no_active_filters`.
- **Fix**: separado trigger filter del fast path. Solo-trigger usa un scan ligero sin pre-calcular lowercases.

### 🟠 SQL classification (query.rs)
- `MERGE` clasificado como `"insert"`, `BEGIN` clasificado como `"create"`.
- **Fix**: clases separadas `"merge"` y `"begin"` con sus propios colers en frontend.

### 🟠 Tests (tests/fixtures/)
- 6 tests de readers fallaban por falta de fixtures.
- **Fix**: creados `sample_core.out`, `sample_reu.out`, `sample_reu_detect.out`, `sample_plain.log`. **14/14 tests pasan**.

### 🟡 CSP (tauri.conf.json)
- CSP deshabilitado (`null`).
- **Fix**: `default-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:`

## Done

### UI / Frontend
- Multi-select dropdown para Usuario (checkboxes, full-width button, fixed positioning para evitar clipping).
- Date range pickers (`datetime-local`) para Fecha (Desde / Hasta), server-side.
- Dark/light theme toggle (☀ en dark, ☾ en light) con CSS variables y localStorage.
- `formatSql()` / `formatSqlHtml()` para SQL syntax highlighting con saltos de línea.
- `formatXml()` / `formatXmlHtml()` para XML pretty-printing (tags, attributes, values, PI, inline elements).
- Modal click-to-expand para Texto column: SQL/XML/plain text, título dinámico, cierre con ×/Escape/click-outside.
- Botón "📋 Copy" en modal (copia texto formateado al portapapeles, feedback "✓ Copied" 2s).
- `updateColVisibility()`: oculta columnas cuyo campo está vacío en la página actual.
- SQL op detection con barra coloreada (`.op-bar`) en Texto column + keyword coloreada en modal.
- Trigger checkbox (`☐ triggers`): unchecked → triggers ocultos, checked → visibles. Persiste entre archivos.
- Botón "Open file…" que activa `<input type="file">` oculto via JS.
- App starts empty, no pre-loaded files, no file-picker dropdown.
- Loading spinner (anillo centrado, animado) durante upload y queries.
- Old data se limpia inmediatamente al empezar un nuevo upload.
- Event delegation: un solo listener en `container` para clicks/input/change, sin listeners por fila.
- Solo rebuild tbody: cuando la tabla existe, solo reemplaza `tbody.innerHTML`, no destruye/crea la tabla.
- `renderRows` con array pre-dimensionado + `join('')` en vez de concatenación de strings.
- `esc()` con regex (`replace(/&/g,'&amp;')...`) en vez de crear/leer elementos DOM.
- Debounce 400ms en filtros de columna (evita request por tecla).
- `container.scrollTop = 0` al renderizar (evita espacio en blanco post-búsqueda).
- Sin "Search all" global (eliminado por problemas de scroll y UI).

### Backend / Rust
- `LogCoreReader`: state machine que lee registros delimitados por `~@_~` (soporta XML multi-línea).
- `LogQueryReader`: mergea header CONTEXT + SQL text en un solo registro.
- `view_reu.json`: regex para campos CONTEXT (timing, level, component, proc, timestamp, source, line) + SQL query.
- `detect_reader()`: lee primeros 512 bytes y retorna `LogCoreReader` (si `~@_~`), `LogQueryReader` (si empieza con `/***`), o `LogFile` (line-by-line).
- `process()` con `Box<dyn LogReader>` para dispatch dinámico.
- View auto-detection por filename (`view_for_file()`).
- Upload handler: solo `{ path }` (sin conteo); cleanup tras 3600s.
- Query handler: filtros server-side (case-insensitive substring), date range (ISO datetime), trigger filter.
- `date_str_to_iso()`: convierte `dd/MM/yy` y `yy/MM/dd` a ISO 8601.
- Date filter con `datetime-local`; `dateTo` normalizado a `:59`.
- Cache: `Arc<RwLock<HashMap<String, Arc<CachedDataSet>>>>`, primer query carga archivo a RAM, queries siguientes usan `Arc` + rayon parallel filtering (<30ms).
- `CachedDataSet`: records + ISO timestamps pre-computados; empty-timestamp records al final.
- `record_to_json()`: single-pass field extraction en vez de 11 llamadas `rec.get()`.
- `detect_op_class()`: SQL op por primer keyword (SELECT, INSERT, etc.) server-side, sin allocs.
- JSON response: `{ r: [{ v: {fields...}, c: color, op: opClass, tr: isTrigger }], t: total }`.

### Optimizations (Fase 1 — I/O + CPU)
- **Batch I/O en LogCoreReader**: `fill_buf()`/`consume()` + `String::with_capacity(512)` — elimina ~156M llamadas `read()` byte-a-byte (`readers.rs:81-117`).
- **Filtros sin heap allocations**: pre-cálculo lowercase fuera del loop + `to_ascii_lowercase()` + `eq_ignore_ascii_case()` en bytes — elimina ~7.6M `to_lowercase()` allocs por query (`query.rs:166-174, 247-270`).
- **`evaluate()` con `Cow<str>`**: evita clonar `Record.text` en `If`/`Match`/`Set` (`process.rs:29-41`).
- **`match_string()` retorna `Vec<(String,String)>`**: elimina HashMap (~382K allocs menos) (`filters.rs:120-136`).
- **`spawn_blocking`**: `handle_query` en thread pool separado, no bloquea tokio (`web.rs:110-118`).

### Optimizations (Fase 2 — Memoria + Frontend)
- **Eliminado `search_texts` (~248 MB)**: `rec.text.to_ascii_lowercase()` computado on-demand durante búsqueda (`query.rs:334-337`).
- **Lazy `text_lower`**: eliminado campo `text_lower` de `Record` (~157 MB ahorrados) (`lib.rs:32-36`).
- **Single-pass field extraction**: `record_matches` extrae campos en un loop O(12) en vez de 11 `rec.get()` O(12) (`query.rs:266-282`).
- **Binary search para date break**: O(log n) en vez de O(n) (`query.rs:145-158`).
- **Carga fuera del write lock**: `load_records()` sin lock; solo `insert()` adquiere write lock (`query.rs:368-381`).
- **`detect_op_class()` sin alloc**: `eq_ignore_ascii_case()` en slices de bytes (`query.rs:468-491`).
- **`matches_user_filter()` sin alloc**: comparación byte a byte con `eq_ignore_ascii_case()` (`query.rs:462-466`).
- **`date_str_to_iso()` sin Vec**: parseo directo con `find()` + `split()` (`query.rs:490-511`).
- **Frontend**: `esc()` con regex, array+join en `renderRows`, solo rebuild tbody, event delegation, debounce 400ms.

### Dependency upgrades
- `tokio` 0.2 → 1.x, `warp` 0.2 → 0.4, `bytes` 0.5 → 1, `clap` 2.33 → 4.
- Eliminados `wry`/`tao` del root crate (solo Tauri los usa).
- Creado workspace `Cargo.toml` raíz con `members = ["src-tauri"]`.
- `src-tauri/Cargo.toml`: Tauri 2.11, tauri-build 2.6.

### Code Quality
- `#![warn(clippy::all)]` en lib.rs.
- Edition 2018 → 2021.
- Reader fields (`file`, `pos`) privados.
- `detect_reader` propaga errores con `?`.
- Renamed `next_triable` → `try_next`.
- `&Vec<Operation>` → `&[Operation]`.
- `Display` impl para `Expression`.
- Eliminado `desktop` feature + `src/desktop.rs` (código muerto, Tauri lo reemplaza).
- Eliminados: `base_dir`, `QueryResult`, `matchesColumnFilters()`, `dateToIso()`, `loadRecords()`.

## Key Decisions
- `position: fixed` + JS-calculated coordinates para user dropdown (evita clipping del scrollable container).
- `Box<dyn LogReader>` para dispatch dinámico de readers.
- Reader auto-detection por content signature (primeros 512 bytes), no por filename.
- View auto-detection por filename pattern ("reu" → view_reu.json, ".OUT" → view_core.json).
- Date filter por comparación ISO (conversión de `dd/MM/yy` a `yyyy-MM-dd`).
- Theme toggle con `data-theme` en `<html>` + CSS variables; iconos representan el target.
- Upload retorna solo path (sin total) para evitar doble scan.
- Todo filtering server-side; frontend envía params a `/api/query`.
- Paginación server-side: skip/limit, 2000 records por página.
- Event delegation en `container` para `input`, `click`, `change` (no se pierden al recrear tabla).
- `pageSkip = 0` al cambiar filtros (evita resultados vacíos en páginas > 1).
- Trigger toggle con listener directo (está fuera de `#log-container`).
- Sin "Search all" global (eliminado por problemas de UI/scroll).

## Relevant Files
- `src/web.rs`: server routes + embedded frontend (warp), `handle_query` con `spawn_blocking`, `handle_upload`.
- `src/readers.rs`: `LogFile` (line-by-line), `LogQueryReader` (reu multi-line merger), `LogCoreReader` (tilde-delimited, batch I/O), `detect_reader()`.
- `src/process.rs`: `process()`, `FilteredLogIterator`, `evaluate()` con `Cow<str>`.
- `src/cli.rs`: CLI entry (process, web subcommands), clap 4.
- `src/filters.rs`: regex patterns, View/Operation/Condition types, `match_string()` retorna Vec.
- `src/query.rs`: query engine, `ColumnStore`-like access, `record_matches` single-pass, `record_to_json`, `cached_records`, `date_str_to_iso`, `detect_op_class`.
- `src/lib.rs`: `Record`, `Color` types, `text_lower` eliminado.
- `Cargo.toml`: features (cli, web, json), edition 2021, workspace con members = ["src-tauri"].
- `src-tauri/Cargo.toml`: Tauri 2.11, tokio 1.
- `src-tauri/tauri.conf.json`: window 1400×900, URL http://127.0.0.1:8731, bundle iconos.
- `src-tauri/src/lib.rs`: `run()`: tokio runtime + warp server + Tauri window.
- `view_core.json`: regex CORE.OUT (15 `~`-delimited fields, 11 captured).
- `view_reu.json`: regex reu.out (CONTEXT fields + SQL).
- `examples/CORE.OUT`: ~382K-record (156 MB).
- `examples/reu.out`: ~3.8K-query (13 MB).
- `AGENTS.md`: this file.

## Build & Run

### Prerequisites
- [Rust](https://rustup.rs/) (stable toolchain, MSRV 1.77.2+)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (macOS: Xcode CLI tools)
- Git

> **Importante**: hay **dos crates** en un workspace. Siempre compilar desde la raíz del proyecto (`/Users/emi/Code/LogViewer`), **no** desde `src-tauri/`. El target dir compartido es `target/`.

### 1. Desktop app — bundle completo (.app + .dmg)

```bash
cd src-tauri && cargo tauri build --bundles "app,dmg"

# Output:
#   target/release/bundle/macos/LogViewer.app     ← se conserva
#   target/release/bundle/dmg/LogViewer_0.1.0_aarch64.dmg
```

### 2. Desktop app — solo binario (sin bundle)

```bash
cargo build --release -p logviewer-desktop
# Binary: target/release/logviewer-desktop
```

### 3. CLI + web server (sin GUI)

```bash
cargo build --release
# Binary: target/release/logviewer
```

### 4. Tests

```bash
cargo test
# 8 tests relevantes pasan; 6 tests de readers fallan por fixtures faltantes (pre-existente)
```

### 5. Run

```bash
# Web server (http://127.0.0.1:8000, upload file via UI)
./target/release/logviewer web

# CLI: procesar log con una vista (JSON lines a stdout)
./target/release/logviewer process view_core.json examples/CORE.OUT
```

### Cross-compile para Windows

```bash
# Mingw
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu

# MSVC (requiere Visual Studio linker)
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

### Windows icon

Tauri usa `src-tauri/icons/icon.ico`. Reemplazar ese archivo para personalizar el icono.
Para regenerar todos los iconos desde `assets/logo.png`:
```bash
magick convert assets/logo.png -resize 32x32 -alpha on -define png:color-type=6 src-tauri/icons/32x32.png
magick convert assets/logo.png -resize 128x128 -alpha on -define png:color-type=6 src-tauri/icons/128x128.png
magick convert assets/logo.png -resize 256x256 -alpha on -define png:color-type=6 src-tauri/icons/128x128@2x.png
magick convert assets/logo.png \( -clone 0 -resize 16x16 \) \( -clone 0 -resize 32x32 \) \( -clone 0 -resize 48x48 \) \( -clone 0 -resize 64x64 \) \( -clone 0 -resize 128x128 \) \( -clone 0 -resize 256x256 \) -delete 0 src-tauri/icons/icon.ico
```
