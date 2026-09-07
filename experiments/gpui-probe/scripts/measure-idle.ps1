param([Parameter(Mandatory)][string]$Exe, [int]$Seconds = 120, [string]$Output = "$PSScriptRoot/../artifacts/idle.csv")
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 1) { throw 'Seconds must be positive' }
$resolvedExe = (Resolve-Path -LiteralPath $Exe).Path
$rootProcess = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $resolvedExe } | Select-Object -First 1
if (!$rootProcess) { throw 'Start the release executable and leave it idle for 60 seconds before measuring.' }
New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
$sampleClock = [System.Diagnostics.Stopwatch]::StartNew()
$nextSampleMs = 0
$rows = while ($sampleClock.Elapsed.TotalSeconds -lt $Seconds) {
    $all = @(Get-CimInstance Win32_Process)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add([int]$rootProcess.ProcessId)
    do {
        $changed = $false
        foreach ($item in $all) {
            if ($ids.Contains([int]$item.ParentProcessId) -and $ids.Add([int]$item.ProcessId)) { $changed = $true }
        }
    } while ($changed)
    $samples = @(foreach ($processId in $ids) { Get-Process -Id $processId -ErrorAction SilentlyContinue })
    if (!$samples) { throw 'Measured process exited' }
    [pscustomobject]@{
        utc = [DateTime]::UtcNow.ToString('o')
        process_count = $samples.Count
        cpu_seconds = ($samples | Measure-Object CPU -Sum).Sum
        private_bytes = ($samples | Measure-Object PrivateMemorySize64 -Sum).Sum
        working_set_bytes = ($samples | Measure-Object WorkingSet64 -Sum).Sum
        handles = ($samples | Measure-Object HandleCount -Sum).Sum
    }
    $nextSampleMs += 1000
    $delayMs = [Math]::Min($Seconds * 1000, $nextSampleMs) - $sampleClock.ElapsedMilliseconds
    if ($delayMs -gt 0) { Start-Sleep -Milliseconds ([int]$delayMs) }
}
$rows | Export-Csv -NoTypeInformation -Encoding utf8 $Output
# Raw cumulative CPU can decrease if children exit; reject such runs for CPU comparisons.
Write-Output "Recorded $($rows.Count) process-tree samples: $Output"
