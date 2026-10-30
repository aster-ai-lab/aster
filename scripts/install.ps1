# Install the aster-cli pair on Windows from this repository's GitHub Releases.
# One command:
#   irm "https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.ps1" -OutFile "$env:TEMP\install.ps1"
#   powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\install.ps1" -Prefix "$env:LOCALAPPDATA\Aster\bin"
# The newest release is discovered from the channel manifest:
#   https://github.com/aster-ai-lab/aster/releases/download/aster-cli-stable/latest.json
# -Version X.Y.Z installs that exact release. The Windows archive contains
# aster.exe and a3s-code-acp.exe. Does not clone and does not compile.
param(
    [string]$Prefix = "",
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$OfficialReleases = "https://github.com/aster-ai-lab/aster/releases"
$ManifestName = "latest.json"
$LatestManifest = "https://github.com/aster-ai-lab/aster/releases/download/aster-cli-stable/latest.json"

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
$directUrl = $null
foreach ($arg in $args) {
    $text = [string]$arg
    if ($text -notmatch "://") { continue }
    if ($text -match "^https://github\.com/aster-ai-lab/aster/releases/download/aster-cli-v[0-9]+\.[0-9]+\.[0-9]+/aster-cli-v[0-9]+\.[0-9]+\.[0-9]+-(macos|linux|windows)\.(tar\.gz|zip)$") {
        $directUrl = $text
        continue
    }
    Refuse-Download
}

$wantVersion = $null
if ($Version) {
    $wantVersion = $Version -replace "^aster-cli-v", "" -replace "^v", ""
    if ($wantVersion -notmatch "^[0-9]+\.[0-9]+\.[0-9]+$") {
        Write-Error "--version wants a release triple such as 0.1.2"
        exit 1
    }
}

$manifest = $null
$localBase = $null
$assetUrl = $null
$assetName = $null
$expectedSha = $null
$releaseVersion = ""
$releaseTag = ""
if ($directUrl) {
    $assetUrl = $directUrl
    $assetName = Split-Path -Leaf $directUrl
} elseif ($env:ASTER_RELEASE_BASE) {
    $base = $env:ASTER_RELEASE_BASE
    if ($base -match "x\.ai" -or $base -match "://") {
        Refuse-Download
    }
    if (-not (Test-Path -LiteralPath $base -PathType Container)) {
        Write-Error "ASTER_RELEASE_BASE is not a local release directory"
        exit 1
    }
    $manifestPath = Join-Path $base $ManifestName
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        Write-Error "local release directory has no $ManifestName"
        exit 1
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $localBase = $base
} else {
    $manifestUrl = if ($wantVersion) {
        "$OfficialReleases/download/aster-cli-v$wantVersion/$ManifestName"
    } else {
        $LatestManifest
    }
    $manifest = Invoke-RestMethod -Uri $manifestUrl
}

if ($manifest) {
    $releaseVersion = [string]$manifest.version
    $releaseTag = [string]$manifest.tag
    if ($releaseVersion -notmatch "^[0-9]+\.[0-9]+\.[0-9]+$" -or $releaseTag -cne "aster-cli-v$releaseVersion") {
        throw "channel manifest is malformed"
    }
    if ($wantVersion -and $releaseVersion -cne $wantVersion) {
        throw "release $releaseTag does not match the requested version $wantVersion"
    }
    $assetName = "aster-cli-v$releaseVersion-windows.zip"
    $entry = $manifest.assets.windows
    if (-not $entry -or [string]$entry.name -cne $assetName -or [string]$entry.sha256 -notmatch "^[0-9a-f]{64}$") {
        throw "channel manifest has no $assetName"
    }
    $expectedSha = [string]$entry.sha256
    if ($localBase) {
        $assetUrl = Join-Path $localBase $assetName
        if (-not (Test-Path -LiteralPath $assetUrl -PathType Leaf)) {
            throw "local release is missing $assetName"
        }
    } else {
        $assetUrl = "$OfficialReleases/download/$releaseTag/$assetName"
    }
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
    $archive = Join-Path $work $assetName
    Write-Output "platform=windows"
    # Windows PowerShell 5.1 parses no ternary operator: keep this file 5.1-clean.
    $releaseLabel = if ($releaseTag) { $releaseTag } else { "direct" }
    Write-Output "release=$releaseLabel"
    Write-Output "asset=$assetUrl"
    if ($localBase) {
        Copy-Item -LiteralPath $assetUrl -Destination $archive -Force
    } else {
        Invoke-WebRequest -Uri $assetUrl -OutFile $archive
    }
    if ($expectedSha) {
        $archiveSha = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($archiveSha -cne $expectedSha) {
            throw "channel checksum mismatch"
        }
    }
    $unpack = Join-Path $work "unpack"
    Expand-Archive -LiteralPath $archive -DestinationPath $unpack -Force
    $srcAster = Join-Path $unpack "aster.exe"
    $srcAcp = Join-Path $unpack "a3s-code-acp.exe"
    if (-not (Test-Path -LiteralPath $srcAster) -or -not (Test-Path -LiteralPath $srcAcp)) {
        Write-Error "archive $assetName did not contain both aster-cli binaries"
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
    if ($releaseVersion) {
        Write-Output "installed Aster v$releaseVersion into $Prefix"
    } else {
        Write-Output "installed Aster into $Prefix"
    }
    Write-Output "aster=$destAster"
    Write-Output "a3s-code-acp=$destAcp"
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
