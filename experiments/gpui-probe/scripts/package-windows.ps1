param([string]$Makensis, [string]$Target = 'x86_64-pc-windows-msvc')
$ErrorActionPreference = 'Stop'
if (!$Makensis) {
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'),
        (Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe')
    )
    $Makensis = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (!$Makensis) { throw 'NSIS compiler unavailable; provide -Makensis with its absolute path.' }
$probe = (Resolve-Path "$PSScriptRoot/..").Path
& cargo +1.97.0 build --manifest-path "$probe/Cargo.toml" --release --locked --target $Target
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
$exe = "$probe/target/$Target/release/rotor-gpui-probe.exe"
New-Item -ItemType Directory -Force "$probe/artifacts" | Out-Null
$installer = "$probe/artifacts/Rotor-GPUI-P0-setup.exe"
& $Makensis "/DPROBE_EXE=$exe" "/DOUTPUT_FILE=$installer" "$probe/packaging/windows.nsi"
if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed' }
Get-FileHash -LiteralPath $installer | Select-Object Algorithm,Hash |
    ConvertTo-Json | Set-Content "$probe/artifacts/installer-hash.json"
Write-Output "Unsigned isolated installer: $installer"
