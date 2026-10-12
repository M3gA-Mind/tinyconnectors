param(
    [Parameter(Mandatory = $true)][string]$Image,
    [Parameter(Mandatory = $true)][string]$Crate,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$HostKey,
    [string]$ExtraFilesJson = '[]',
    [string]$OutputDir = 'release-assets'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root

$wrapper = 'scripts/test-distro.sh'
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
if ($IsWindows) {
    $git = (Get-Command git -ErrorAction Stop).Source
    $bash = & (Join-Path $root 'scripts/resolve-git-bash.ps1') -GitPath $git
} else {
    $bash = (Get-Command bash -ErrorAction Stop).Source
}
& $bash @arguments
exit $LASTEXITCODE
