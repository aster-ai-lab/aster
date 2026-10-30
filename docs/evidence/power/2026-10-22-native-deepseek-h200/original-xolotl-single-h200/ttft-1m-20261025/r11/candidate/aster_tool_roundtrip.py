"""Exact completed-round loop detection and one bounded, tool-free recovery response."""
import json

RECOVERY_KEY = '_aster_completed_tool_loop_recovery'
RECOVERY_MAX_TOKENS = 8192
RECOVERY_NOTE = 'SERVER TOOL-LOOP EXECUTION STATUS: STOPPED. Completed identical tool rounds: 3. The requested fourth round was prevented; it has NOT executed. No further tools are available. Accurately report three completed rounds, never four. Finish the original task from existing results, or state the actual tool error and missing information. Respect any requested response format. Never claim an unexecuted action completed. Do not emit tool calls or tool-call markup.'


def _canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False)


def repeated_completed_suffix(messages):
    """Require three adjacent complete rounds, same exact names/arguments/results; never infer tool meaning."""
    if not isinstance(messages, list):
        return False
    end = len(messages)
    fingerprints = []
    for _ in range(3):
        results = {}
        while end and isinstance(messages[end - 1], dict) and messages[end - 1].get('role') == 'tool':
            message = messages[end - 1]
            cid = message.get('tool_call_id')
            if not isinstance(cid, str) or not cid or cid in results:
                return False
            results[cid] = message.get('content')
            end -= 1
            if len(results) > 64:
                return False
        if not results or not end:
            return False
        message = messages[end - 1]
        if not isinstance(message, dict) or message.get('role') != 'assistant':
            return False
        calls = message.get('tool_calls')
        if not isinstance(calls, list) or not 1 <= len(calls) <= 64:
            return False
        ordered = []
        seen = set()
        for call in calls:
            if not isinstance(call, dict):
                return False
            cid = call.get('id')
            fn = call.get('function')
            if not isinstance(cid, str) or not cid or cid in seen or cid not in results or not isinstance(fn, dict):
                return False
            if not isinstance(fn.get('name'), str) or not fn['name']:
                return False
            args = fn.get('arguments')
            try:
                args = json.loads(args) if isinstance(args, str) else args
                if not isinstance(args, dict):
                    return False
                ordered.append((fn['name'], args, results[cid]))
            except (ValueError, TypeError):
                return False
            seen.add(cid)
        if seen != set(results):
            return False
        try:
            # Parallel call order and result arrival order do not change a round's meaning.
            fingerprints.append(_canonical(sorted(_canonical(item) for item in ordered)))
        except (ValueError, TypeError):
            return False
        end -= 1
    return len(set(fingerprints)) == 1


def install(server):
    original_converter = server.openai_to_messages
    original_chunks = server.openai_chunks

    def converter(req):
        req.pop(RECOVERY_KEY, None)
        repeated = repeated_completed_suffix(req.get('messages'))
        messages, tools, kwargs = original_converter(req)
        if not repeated or not tools:
            return messages, tools, kwargs
        # Keep the original schema in the rendered prefix, retaining the existing KV cache.
        wrapped = [t if 'function' in t else dict(type='function', function=t) for t in tools]
        if messages and messages[0].get('role') == 'system':
            messages[0] = dict(messages[0], tools=wrapped)
        else:
            messages.insert(0, dict(role='system', content='', tools=wrapped))
        messages.append(dict(role='user', content=RECOVERY_NOTE))
        raw = req.get('max_completion_tokens') or req.get('max_tokens') or 0
        if raw in (None, 0, -1) and not isinstance(raw, bool):
            limit = RECOVERY_MAX_TOKENS
        elif isinstance(raw, int) and not isinstance(raw, bool) and raw > 0:
            limit = min(raw, RECOVERY_MAX_TOKENS)
        else:
            raise ValueError('tool_loop_detected: invalid recovery output token limit')
        req['max_tokens'] = req['max_completion_tokens'] = limit
        req.pop('tool_choice', None)
        req[RECOVERY_KEY] = True
        return messages, None, kwargs

    def chunks(svc, req, *args, **kwargs):
        iterator = original_chunks(svc, req, *args, **kwargs)
        if req.get(RECOVERY_KEY) is not True:
            yield from iterator
            return
        buffered, size, content = [], 0, []
        try:
            for chunk in iterator:
                if chunk is None:
                    # Keep the connection alive during the bounded recovery, without exposing a tool call.
                    yield None
                    continue
                size += len(_canonical(chunk).encode())
                if size > 2 * 1024**2:
                    raise ValueError('tool_loop_detected: bounded recovery response exceeded')
                for choice in chunk.get('choices', []):
                    delta = choice.get('delta', {})
                    if delta.get('tool_calls') or choice.get('finish_reason') == 'tool_calls':
                        raise ValueError('tool_loop_detected: repeated tools stopped; recovery requested another tool')
                    if isinstance(delta.get('content'), str):
                        content.append(delta['content'])
                buffered.append(chunk)
            answer = ''.join(content)
            if not answer.strip() or '<｜DSML｜' in answer:
                raise ValueError('tool_loop_detected: repeated tools stopped; recovery did not produce a final answer')
            yield from buffered
        finally:
            if hasattr(iterator, 'close'):
                iterator.close()

    server.openai_to_messages = converter
    server.openai_chunks = chunks

