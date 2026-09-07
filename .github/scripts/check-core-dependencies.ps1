param([string]$Target = $(if ($IsMacOS) { 'aarch64-apple-darwin' } else { 'x86_64-pc-windows-msvc' }))
$ErrorActionPreference = 'Stop'
$manifest = (Resolve-Path "$PSScriptRoot/../../Cargo.toml").Path
$packageArgs = @('rotor-common', 'rotor-platform', 'rotor-runtime', 'rotor-searcher', 'rotor-screenshot', 'rotor-translator') |
    ForEach-Object { '-p'; $_ }
$tree = & cargo tree --manifest-path $manifest @packageArgs --target $Target --locked --edges normal,build --prefix none
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve shared core dependency graph' }
$forbidden = @($tree | Where-Object { $_ -match '^(tauri|tauri-runtime(?:-\S+)?|tauri-plugin-\S+|gpui(?:-\S+)?|wry|webview2-com|rquickjs(?:-\S+)?) v' })
if ($forbidden.Count) { throw "UI runtime leaked into core: $($forbidden -join ', ')" }
Write-Output "Shared core dependency boundary passed: $Target"
