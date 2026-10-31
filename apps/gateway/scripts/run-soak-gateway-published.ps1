# Publish dedicated-hardware capacity envelopes (Enterprise GA).
# Fail closed unless required env pins are set. Default duration is 7200s (2h).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Require-Env([string]$Name) {
  $v = [Environment]::GetEnvironmentVariable($Name)
  if ([string]::IsNullOrWhiteSpace($v)) {
    throw "refuse: set environment variable $Name"
  }
  return $v
}

$dedicated = Require-Env "ASTER_GATEWAY_DEDICATED_RUNNER"
if ($dedicated -ne "1") { throw "refuse: ASTER_GATEWAY_DEDICATED_RUNNER must be 1" }
Require-Env "ASTER_GATEWAY_HW_HOST" | Out-Null
Require-Env "ASTER_GATEWAY_HW_CPU_MODEL" | Out-Null
Require-Env "ASTER_GATEWAY_HW_MEMORY_GB" | Out-Null

$Bin = if ($args.Count -ge 1) { $args[0] } else { ".\target\release\aster-gateway.exe" }
if (-not (Test-Path $Bin)) {
  Write-Host "building release aster-gateway..."
  cargo build --locked --release --bin aster-gateway
  $Bin = ".\target\release\aster-gateway.exe"
}

$Dur = if ($env:ASTER_GATEWAY_SOAK_DURATION) { $env:ASTER_GATEWAY_SOAK_DURATION } else { "7200" }
$C = if ($env:ASTER_GATEWAY_SOAK_CONCURRENCY) { $env:ASTER_GATEWAY_SOAK_CONCURRENCY } else { "16" }

function Invoke-Python {
  param([string[]]$PythonArgs)
  if (Get-Command py -ErrorAction SilentlyContinue) {
    & py -3 @PythonArgs
  } elseif (Get-Command python3 -ErrorAction SilentlyContinue) {
    & python3 @PythonArgs
  } else {
    & python @PythonArgs
  }
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

foreach ($profile in @("http-json", "sse-finite", "openai-json", "openai-sse")) {
  Invoke-Python @(
    "scripts/soak-gateway.py",
    "--bin", $Bin,
    "--profile", $profile,
    "--duration", $Dur,
    "--concurrency", $C,
    "--envelope-status", "published"
  )
}
Write-Host "soak-gateway published: OK (update capacity-envelope-draft.md and ROADMAP with JSON evidence)"
