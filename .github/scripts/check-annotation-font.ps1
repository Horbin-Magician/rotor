$ErrorActionPreference = 'Stop'
$assetRoot = (Resolve-Path "$PSScriptRoot/../../src-tauri/assets/fonts").Path
$fontManifest = Get-Content -LiteralPath (Join-Path $assetRoot 'source.json') -Raw | ConvertFrom-Json
foreach ($entry in $fontManifest.files) {
    if ([System.IO.Path]::GetFileName($entry.file) -ne $entry.file -or $entry.file -in @('.', '..')) {
        throw 'Font manifest entries must be local filenames'
    }
    $assetPath = Join-Path $assetRoot $entry.file
    if ((Get-Item -LiteralPath $assetPath).Length -ne $entry.bytes) { throw "Font resource length mismatch: $($entry.file)" }
    $digest = (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($digest -ne $entry.sha256) { throw "Font resource checksum mismatch: $($entry.file)" }
}
Write-Output 'Annotation font and license checksums passed'
