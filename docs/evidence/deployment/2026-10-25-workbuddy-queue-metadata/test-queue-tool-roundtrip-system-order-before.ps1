param(
    [Parameter(Mandatory=$true)][System.Net.Http.HttpClient]$Client,
    [string]$EvidenceDirectory = $PSScriptRoot,
    [string]$BaseUrl = 'http://117.162.0.179:8080/v1',
    [string[]]$Models = @('deepseek-v4.1-flash', 'prism-27b')
)
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$records = New-Object 'System.Collections.Generic.List[object]'
$proofFile = Join-Path $EvidenceDirectory 'workbuddy-tool-roundtrip.json'

function Save-Proof {
    $proof = [ordered]@{
        at = [DateTimeOffset]::UtcNow.ToString('o')
        base_url = $BaseUrl
        cases = @($records.ToArray())
        all_passed = ($records.Count -gt 0 -and @($records | Where-Object { -not $_.passed }).Count -eq 0)
    }
    [IO.File]::WriteAllText($proofFile, (ConvertTo-Json -InputObject $proof -Depth 60), $utf8)
}

function Invoke-Chat([hashtable]$Body, [string]$Case) {
    $prefix=@()
    for($i=0;$i -lt 12;$i++){
        $prefix+=@{role='user';content="Calibration turn $i."}
        $prefix+=@{role='assistant';content='Acknowledged.'}
    }
    $Body.messages=@($prefix)+@($Body.messages)
    foreach($message in $Body.messages){
        if($message -is [hashtable]){$message.queuePosition=1;$message.queueTotal=2}
        else{$message | Add-Member -NotePropertyName queuePosition -NotePropertyValue 1 -Force; $message | Add-Member -NotePropertyName queueTotal -NotePropertyValue 2 -Force}
    }
    [IO.File]::WriteAllText((Join-Path $EvidenceDirectory ($Case+'-request.json')),(ConvertTo-Json -InputObject $Body -Depth 60 -Compress),$utf8)
    $request = New-Object System.Net.Http.HttpRequestMessage([System.Net.Http.HttpMethod]::Post, "$BaseUrl/chat/completions")
    $request.Content = New-Object System.Net.Http.StringContent((ConvertTo-Json -InputObject $Body -Depth 60 -Compress), $utf8, 'application/json')
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $response = $null
    try {
        $response = $Client.SendAsync($request).GetAwaiter().GetResult()
        $wire = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
        $status = [int]$response.StatusCode
        $file = $Case + $(if ($Body.stream) { '.sse' } else { '.json' })
        [IO.File]::WriteAllText((Join-Path $EvidenceDirectory $file), $wire, $utf8)
        if ($status -ne 200) { throw "HTTP $status; response retained in $file" }
        if (-not $Body.stream) {
            $value = ConvertFrom-Json -InputObject $wire
            if ($value.PSObject.Properties.Name -contains 'success') { $value = $value.data }
            if ($null -eq $value.choices -or $value.choices.Count -ne 1) { throw 'Expected one chat choice' }
            return [pscustomobject]@{ message=$value.choices[0].message; finish=$value.choices[0].finish_reason; usage=$value.usage; done=$true; status=$status; seconds=$watch.Elapsed.TotalSeconds; response_file=$file }
        }
        if ($response.Content.Headers.ContentType.MediaType -ne 'text/event-stream') { throw 'Expected SSE response media type' }
        $calls = @{}
        $content = ''
        $reasoning = ''
        $finish = $null
        $usage = $null
        $done = $false
        foreach ($frame in ($wire -split '\r?\n\r?\n')) {
            $data = @(foreach ($line in ($frame -split '\r?\n')) { if ($line.StartsWith('data:')) { $line.Substring(5).TrimStart(' ') } }) -join "`n"
            if ([string]::IsNullOrEmpty($data)) { continue }
            if ($data -eq '[DONE]') { $done=$true; continue }
            if ($done) { throw 'SSE data arrived after DONE' }
            $event = ConvertFrom-Json -InputObject $data
            if ($event.error) { throw ('SSE error: '+$event.error.message) }
            if ($event.usage) { $usage=$event.usage }
            foreach ($choice in $event.choices) {
                if ($choice.finish_reason) { $finish=$choice.finish_reason }
                $content += [string]$choice.delta.content
                $reasoning += [string]$choice.delta.reasoning_content
                foreach ($call in $choice.delta.tool_calls) {
                    if ($null -eq $call.index) { throw 'Missing tool index' }
                    $index=[int]$call.index
                    if (-not $calls.ContainsKey($index)) { $calls[$index]=@{ id=''; type='function'; function=@{name=''; arguments=''} } }
                    if ($call.id) { $calls[$index].id=$call.id }
                    if ($call.type) { $calls[$index].type=$call.type }
                    $calls[$index].function.name += [string]$call.function.name
                    $calls[$index].function.arguments += [string]$call.function.arguments
                }
            }
        }
        if (-not $done -or -not $finish) { throw 'Missing SSE finish or DONE' }
        $message=@{role='assistant'; content=$content}
        if ($reasoning) { $message.reasoning_content=$reasoning }
        if ($calls.Count) { $message.tool_calls=@($calls.Keys | Sort-Object | ForEach-Object { $calls[$_] }) }
        return [pscustomobject]@{ message=$message; finish=$finish; usage=$usage; done=$done; status=$status; seconds=$watch.Elapsed.TotalSeconds; response_file=$file }
    } finally {
        if ($null -ne $response) { $response.Dispose() }
        $request.Dispose()
    }
}

