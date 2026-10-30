param(
    [Parameter(Mandatory=$true)][System.Net.Http.HttpClient]$Client,
    [string]$EvidenceDirectory = $PSScriptRoot,
    [string]$BaseUrl = 'http://117.162.0.179:8080/v1',
    [string[]]$Models = @('deepseek-v4.1-flash','prism-27b')
)
$ErrorActionPreference='Stop'
$utf8=New-Object System.Text.UTF8Encoding($false)
[IO.Directory]::CreateDirectory($EvidenceDirectory) | Out-Null
$history=@()
for($i=0;$i -lt 13;$i++){
    $history+=@{role='user';content="Calibration turn $i."}
    $history+=@{role='assistant';content='Acknowledged.'}
}
$history[25].queuePosition=1
$history[25].queueTotal=2
$history+=@{role='user';content='Reply with exactly OK.'}
$results=@()
foreach($model in $Models){
    $body=@{model=$model;messages=$history;max_tokens=128;temperature=0;stream=$false}
    $wire=ConvertTo-Json -InputObject $body -Depth 30 -Compress
    [IO.File]::WriteAllText((Join-Path $EvidenceDirectory ($model+'-request.json')),$wire,$utf8)
    $payload=New-Object System.Net.Http.StringContent($wire,$utf8,'application/json')
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $response=$Client.PostAsync("$BaseUrl/chat/completions",$payload).GetAwaiter().GetResult()
    try{
        $raw=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
        [IO.File]::WriteAllText((Join-Path $EvidenceDirectory ($model+'-response.json')),$raw,$utf8)
        $results+=@{model=$model;status=[int]$response.StatusCode;seconds=$watch.Elapsed.TotalSeconds}
    } finally {$response.Dispose();$payload.Dispose()}
}
[IO.File]::WriteAllText((Join-Path $EvidenceDirectory 'summary.json'),(ConvertTo-Json -InputObject $results -Depth 30),$utf8)
$results | ConvertTo-Json -Compress

