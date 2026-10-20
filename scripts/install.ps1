# Install aster-cli v0.1.1 on Windows from this repository's GitHub Release.
# One command:
#   irm "https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.ps1" -OutFile "$env:TEMP\install.ps1"
#   powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\install.ps1" -Prefix "$env:LOCALAPPDATA\Aster\bin"
# Downloads aster-cli-v0.1.1-windows.zip, which contains aster.exe and a3s-code-acp.exe.
# Does not clone this repository and does not compile.
param(
    [string]$Prefix = ""
)

$ErrorActionPreference = "Stop"
$OfficialBase = "https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v0.1.1"
$AssetName = "aster-cli-v0.1.1-windows.zip"
$OfficialAsset = "$OfficialBase/$AssetName"

function Refuse-Download {
    Write-Error "this installer does not download that binary"
    exit 1
}

if ($env:OS -ne "Windows_NT") {
    Write-Error "This branch is for Windows. On macOS and Linux run https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh"
    exit 1
}
if ($env:ASTER_INSTALL_URL -or $env:GROK_INSTALL_URL) {
    Refuse-Download
}
if ($Prefix -match "://") {
    Refuse-Download
}
foreach ($arg in $args) {
    $text = [string]$arg
    if ($text -notmatch "://") { continue }
    if ($text -eq $OfficialAsset) { continue }
    Refuse-Download
}

$base = if ($env:ASTER_RELEASE_BASE) { $env:ASTER_RELEASE_BASE } else { $OfficialBase }
if ($base -match "x\.ai") {
    Refuse-Download
}
$localAsset = $null
if ($base -eq $OfficialBase) {
    $assetUrl = "$base/$AssetName"
} elseif (($base -notmatch "://") -and (Test-Path -LiteralPath $base -PathType Container)) {
    $assetUrl = Join-Path $base $AssetName
    $localAsset = $assetUrl
} else {
    Refuse-Download
}

if (-not $Prefix) {
    if ($env:LOCALAPPDATA) {
        $Prefix = Join-Path $env:LOCALAPPDATA "Aster\bin"
    } else {
        $Prefix = Join-Path $env:USERPROFILE ".local\bin"
    }
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("aster-cli-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $work | Out-Null
try {
    $archive = Join-Path $work $AssetName
    Write-Output "platform=windows"
    Write-Output "release=aster-cli-v0.1.1"
    Write-Output "asset=$assetUrl"
    if ($localAsset) {
        Copy-Item -LiteralPath $localAsset -Destination $archive -Force
    } else {
        Invoke-WebRequest -Uri $assetUrl -OutFile $archive
    }
    $unpack = Join-Path $work "unpack"
    Expand-Archive -LiteralPath $archive -DestinationPath $unpack -Force
    $srcAster = Join-Path $unpack "aster.exe"
    $srcAcp = Join-Path $unpack "a3s-code-acp.exe"
    if (-not (Test-Path -LiteralPath $srcAster) -or -not (Test-Path -LiteralPath $srcAcp)) {
        Write-Error "archive $AssetName did not contain both aster-cli binaries"
        exit 1
    }
    $identityPath = Join-Path $unpack "BUILD_ID"
    $checksumPath = Join-Path $unpack "SHA256SUMS"
    if (-not (Test-Path -LiteralPath $identityPath) -or -not (Test-Path -LiteralPath $checksumPath)) {
        throw "release is missing paired build identity or checksums"
    }
    $expectedSums = @("$((Get-FileHash -LiteralPath $srcAster -Algorithm SHA256).Hash.ToLowerInvariant())  aster.exe",
        "$((Get-FileHash -LiteralPath $srcAcp -Algorithm SHA256).Hash.ToLowerInvariant())  a3s-code-acp.exe") -join "`n"
    if ((Get-Content -LiteralPath $checksumPath -Raw).Trim() -cne $expectedSums) {
        throw "paired binary checksum mismatch"
    }
    $identity = (Get-Content -LiteralPath $identityPath -Raw).Trim()
    $asterIdentity = & $srcAster --build-id
    if ($LASTEXITCODE -ne 0) { throw "cannot read Aster source identity" }
    $acpIdentity = & $srcAcp --build-id
    if ($LASTEXITCODE -ne 0 -or -not $identity.StartsWith("aster-cli-pair-v1:") -or
        $identity -cne $asterIdentity -or $identity -cne $acpIdentity) {
        throw "paired binary source identity mismatch"
    }
    New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
    $lock = Join-Path $Prefix ".aster-install.lock"
    New-Item -ItemType Directory -Path $lock -ErrorAction Stop | Out-Null
    $stage = Join-Path $Prefix (".aster-install." + [guid]::NewGuid().ToString("n"))
    $promoted = @()
    $installed = $false
    try {
        New-Item -ItemType Directory -Path $stage | Out-Null
        # shortcut: two renames cannot survive every power loss; use one version-directory pointer when crash-atomic upgrades are required.
        foreach ($name in @("aster.exe", "a3s-code-acp.exe")) {
            if (Test-Path -LiteralPath (Join-Path $Prefix $name) -PathType Container) {
                throw "binary destination is a directory"
            }
            Copy-Item -LiteralPath (Join-Path $unpack $name) -Destination (Join-Path $stage "new-$name")
            $destination = Join-Path $Prefix $name
            if (Test-Path -LiteralPath $destination) {
                Copy-Item -LiteralPath $destination -Destination (Join-Path $stage "old-$name")
            }
        }
        foreach ($name in @("aster.exe", "a3s-code-acp.exe")) {
            Move-Item -LiteralPath (Join-Path $stage "new-$name") -Destination (Join-Path $Prefix $name) -Force
            $promoted += $name
        }
        $installed = $true
    } finally {
        $recovered = $true
        if (-not $installed) {
            foreach ($name in $promoted) {
                try {
                    $backup = Join-Path $stage "old-$name"
                    if (Test-Path -LiteralPath $backup) {
                        Move-Item -LiteralPath $backup -Destination (Join-Path $Prefix $name) -Force
                    } else {
                        Remove-Item -LiteralPath (Join-Path $Prefix $name) -Force
                    }
                } catch {
                    $recovered = $false
                    Write-Warning "Pair recovery needs attention; backup retained at $stage : $_"
                }
            }
        }
        if ($recovered) {
            Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath $lock -Force
        }
    }
    $destAster = Join-Path $Prefix "aster.exe"
    $destAcp = Join-Path $Prefix "a3s-code-acp.exe"
    Write-Output "installed Aster v0.1.1 into $Prefix"
    Write-Output "aster=$destAster"
    Write-Output "a3s-code-acp=$destAcp"
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
