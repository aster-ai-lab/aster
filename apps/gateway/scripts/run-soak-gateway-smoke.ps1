# Short smoke soak for the capacity/soak harness (not a published envelope).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Bin = if ($args.Count -ge 1) { $args[0] } else { ".\target\debug\aster-gateway.exe" }
if (-not (Test-Path $Bin)) {
  Write-Host "building aster-gateway..."
  cargo build --bin aster-gateway
  $Bin = ".\target\debug\aster-gateway.exe"
}

$Dur = if ($env:ASTER_GATEWAY_SOAK_DURATION) { $env:ASTER_GATEWAY_SOAK_DURATION } else { "30" }
$HttpC = if ($env:ASTER_GATEWAY_SOAK_CONCURRENCY_HTTP) { $env:ASTER_GATEWAY_SOAK_CONCURRENCY_HTTP } else { "16" }
$SseC = if ($env:ASTER_GATEWAY_SOAK_CONCURRENCY_SSE) { $env:ASTER_GATEWAY_SOAK_CONCURRENCY_SSE } else { "8" }

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

Invoke-Python @("scripts/soak-gateway.py", "--bin", $Bin, "--profile", "http-json", "--duration", $Dur, "--concurrency", $HttpC, "--envelope-status", "smoke-only")
Invoke-Python @("scripts/soak-gateway.py", "--bin", $Bin, "--profile", "sse-finite", "--duration", $Dur, "--concurrency", $SseC, "--envelope-status", "smoke-only")
Invoke-Python @("scripts/soak-gateway.py", "--bin", $Bin, "--profile", "openai-json", "--duration", $Dur, "--concurrency", $HttpC, "--envelope-status", "smoke-only")
Invoke-Python @("scripts/soak-gateway.py", "--bin", $Bin, "--profile", "openai-sse", "--duration", $Dur, "--concurrency", $SseC, "--envelope-status", "smoke-only")
Write-Host "soak-gateway smoke: OK"
