param([string]$Target = 'x86_64-pc-windows-msvc')
$ErrorActionPreference = 'Stop'
$tree = & cargo +1.97.0 tree --manifest-path "$PSScriptRoot/../Cargo.toml" --locked --target $Target --edges normal,build --prefix none
if ($LASTEXITCODE -ne 0) { throw 'cargo tree failed' }
$forbidden = @($tree | Where-Object { $_ -match '^(tauri|tauri-runtime(?:-\S+)?|tauri-plugin-\S+|wry|webview2-com|gpui-wry|gpui-shell|gpui-component-shell|rquickjs(?:-\S+)?|quick-js|boa_engine) v' })
if ($forbidden.Count) { throw "Forbidden runtime dependencies: $($forbidden -join ', ')" }
New-Item -ItemType Directory -Force "$PSScriptRoot/../artifacts" | Out-Null
$tree | Set-Content "$PSScriptRoot/../artifacts/dependency-tree-$Target.txt"
Write-Output "Native normal/build dependency closure accepted for $Target"
