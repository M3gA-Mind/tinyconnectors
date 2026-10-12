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
$bash = (Get-Command bash -ErrorAction Stop).Source
if ($IsWindows) {
    $git = (Get-Command git -ErrorAction Stop).Source
    $gitDirectory = Split-Path -Parent $git
    if ((Split-Path -Leaf $gitDirectory) -in @('cmd', 'bin')) {
        $gitDirectory = Split-Path -Parent $gitDirectory
    }
    $gitBash = Join-Path $gitDirectory 'bin/bash.exe'
    if (-not (Test-Path -LiteralPath $gitBash -PathType Leaf)) {
        throw "Git Bash was not found next to git: $gitBash"
    }
    $bash = $gitBash
}
& $bash @arguments
exit $LASTEXITCODE
