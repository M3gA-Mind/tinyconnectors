param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

if ($Target -notmatch '^[A-Za-z0-9_-]+$') { throw "Invalid Rust target: $Target" }
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$') { throw "Invalid module version: $Version" }

$archivePath = Join-Path 'target/module-e2e' "tinyconnectors-$Version-ci.zip"
& pwsh -NoProfile -File scripts/build-module.ps1 `
    -Crate tinyconnectors -Version $Version -Target $Target -HostKey ci `
    -OutputDir target/module-e2e
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& pwsh -NoProfile -File scripts/verify-module.ps1 -Archive $archivePath
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
exit 0
