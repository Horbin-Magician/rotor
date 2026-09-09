param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$TestDirectory,
    [ValidateRange(120, 86400)][int]$SampleSeconds = 120
)
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Windows is required' }
$workspace = (Resolve-Path "$PSScriptRoot/../..").Path
$testRoot = [IO.Path]::GetFullPath($TestDirectory)
if (!$testRoot.StartsWith((Join-Path $workspace 'target') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Test-Path -LiteralPath $testRoot)) { throw 'Use a new workspace target subdirectory' }
$ancestor = Split-Path $testRoot -Parent
while ($ancestor -and $ancestor.StartsWith($workspace, [StringComparison]::OrdinalIgnoreCase)) {
    if ((Test-Path -LiteralPath $ancestor) -and
        ((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Test ancestors must not contain reparse points'
    }
    $ancestor = Split-Path $ancestor -Parent
}
$binary = (Resolve-Path -LiteralPath $Executable).Path
$identity = Get-Content -LiteralPath (Join-Path (Split-Path $binary) 'native-build.json') -Raw | ConvertFrom-Json
if ($identity.production -or $identity.identifier -ne 'cc.fluctus.rotor.gpui-dev') {
    throw 'Only a staged or installed development identity may be sampled'
}
New-Item -ItemType Directory -Path $testRoot | Out-Null
$start = [Diagnostics.ProcessStartInfo]::new($binary)
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.WorkingDirectory = Split-Path $binary
foreach ($argument in @('--background', '--no-elevate', '--no-index', '--no-hotkeys', '--data-dir', (Join-Path $testRoot 'profile'))) {
    $start.ArgumentList.Add($argument)
}
$start.Environment.Remove('ROTOR_DATA_DIR') | Out-Null
$start.Environment.Remove('ROTOR_RESOURCE_DIR') | Out-Null
$process = [Diagnostics.Process]::Start($start)
try {
    for ($second = 0; $second -lt 60; $second++) {
        Start-Sleep -Seconds 1
        if ($process.HasExited) { throw "Process exited during stabilization: $($process.ExitCode)" }
    }
    $process.Refresh()
    $firstCpu = $process.TotalProcessorTime.TotalSeconds
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $rows = [Collections.Generic.List[object]]::new()
    for ($second = 0; $second -lt $SampleSeconds; $second++) {
        Start-Sleep -Seconds 1
        $process.Refresh()
        if ($process.HasExited) { throw "Process exited during sampling: $($process.ExitCode)" }
        $rows.Add([ordered]@{
            seconds = $clock.Elapsed.TotalSeconds
            cpu_seconds = $process.TotalProcessorTime.TotalSeconds - $firstCpu
            private_bytes = $process.PrivateMemorySize64
            working_set_bytes = $process.WorkingSet64
            handles = $process.HandleCount
        })
    }
    $last = $rows[$rows.Count - 1]
    $cpuPercent = 100 * $last.cpu_seconds / $last.seconds
    [ordered]@{
        utc = [DateTime]::UtcNow.ToString('o')
        version = $identity.version
        binary_sha256 = (Get-FileHash -LiteralPath $binary).Hash.ToLowerInvariant()
        os = [Environment]::OSVersion.VersionString
        logical_processors = [Environment]::ProcessorCount
        stabilization_seconds = 60
        single_core_cpu_percent = $cpuPercent
        idle_cpu_budget_passed = $cpuPercent -le 0.5
        scope = 'Development installation; background, no index/hotkeys, empty synthetic profile; main process only'
        limitations = 'No legacy comparison, GPU measurement, interactive workload, cold-start or shutdown acceptance. Only the process created by this script is terminated after sampling.'
        samples = $rows
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $testRoot 'idle.json') -Encoding utf8
    Write-Output "Idle CPU (one logical core): $cpuPercent%; evidence: $testRoot"
} finally {
    if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }
    $process.Dispose()
}
