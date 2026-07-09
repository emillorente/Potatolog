# Auditoría de dependencias — LogViewer

Última revisión: 2026-07-07

## Toolchain Rust

| Componente | Valor actual (máquina dev) | Recomendado proyecto | Notas |
|------------|---------------------------|----------------------|-------|
| `rustc` | 1.96.0 | **1.90+** (pin en `rust-toolchain.toml`) | Tauri 2 MSRV oficial: **1.77.2** |
| Target Windows | `x86_64-pc-windows-msvc` | Igual | Coherente con WebView2 y MSVC |
| CRT | `crt-static` (`.cargo/config.toml`) | Mantener en release portable actual | Tauri usa linking dinámico por defecto; revisar al migrar |

## Dependencias actuales (`Cargo.toml` / `Cargo.lock`)

| Crate | Manifest | Lockfile | Última crates.io | Estado | Acción recomendada |
|-------|----------|----------|------------------|--------|-------------------|
| `clap` | 2.33 ? **4** | 4.6.1 | 4.6.x | OK | Mantener |
| `tokio` | 1 | 1.52.3 | 1.52.x | OK | Solo feature `web` |
| `warp` | 0.3 | 0.3.7 | **0.4.3** | Desactualizado | **Eliminar** en app desktop; mantener solo dev o migrar a 0.4 |
| `bytes` | 1 | 1.12.0 | 1.12.x | OK | Solo feature `web` |
| `rayon` | 1.5 | 1.12.0 | 1.12.0 | Manifest viejo | Actualizar manifest a `1.12` |
| `regex` | 1.0 | **1.4.2** | **1.12.4** | Muy desactualizado | Actualizar a `1.12` (mejor rendimiento, mismo API) |
| `serde` / `serde_json` | 1.0 | 1.0.228 | 1.0.x | OK | Mantener |
| `embed-resource` | 2 | 2.5.2 | **3.0.11** | Legacy | Sustituir por iconos Tauri en desktop; o subir a 3.x si se mantiene build actual |

## Dependencias nuevas (app desktop Tauri)

| Crate | Versión objetivo | Uso |
|-------|------------------|-----|
| `tauri` | **2.11.5** | Shell desktop, WebView2, IPC |
| `tauri-build` | **2.x** (alineado con tauri) | Build script |
| `tauri-plugin-dialog` | **2.x** | Diálogo nativo "Abrir archivo" |
| `tauri-plugin-opener` | **2.x** (opcional) | Abrir enlaces externos |

### Transitive stack Tauri (referencia, no añadir manualmente)

- `wry` — WebView2 en Windows
- `tao` — ventanas nativas
- `tokio` — runtime interno de Tauri (no duplicar lógica async propia)

## Matriz: qué depende de qué

```
CLI (process)     ? regex, serde, rayon, readers, filters
Web legacy (warp) ? + tokio, warp, bytes  [feature web]
Desktop (Tauri)   ? regex, serde, rayon, query engine  [sin warp/tokio propio]
```

## Prerrequisitos Windows (desarrollo + runtime)

### Desarrollo

- [Rust](https://rustup.rs/) ? 1.77.2 (recomendado 1.90+)
- **Visual Studio Build Tools 2022** con workload "Desktop development with C++"
- **WebView2** (suele venir con Windows 11; en Win10 instalar [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/))
- `cargo install tauri-cli --version "^2.0"` (CLI v2)

### Runtime usuario final (desktop)

- Windows 10+
- WebView2 Runtime (preinstalado en la mayoría de equipos)
- **No** requiere: Rust, Node.js, navegador externo, Visual C++ Redist (si Tauri no usa crt-static)

## Comandos de verificación

```powershell
rustc --version
cargo --version
cargo tree -p logviewer --features web
cargo test --all-features
cargo search tauri --limit 1
```

## Política de actualización sugerida

1. **Antes de Tauri:** extraer `query.rs`, no tocar versiones salvo `regex`/`rayon` (bajo riesgo).
2. **Al añadir Tauri:** fijar `tauri = "2.11"` en `src-tauri/Cargo.toml`.
3. **Tras desktop estable:** retirar `warp`/`tokio`/`bytes` del default; dejar `web` como feature opcional para desarrollo.
4. **No migrar warp 0.4** si se va a eliminar el modo servidor HTTP.
