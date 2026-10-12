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
    $gitBash = $null
    while ($gitDirectory) {
        $candidate = Join-Path $gitDirectory 'bin/bash.exe'
        $gitCommand = Join-Path $gitDirectory 'cmd/git.exe'
        if ((Test-Path -LiteralPath $candidate -PathType Leaf) -and
            (Test-Path -LiteralPath $gitCommand -PathType Leaf)) {
            $gitBash = $candidate
            break
        }
        $parent = Split-Path -Parent $gitDirectory
        if (-not $parent -or $parent -eq $gitDirectory) {
            break
        }
        $gitDirectory = $parent
    }
    if (-not $gitBash) {
        throw "Git Bash was not found in the installation containing git: $git"
    }
    $bash = $gitBash
}
& $bash @arguments
exit $LASTEXITCODE
