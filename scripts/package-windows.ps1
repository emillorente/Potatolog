# Package LogViewer as a standalone Windows desktop app (Tauri + WebView2)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$env:CARGO_TARGET_DIR = Join-Path $Root "target"
$Dist = Join-Path $Root "dist\LogViewer"
$ZipPath = Join-Path $Root "dist\LogViewer-win64.zip"
$Release = Join-Path $Root "target\release\logviewer-desktop.exe"

Write-Host "Building Tauri desktop release..."
Push-Location $Root
cargo build --release -p logviewer-desktop
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
Pop-Location

if (-not (Test-Path $Release)) {
    Write-Error "Release binary not found at $Release"
}

New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Get-ChildItem $Dist -Force | Remove-Item -Recurse -Force
Copy-Item $Release (Join-Path $Dist "LogViewer.exe") -Force

# Shortcut with icon (double-click to launch, no console)
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut((Join-Path $Dist "LogViewer.lnk"))
$Shortcut.TargetPath = Join-Path $Dist "LogViewer.exe"
$Shortcut.WorkingDirectory = $Dist
$Shortcut.IconLocation = Join-Path $Dist "LogViewer.exe,0"
$Shortcut.Description = "LogViewer - Visor de logs"
$Shortcut.Save()

@"
LogViewer - Aplicacion de escritorio para Windows
==================================================

INICIO RAPIDO
  Doble clic en "LogViewer.lnk" o "LogViewer.exe".
  Se abre la ventana de la aplicacion (WebView2).

USO
  1. Pulsa "Open file..." para cargar un log (CORE.OUT o reu.out)
  2. Filtra por columnas, fechas o busqueda global
  3. Haz clic en Texto para ver SQL/XML completo

CONTENIDO
  LogViewer.exe    Aplicacion de escritorio (vistas embebidas)
  LogViewer.lnk    Acceso directo recomendado
  logviewer.log    Log de arranque (se crea al ejecutar)

REQUISITOS
  Windows 10 o superior con WebView2 Runtime (incluido en Windows 11;
  en Windows 10 puede requerir instalacion previa de WebView2).

DISTRIBUCION
  Copia toda la carpeta LogViewer o el ZIP LogViewer-win64.zip
  a cualquier ubicacion (USB, red, escritorio).
"@ | Set-Content -Encoding UTF8 (Join-Path $Dist "LEEME.txt")

if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Compress-Archive -Path $Dist -DestinationPath $ZipPath -Force

Write-Host ""
Write-Host "Desktop app packaged:"
Write-Host "  Folder: $Dist"
Write-Host "  ZIP:    $ZipPath"
Get-ChildItem $Dist | Format-Table Name, Length
