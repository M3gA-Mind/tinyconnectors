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
    $hadMsysOption = Test-Path Env:MSYS
    $previousMsysOption = $env:MSYS
    try {
        if ([string]::IsNullOrEmpty($env:MSYS)) {
            $env:MSYS = 'noglob'
        } elseif (($env:MSYS -split '\s+') -notcontains 'noglob') {
            $env:MSYS = "$($env:MSYS) noglob"
        }
        & $bash @arguments
        $exitCode = $LASTEXITCODE
    } finally {
        if ($hadMsysOption) {
            $env:MSYS = $previousMsysOption
        } else {
            Remove-Item Env:MSYS -ErrorAction SilentlyContinue
        }
    }
} else {
    $bash = (Get-Command bash -ErrorAction Stop).Source
    & $bash @arguments
    $exitCode = $LASTEXITCODE
}
exit $exitCode
