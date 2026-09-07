param([string]$Exe = "$PSScriptRoot/../target/release/rotor-gpui-probe.exe")
$ErrorActionPreference = 'Stop'
$out = "$PSScriptRoot/../artifacts"
New-Item -ItemType Directory -Force $out | Out-Null
& $Exe 2>&1 | Tee-Object "$out/runtime.log"
exit $LASTEXITCODE
