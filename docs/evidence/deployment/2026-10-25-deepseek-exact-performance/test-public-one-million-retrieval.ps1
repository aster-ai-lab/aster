param(
    [Parameter(Mandatory=$true)][System.Net.Http.HttpClient]$Client,
    [Parameter(Mandatory=$true)][string]$PromptGzip,
    [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
    [string]$BaseUrl='http://117.162.0.179:8080/v1'
)
$ErrorActionPreference='Stop'
$utf8=New-Object Text.UTF8Encoding($false)
[IO.Directory]::CreateDirectory($EvidenceDirectory) | Out-Null
$inputStream=[IO.File]::OpenRead($PromptGzip)
$gzip=New-Object IO.Compression.GZipStream($inputStream,[IO.Compression.CompressionMode]::Decompress)
$reader=New-Object IO.StreamReader($gzip,$utf8)
try{$prompt=$reader.ReadToEnd()}finally{$reader.Dispose();$gzip.Dispose();$inputStream.Dispose()}
$sha=[Security.Cryptography.SHA256]::Create()
$promptBytes=$utf8.GetBytes($prompt)
$promptDigest=([BitConverter]::ToString($sha.ComputeHash($promptBytes))).Replace('-','').ToLowerInvariant()
if($promptDigest -ne '539aadd60c25d9abfbfc4defab2a3c658b9abc9e78cd45355b3b0930bdd12489'){throw 'Fixture digest mismatch'}
$expected='EARLY=violet-7391-copper;MIDDLE=silver-4826-orchid;LATE=amber-9153-lantern'
$body=@{model='deepseek-v4.1-flash';messages=@(@{role='user';content=$prompt;queuePosition=1;queueTotal=2;agent='owned-one-million-test';startsNewUserRequest=$true});max_tokens=1024;temperature=0;seed=42;stream=$true;stream_options=@{include_usage=$true}}
$json=ConvertTo-Json -InputObject $body -Depth 20 -Compress
$requestBytes=$utf8.GetBytes($json)
$proof=[ordered]@{started_at=[DateTimeOffset]::UtcNow.ToString('o');base_url=$BaseUrl;model=$body.model;prompt_sha256=$promptDigest;request_utf8_bytes=$requestBytes.Length;request_sha256=([BitConverter]::ToString($sha.ComputeHash($requestBytes))).Replace('-','').ToLowerInvariant();success=$false;usage_precision='public usage may be rounded; exact usage belongs to native cold proof'}
[IO.File]::WriteAllText((Join-Path $EvidenceDirectory 'request-profile.json'),(ConvertTo-Json -InputObject $proof -Depth 20),$utf8)
$payload=New-Object Net.Http.StringContent($json,$utf8,'application/json')
$response=$null
$watch=[Diagnostics.Stopwatch]::StartNew()
try{
    $response=$Client.PostAsync("$BaseUrl/chat/completions",$payload).GetAwaiter().GetResult()
    $raw=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $EvidenceDirectory 'response.sse'),$raw,$utf8)
    $proof.status=[int]$response.StatusCode
    if($proof.status -ne 200){throw "Gateway HTTP $($proof.status)"}
    if($response.Content.Headers.ContentType.MediaType -ne 'text/event-stream'){throw 'Expected SSE'}
    $content='';$reasoning='';$finish=$null;$usage=$null;$done=$false
    foreach($frame in ($raw -split '\r?\n\r?\n')){
        $data=@(foreach($line in ($frame -split '\r?\n')){if($line.StartsWith('data:')){$line.Substring(5).TrimStart(' ')}}) -join "`n"
        if([string]::IsNullOrEmpty($data)){continue}
        if($data -eq '[DONE]'){$done=$true;continue}
        if($done){throw 'Data after DONE'}
        $event=ConvertFrom-Json -InputObject $data
        if($event.error){throw ('Gateway SSE error: '+$event.error.message)}
        if($event.usage){$usage=$event.usage}
        foreach($choice in $event.choices){
            $content += [string]$choice.delta.content
            $reasoning += [string]$choice.delta.reasoning_content
            if($choice.finish_reason){$finish=$choice.finish_reason}
        }
    }
    if(-not $done -or $finish -ne 'stop' -or $content.Trim() -ne $expected){throw 'Full-input gateway retrieval mismatch'}
    if($null -eq $usage -or $usage.prompt_tokens -lt 1000000 -or $usage.total_tokens -ne ($usage.prompt_tokens+$usage.completion_tokens)){throw 'Public token accounting did not retain a million-token prompt'}
    $proof.success=$true;$proof.full_input_retrieval_passed=$true;$proof.content=$content;$proof.reasoning_content=$reasoning;$proof.usage=$usage;$proof.finish_reason=$finish
}catch{$proof.error=$_.Exception.Message;throw}
finally{
    $proof.wall_seconds=$watch.Elapsed.TotalSeconds;$proof.finished_at=[DateTimeOffset]::UtcNow.ToString('o')
    [IO.File]::WriteAllText((Join-Path $EvidenceDirectory 'result.json'),(ConvertTo-Json -InputObject $proof -Depth 30),$utf8)
    if($null -ne $response){$response.Dispose()};$payload.Dispose();$sha.Dispose()
}
[pscustomobject]$proof | ConvertTo-Json -Depth 30 -Compress

