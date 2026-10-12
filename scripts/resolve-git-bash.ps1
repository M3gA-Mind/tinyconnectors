param(
    [Parameter(Mandatory = $true)][string]$GitPath
)

$gitDirectory = Split-Path -Parent $GitPath
while ($gitDirectory) {
    $bash = Join-Path $gitDirectory 'bin/bash.exe'
    $gitCommand = Join-Path $gitDirectory 'cmd/git.exe'
    if ((Test-Path -LiteralPath $bash -PathType Leaf) -and
        (Test-Path -LiteralPath $gitCommand -PathType Leaf)) {
        Write-Output $bash
        exit 0
    }
    $parent = Split-Path -Parent $gitDirectory
    if (-not $parent -or $parent -eq $gitDirectory) {
        break
    }
    $gitDirectory = $parent
}

throw "Git Bash was not found in the installation containing git: $GitPath"
