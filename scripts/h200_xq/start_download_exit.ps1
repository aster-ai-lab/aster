$ErrorActionPreference = 'Stop'
$taskStateDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target/h200-xq'))
$null = [IO.Directory]::CreateDirectory($taskStateDirectory)
$taskKeyBytes = [byte[]]::new(32)
[Security.Cryptography.RandomNumberGenerator]::Fill($taskKeyBytes)
$taskPassword = [Convert]::ToHexString($taskKeyBytes).ToLowerInvariant()
$taskBootstrap = @{username='aster-download'; password=$taskPassword; port=18890} | ConvertTo-Json -Compress
$taskNodeInfo = [Diagnostics.ProcessStartInfo]::new()
$taskNodeInfo.FileName = (Get-Command node).Source
$taskNodeInfo.ArgumentList.Add((Join-Path $PSScriptRoot 'download_exit.cjs'))
$taskNodeInfo.UseShellExecute = $false
$taskNodeInfo.CreateNoWindow = $true
$taskNodeInfo.RedirectStandardInput = $true
$taskNodeInfo.RedirectStandardOutput = $true
$taskNodeInfo.RedirectStandardError = $true
$taskNode = [Diagnostics.Process]::Start($taskNodeInfo)
$taskInstalled = $false
try {
    $taskNode.StandardInput.Write($taskBootstrap)
    $taskNode.StandardInput.Close()
    $taskReadyRead = $taskNode.StandardOutput.ReadLineAsync()
    if (-not $taskReadyRead.Wait(10000)) { throw 'temporary download proxy startup timed out' }
    $taskReady = $taskReadyRead.Result | ConvertFrom-Json
    if (-not $taskReady.ready) { throw 'temporary download proxy did not start' }
    $taskRemotePython = @'
import json,os,sys
from pathlib import Path
value=json.loads(sys.stdin.read(2049))
if value.get("username")!="aster-download" or len(value.get("password",""))!=64:
    raise RuntimeError("invalid authentication bootstrap")
root=Path.home()/".aster-h200-xq-download"
if root.exists() and (root.is_symlink() or root.stat().st_uid!=os.getuid() or root.stat().st_mode&0o777!=0o700):
    raise RuntimeError("private credential directory has invalid ownership or mode")
root.mkdir(mode=0o700,exist_ok=True)
auth=json.dumps({"username":value["username"],"password":value["password"]})
curl="proxy-user = "+json.dumps(value["username"]+":"+value["password"])+"\n"
for name,contents in (("auth.json",auth),(".curlrc",curl)):
    temporary=root/(name+".tmp")
    fd=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600)
    os.fchmod(fd,0o600)
    with os.fdopen(fd,"w") as stream:stream.write(contents)
    temporary.replace(root/name)
print("PRIVATE_DOWNLOAD_AUTH_READY")
'@
    if ($taskRemotePython.Contains("'")) { throw 'remote bootstrap quoting invariant failed' }
    $taskRemoteCommand = ". /etc/profile.d/ssh-init.sh >/dev/null 2>&1`npython3 -c '$taskRemotePython'"
    $taskSshInfo = [Diagnostics.ProcessStartInfo]::new()
    $taskSshInfo.FileName = (Get-Command ssh).Source
    foreach ($taskArgument in @('-CAXY','-o','BatchMode=yes','-o','ConnectTimeout=20',
            'yangchao-dev.yangchao.ailab-hrprdduanyan.ws@h.pjlab.org.cn',$taskRemoteCommand)) {
        $taskSshInfo.ArgumentList.Add($taskArgument)
    }
    $taskSshInfo.UseShellExecute = $false
    $taskSshInfo.CreateNoWindow = $true
    $taskSshInfo.RedirectStandardInput = $true
    $taskSshInfo.RedirectStandardOutput = $true
    $taskSshInfo.RedirectStandardError = $true
    $taskSsh = [Diagnostics.Process]::Start($taskSshInfo)
    $taskSsh.StandardInput.Write($taskBootstrap)
    $taskSsh.StandardInput.Close()
    if (-not $taskSsh.WaitForExit(30000)) { $taskSsh.Kill(); throw 'private credential staging timed out' }
    $taskResult = $taskSsh.StandardOutput.ReadToEnd().Trim()
    if ($taskSsh.ExitCode -ne 0 -or $taskResult -ne 'PRIVATE_DOWNLOAD_AUTH_READY') {
        throw 'private credential staging did not succeed'
    }
    $taskMetadata = [ordered]@{node_pid=$taskNode.Id; local_port=18890; authenticated=$true; max_connections=$taskReady.max_connections;
        private_credentials_staged=$true; started_unix=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()}
    [IO.File]::WriteAllText((Join-Path $taskStateDirectory 'auth-proxy-state.json'),
        (ConvertTo-Json -InputObject $taskMetadata), [Text.UTF8Encoding]::new($false))
    $taskInstalled = $true
    ConvertTo-Json -InputObject $taskMetadata -Compress
} finally {
    if (-not $taskInstalled -and -not $taskNode.HasExited) { $taskNode.Kill(); $taskNode.WaitForExit() }
    $taskPassword = $null
    $taskBootstrap = $null
    [Array]::Clear($taskKeyBytes)
}