$tools=@(@{type='function';function=@{name='lookup_test_record';description="Return a record's verification value.";parameters=@{type='object';properties=@{id=@{type='integer'}};required=@('id');additionalProperties=$false}}})
foreach ($model in $Models) {
    foreach ($mode in @(@{name='auto-json';stream=$false;choice='auto'},@{name='named-sse';stream=$true;choice=@{type='function';function=@{name='lookup_test_record'}}},@{name='required-json';stream=$false;choice='required'},@{name='auto-sse';stream=$true;choice='auto'})) {
        $case=($model+'-'+$mode.name)
        $record=[ordered]@{ case=$case;model=$model;stream=$mode.stream;passed=$false }
        try {
            $messages=@(
                @{role='system';content='Use lookup_test_record when asked for a record. After the tool replies, output only its verification value.'},
                @{role='user';content='Look up record 17 and return its verification value.';agent=@{name='workbuddy-test'};conversationRequestId=[Guid]::NewGuid().ToString();startsNewUserRequest=$true}
            )
            $body=@{model=$model;messages=$messages;tools=$tools;tool_choice=$mode.choice;parallel_tool_calls=$false;temperature=0;max_tokens=512;stream=$mode.stream}
            if ($mode.stream) { $body.stream_options=@{include_usage=$true} }
            $first=Invoke-Chat $body ($case+'-call')
            $calls=@($first.message.tool_calls)
            if ($first.finish -ne 'tool_calls' -or $calls.Count -ne 1) { throw 'Expected exactly one completed tool call' }
            $call=$calls[0]
            $arguments=ConvertFrom-Json -InputObject $call.function.arguments
            if ($call.type -ne 'function' -or -not $call.id -or $call.function.name -ne 'lookup_test_record' -or $arguments.id -ne 17 -or @($arguments.PSObject.Properties).Count -ne 1) { throw 'Incorrect tool identity or arguments' }
            # Generated only after the model has made its call: the model cannot guess the answer from the initial prompt.
            $nonce='proof-'+[Guid]::NewGuid().ToString('N').Substring(0,16)
            $body.tool_choice='none'
            $body.messages=@($messages)+@($first.message)+@(@{role='tool';tool_call_id=$call.id;name='lookup_test_record';content=(ConvertTo-Json -InputObject @{verification=$nonce} -Compress);agent='workbuddy-tool';startsNewUserRequest=$false})
            $last=Invoke-Chat $body ($case+'-result')
            if ($last.finish -ne 'stop' -or ([string]$last.message.content).Trim() -ne $nonce -or @($last.message.tool_calls | Where-Object { $null -ne $_ }).Count -gt 0) { throw 'Tool-result roundtrip did not return the verification value' }
            foreach ($item in @($first,$last)) {
                if ($null -eq $item.usage -or $item.usage.total_tokens -ne ($item.usage.prompt_tokens+$item.usage.completion_tokens)) { throw 'Missing or inconsistent reported token usage' }
            }
            $record.passed=$true
            $record.call_status=$first.status
            $record.result_status=$last.status
            $record.call_seconds=$first.seconds
            $record.result_seconds=$last.seconds
            $record.tool_call_id=$call.id
            $record.arguments=$arguments
            $record.verification=$nonce
            $record.response_files=@($first.response_file,$last.response_file)
            $record.usage=@($first.usage,$last.usage)
        } catch { $record.error=$_.Exception.Message }
        $records.Add([pscustomobject]$record)
        Save-Proof
        [pscustomobject]$record | ConvertTo-Json -Depth 8 -Compress | Write-Output
    }
    foreach ($streaming in @($false,$true)) {
        $case=$model+'-hi-tools-'+$(if ($streaming) {'sse'}else{'json'})
        $record=[ordered]@{case=$case;model=$model;stream=$streaming;passed=$false}
        try {
            $body=@{model=$model;messages=@(@{role='user';content='Hi. Reply with exactly Hi and do not call any tool.';agent='workbuddy';conversationRequestId=[Guid]::NewGuid().ToString();startsNewUserRequest=$true});tools=$tools;tool_choice='auto';temperature=0;max_tokens=256;stream=$streaming}
            if ($streaming) { $body.stream_options=@{include_usage=$true} }
            $reply=Invoke-Chat $body $case
            if ($reply.finish -ne 'stop' -or ([string]$reply.message.content).Trim() -ne 'Hi' -or @($reply.message.tool_calls | Where-Object { $null -ne $_ }).Count -gt 0) { throw 'Hi with offered tools failed' }
            $record.passed=$true;$record.status=$reply.status;$record.seconds=$reply.seconds;$record.response_file=$reply.response_file
        } catch { $record.error=$_.Exception.Message }
        $records.Add([pscustomobject]$record);Save-Proof
        [pscustomobject]$record | ConvertTo-Json -Depth 8 -Compress | Write-Output
    }
}
if (@($records | Where-Object {-not $_.passed}).Count -gt 0) { throw "Tool compatibility validation failed; see $proofFile" }
