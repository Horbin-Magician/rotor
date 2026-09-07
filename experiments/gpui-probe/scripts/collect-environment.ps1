param([string]$Output = "$PSScriptRoot/../artifacts")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Output | Out-Null
$info = [ordered]@{
    utc = [DateTime]::UtcNow.ToString('o')
    os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture
    cpu = Get-CimInstance Win32_Processor | Select-Object Name,NumberOfLogicalProcessors
    gpu = Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution
    rust = (& rustc -Vv) -join "`n"
    cargo = & cargo -V
    baseline_commit = '40addee065765ed343e665d6a712b3e9cbe2bdcc'
    application_commit = & git -C "$PSScriptRoot/../../.." rev-parse HEAD
}
$info | ConvertTo-Json -Depth 6 | Set-Content "$Output/environment.json"
Get-FileHash "$PSScriptRoot/../Cargo.lock" | Select-Object Algorithm,Hash | ConvertTo-Json | Set-Content "$Output/lock-hash.json"
Write-Output 'Record actual screen positions/DPI from the probe window labels and runtime log; GPU display resolution alone is insufficient.'
