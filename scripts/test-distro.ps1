param(
    [Parameter(Mandatory = $true)][string]$Image,
    [Parameter(Mandatory = $true)][string]$Crate,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$HostKey,
    [string]$ExtraFilesJson = '[]',
    [string]$OutputDir = 'release-assets',
    [string]$BashExecutable = 'bash',
    [string]$BashScript = ''
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

$wrapper = Join-Path $root 'scripts/test-distro.sh'
$arguments = @(
    $wrapper,
    $Image,
    '--crate', $Crate,
    '--version', $Version,
    '--target', $Target,
    '--host', $HostKey,
    '--extra-files-json', $ExtraFilesJson,
    '--output-dir', $OutputDir
)
if ($BashScript) {
    & $BashExecutable -NoProfile -File $BashScript @arguments
} else {
    & $BashExecutable @arguments
}
exit $LASTEXITCODE
