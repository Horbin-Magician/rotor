param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [Parameter(Mandatory)][string]$TestDirectory
)
$ErrorActionPreference = 'Stop'
if (!$IsWindows -or [IntPtr]::Size -ne 8) { throw 'Requires PowerShell 7 on 64-bit Windows' }
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run in an already elevated test session; this script does not request UAC'
}
$workspaceRoot = (Resolve-Path "$PSScriptRoot/../..").Path
$targetRoot = [IO.Path]::GetFullPath((Join-Path $workspaceRoot 'target'))
$testRoot = [IO.Path]::GetFullPath($TestDirectory)
if (!$testRoot.StartsWith($targetRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Test-Path -LiteralPath $testRoot) -or $testRoot -match '["\r\n]') {
    throw 'Use a new test directory beneath the workspace target directory'
}
$ancestor = Split-Path $testRoot -Parent
while ($ancestor -and $ancestor.StartsWith($workspaceRoot, [StringComparison]::OrdinalIgnoreCase)) {
    if ((Test-Path -LiteralPath $ancestor) -and
        ((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Test ancestors must not contain reparse points'
    }
    $ancestor = Split-Path $ancestor -Parent
}
$packageRoot = (Resolve-Path -LiteralPath $PackageDirectory).Path
$identity = Get-Content -LiteralPath (Join-Path $packageRoot 'native-build.json') -Raw | ConvertFrom-Json
if ($identity.production -or $identity.product_name -ne 'Rotor GPUI Development' -or
    $identity.identifier -ne 'cc.fluctus.rotor.gpui-dev') { throw 'Only development identity is allowed' }
$installerName = "Rotor-GPUI_$($identity.version)_x64-setup.exe"
$installer = Join-Path $packageRoot $installerName
$inventory = Get-Content -LiteralPath (Join-Path $packageRoot 'resources.json') -Raw | ConvertFrom-Json -AsHashtable
$expected = $inventory.files[$installerName]
if (!$expected -or (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $expected.sha256 -or
    (Get-Item -LiteralPath $installer).Length -ne $expected.bytes) { throw 'Installer inventory mismatch' }
$productKey = 'HKLM:\Software\RotorGpuiDevelopment'
$uninstallKey = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment'
$shortcut = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'Rotor GPUI Development.lnk'
$runKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$runValue = if (Test-Path $runKeyPath) { (Get-Item -LiteralPath $runKeyPath).GetValue('Rotor GPUI Development', $null) }
if ((Test-Path $productKey) -or (Test-Path $uninstallKey) -or (Test-Path -LiteralPath $shortcut) -or
    $null -ne $runValue) {
    throw 'An existing development installation/startup entry must not be touched by this test'
}
New-Item -ItemType Directory -Path $testRoot | Out-Null
$installRoot = Join-Path $testRoot '安装 Rotor'
$profileRoot = Join-Path $testRoot 'profile'
New-Item -ItemType Directory -Path $profileRoot | Out-Null
$profileFile = Join-Path $profileRoot 'config.toml'
Set-Content -LiteralPath $profileFile -Value 'future_key = "retain synthetic profile"' -Encoding utf8
$profileHash = (Get-FileHash -LiteralPath $profileFile).Hash
$results = [Collections.Generic.List[object]]::new()
function Invoke-CheckedInstaller([string]$File, [string]$Arguments, [bool]$Success, [string]$Case) {
    $logPath = Join-Path $testRoot "$Case.log"
    $Arguments = $Arguments.Replace(' /D=', " /LOG=`"$logPath`" /D=")
    $process = Start-Process -FilePath $File -ArgumentList $Arguments -WindowStyle Hidden -PassThru -Wait
    $results.Add([ordered]@{ case = $Case; exit_code = $process.ExitCode })
    if (($process.ExitCode -eq 0) -ne $Success) {
        if (Test-Path -LiteralPath $logPath) { Get-Content -LiteralPath $logPath | Write-Output }
        throw "$Case returned unexpected exit code $($process.ExitCode)"
    }
}
function Assert-Installed {
    $registered = (Get-ItemProperty -LiteralPath $uninstallKey).InstallLocation
    if ($registered -ne $installRoot) { throw 'Installer did not honor the dedicated test path' }
    if (!(Test-Path -LiteralPath $shortcut)) { throw 'Start menu shortcut is missing' }
    $installedInventory = Get-Content -LiteralPath (Join-Path $installRoot 'resources.json') -Raw | ConvertFrom-Json -AsHashtable
    foreach ($entry in $installedInventory.files.GetEnumerator()) {
        $filePath = [IO.Path]::GetFullPath((Join-Path $installRoot $entry.Key))
        if (!$filePath.StartsWith($installRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
            (Get-FileHash -LiteralPath $filePath).Hash -ne $entry.Value.sha256 -or
            (Get-Item -LiteralPath $filePath).Length -ne $entry.Value.bytes) { throw 'Installed payload inventory mismatch' }
    }
    foreach ($argument in @('--build-info', '--check-resources')) {
        $probe = [Diagnostics.ProcessStartInfo]::new((Join-Path $installRoot 'rotor-desktop.exe'))
        $probe.ArgumentList.Add($argument)
        $probe.WorkingDirectory = $testRoot
        $probe.UseShellExecute = $false
        $probe.CreateNoWindow = $true
        $probe.RedirectStandardOutput = $true
        $probe.RedirectStandardError = $true
        $probe.Environment.Remove('ROTOR_RESOURCE_DIR') | Out-Null
        $probe.Environment.Remove('ROTOR_DATA_DIR') | Out-Null
        $process = [Diagnostics.Process]::Start($probe)
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "Installed diagnostic failed: $stderr" }
        if ($argument -eq '--build-info') {
            $observed = $stdout | ConvertFrom-Json
            if ($observed.production -or $observed.product_name -ne $identity.product_name -or
                $observed.version -ne $identity.version) { throw 'Installed executable identity differs from the package' }
        } elseif (!$stdout.Contains($installRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Installed resource lookup did not select this installation'
        }
        $results.Add([ordered]@{ case = $argument; exit_code = $process.ExitCode; output = $stdout.Trim() })
    }
}
try {
    $unrelated = Join-Path $testRoot 'unrelated'
    New-Item -ItemType Directory -Path $unrelated | Out-Null
    $sentinel = Join-Path $unrelated 'keep.txt'
    Set-Content -LiteralPath $sentinel -Value 'unrelated synthetic file'
    $sentinelHash = (Get-FileHash -LiteralPath $sentinel).Hash
    # NSIS requires /D to be the final unquoted parameter, including spaces.
    Invoke-CheckedInstaller $installer "/S /D=$unrelated" $false 'reject_nonempty_unrelated_directory'
    if ((Get-FileHash -LiteralPath $sentinel).Hash -ne $sentinelHash -or (Test-Path $productKey)) {
        throw 'Rejected installation changed existing data or registration'
    }
    $arguments = "/S /NOELEVATE /NOINDEX /NOHOTKEYS /PROFILE=$profileRoot /D=$installRoot"
    Invoke-CheckedInstaller $installer $arguments $true 'fresh_install'
    Assert-Installed
    $binary = Join-Path $installRoot 'rotor-desktop.exe'
    $binaryHash = (Get-FileHash -LiteralPath $binary).Hash
    $retained = Join-Path $installRoot 'user-added.txt'
    Set-Content -LiteralPath $retained -Value 'retained in upgrade backup'
    $retainedHash = (Get-FileHash -LiteralPath $retained).Hash
    $held = [IO.File]::Open($binary, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try { Invoke-CheckedInstaller $installer $arguments $false 'locked_installation_preserved' }
    finally { $held.Dispose() }
    if ((Get-FileHash -LiteralPath $binary).Hash -ne $binaryHash) { throw 'Failed upgrade changed the old executable' }
    Invoke-CheckedInstaller $installer $arguments $true 'same_version_replacement'
    Assert-Installed
    $backup = (Get-ItemProperty -LiteralPath $productKey).PreviousInstallLocation
    if (!$backup -or !$backup.StartsWith($testRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Upgrade backup escaped the test directory'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $backup 'user-added.txt')).Hash -ne $retainedHash) {
        throw 'Upgrade did not retain user files in its backup'
    }
    Set-Content -LiteralPath $retained -Value 'retain on uninstall'
    $retainedHash = (Get-FileHash -LiteralPath $retained).Hash
    Invoke-CheckedInstaller (Join-Path $installRoot 'uninstall.exe') '/S' $true 'uninstall'
    if ((Test-Path $productKey) -or (Test-Path $uninstallKey) -or (Test-Path -LiteralPath $shortcut) -or
        (Test-Path -LiteralPath $binary)) { throw 'Uninstall left application files or registration' }
    if ((Get-FileHash -LiteralPath $retained).Hash -ne $retainedHash -or
        (Get-FileHash -LiteralPath $profileFile).Hash -ne $profileHash) { throw 'Uninstall changed retained data' }
    $remaining = @(Get-ChildItem -LiteralPath $installRoot -File -Recurse)
    if ($remaining.Count -ne 1 -or $remaining[0].FullName -ne $retained) { throw 'Uninstall left package-owned files' }
    $results.Add([ordered]@{ case = 'retained_data'; profile_unchanged = $true; user_file_retained = $true; backup = $backup })
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $testRoot 'results.json') -Encoding utf8
    Write-Output "Silent install/replacement/uninstall checks passed: $testRoot"
}
finally {
    # Clean only a remaining installation whose registered path is this test's
    # dedicated directory. Never force-delete directories or unrelated entries.
    if ((Test-Path $uninstallKey) -and
        (Get-ItemProperty -LiteralPath $uninstallKey).InstallLocation -eq $installRoot -and
        (Test-Path -LiteralPath (Join-Path $installRoot 'uninstall.exe'))) {
        Start-Process -FilePath (Join-Path $installRoot 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -Wait
    }
}
