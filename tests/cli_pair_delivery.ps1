# Native Windows regression; uses the built pair only in a temporary prefix.
param([Parameter(Mandatory=$true)][string]$Archive)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path ([IO.Path]::GetTempPath()) ("aster-pair-test-" + [guid]::NewGuid().ToString("n"))
$prefix = Join-Path $fixture "prefix"
$assets = Join-Path $fixture "assets"
$oldBase = $env:ASTER_RELEASE_BASE
$oldOS = $env:OS
try {
    New-Item -ItemType Directory -Path $prefix, $assets -Force | Out-Null
    Copy-Item -LiteralPath $Archive -Destination (Join-Path $assets "aster-cli-v0.1.1-windows.zip")
    $env:ASTER_RELEASE_BASE = $assets
    $env:OS = "Windows_NT"
    foreach ($name in @("aster.exe", "a3s-code-acp.exe")) {
        [IO.File]::WriteAllText((Join-Path $prefix $name), "old-$name")
    }
    # Failure after Aster is promoted must restore the original pair.
    function Move-Item {
        param([string]$LiteralPath, [string]$Destination, [switch]$Force)
        if ([IO.Path]::GetFileName($LiteralPath) -eq "new-a3s-code-acp.exe") {
            throw "fixture second promotion failure"
        }
        Microsoft.PowerShell.Management\Move-Item -LiteralPath $LiteralPath -Destination $Destination -Force:$Force
    }
    $refused = $false
    try { & (Join-Path $root "scripts/install.ps1") -Prefix $prefix }
    catch { $refused = $true }
    Remove-Item Function:\Move-Item
    if (-not $refused) { throw "fault did not refuse installation" }
    foreach ($name in @("aster.exe", "a3s-code-acp.exe")) {
        if ([IO.File]::ReadAllText((Join-Path $prefix $name)) -cne "old-$name") {
            throw "pair rollback did not preserve $name"
        }
    }
    if (Test-Path (Join-Path $prefix ".aster-install.lock")) { throw "installation lock leaked" }
    & (Join-Path $root "scripts/install.ps1") -Prefix $prefix
    $first = & (Join-Path $prefix "aster.exe") --build-id
    $second = & (Join-Path $prefix "a3s-code-acp.exe") --build-id
    if ($first -cne $second) { throw "installed source identities differ" }
    Write-Output "2 native pair delivery checks passed: rollback and successful installation"
} finally {
    $env:ASTER_RELEASE_BASE = $oldBase
    $env:OS = $oldOS
    Remove-Item -LiteralPath $fixture -Recurse -Force -ErrorAction SilentlyContinue
}
