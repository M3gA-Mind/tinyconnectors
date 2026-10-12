$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

$target = (& rustc -vV | Select-String '^host: ').ToString().Substring(6)
if ($LASTEXITCODE -ne 0) { throw 'Could not determine the Rust host target' }
$metadata = cargo metadata --manifest-path Cargo.toml --format-version 1 --no-deps | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo workspace metadata' }
$version = @($metadata.packages | Where-Object name -eq 'tinyconnectors' | Select-Object -ExpandProperty version)
if ($version.Count -ne 1) { throw 'Cargo metadata must contain exactly one tinyconnectors package' }

$archivePath = Join-Path 'target/module-e2e' "tinyconnectors-$($version[0])-ci.zip"
& pwsh -NoProfile -File scripts/build-module.ps1 `
    -Crate tinyconnectors -Version $version[0] -Target $target -HostKey ci `
    -OutputDir target/module-e2e
if ($LASTEXITCODE -ne 0) { throw 'Failed to build the module E2E archive' }
& pwsh -NoProfile -File scripts/verify-module.ps1 -Archive $archivePath
if ($LASTEXITCODE -ne 0) { throw 'Loaded module E2E verification failed' }
