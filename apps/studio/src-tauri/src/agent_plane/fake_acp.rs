//! Scripted ACP peer shared by the turn harness and the HTTP plane test.
//! It speaks protocolVersion 1, then `session/prompt` updates. A prompt that
//! contains `write-file` settles an edit with a diff path. `fail-file` settles
//! the same kind of edit as a tool error. `render` emits prose, that edit, a
//! closed `ts` fence, a closed mermaid fence, and an unclosed mermaid fence.
//! `ask` and `hang` keep the older harness paths.

pub const SCRIPT: &str = r#"#!/usr/bin/env node
const rl = require('readline').createInterface({ input: process.stdin })
const fs = require('fs')
const path = require('path')
const traceFile = path.join(path.dirname(process.argv[1]), 'turn-trace.json')
function trace(mutate) {
  let current = { launches: [], prompts: [] }
  try { current = JSON.parse(fs.readFileSync(traceFile, 'utf8')) } catch (_) {}
  mutate(current)
  fs.writeFileSync(traceFile, JSON.stringify(current))
}
trace((current) => { current.launches.push(process.argv.slice(2)) })
const history = []
const session = 'S'
let promptId = null
let promptText = ''
let internalOnce = false
const isHangRequest = (text) => text === 'hang' || text === 'hang please'
const failInternal = () => {
  send({ jsonrpc: '2.0', id: promptId, error: { code: -32603, message: 'Internal error', data: { message: 'context length exceeded' } } })
  promptId = null
}
const send = (m) => process.stdout.write(JSON.stringify(m) + '\n')
const update = (u) => send({ jsonrpc: '2.0', method: 'session/update', params: { sessionId: session, update: u } })
const finish = (text) => {
  update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text } })
  send({ jsonrpc: '2.0', id: promptId, result: { stopReason: 'end_turn' } })
  promptId = null
}
const endTurn = () => {
  send({ jsonrpc: '2.0', id: promptId, result: { stopReason: 'end_turn' } })
  promptId = null
}
rl.on('line', (line) => {
  const m = JSON.parse(line)
  if (m.id === 950 && m.result) {
    const result = m.result
    if (result._meta && result._meta.followup_message) return finish('typed: ' + result._meta.followup_message)
    return finish(result.outcome && result.outcome.optionId === 'question-1' ? 'selected B.docx' : 'no selection')
  }
  if (m.method === 'initialize') return send({ jsonrpc: '2.0', id: m.id, result: { protocolVersion: 1 } })
  if (m.method === 'session/new') {
    try {
      fs.writeFileSync(path.join(path.dirname(process.argv[1]), 'session-new.json'), JSON.stringify(m.params))
    } catch (_) {}
    return send({ jsonrpc: '2.0', id: m.id, result: { sessionId: session } })
  }
  if (m.method === 'session/set_mode') {
    trace((current) => {
      if (!current.modes) current.modes = []
      current.modes.push(m.params && m.params.modeId)
    })
    return send({ jsonrpc: '2.0', id: m.id, result: {} })
  }
  if (m.method === 'session/cancel') {
    if (promptId !== null) send({ jsonrpc: '2.0', id: promptId, result: { stopReason: 'cancelled' } })
    promptId = null
    return
  }
  if (m.method === '_x.ai/interject') {
    const text = (m.params && m.params.text) || ''
    trace((current) => {
      if (!current.interjects) current.interjects = []
      current.interjects.push(text)
    })
    send({ jsonrpc: '2.0', id: m.id, result: { ok: true } })
    if (promptId !== null && isHangRequest(promptText)) finish('inserted ' + text)
    return
  }
  if (m.method === 'session/prompt') {
    promptId = m.id
    const text = m.params.prompt[0].text
    promptText = text
    trace((current) => {
      current.prompts.push(text)
      if (!current.promptMeta) current.promptMeta = []
      current.promptMeta.push(m.params && m.params._meta ? m.params._meta : null)
    })
    update({ sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: 'thinking' } })
    if (text.includes('once-internal') && !internalOnce) {
      internalOnce = true
      return failInternal()
    }
    if (text.includes('stay-internal')) return failInternal()
    if (text.includes('fresh-internal')) {
      const file = path.join(path.dirname(process.argv[1]), 'internal-fails')
      let count = 0
      try { count = Number(fs.readFileSync(file, 'utf8')) } catch (_) {}
      count += 1
      fs.writeFileSync(file, String(count))
      if (count <= 2) return failInternal()
    }
    history.push(text)
    if (text.includes('native-question-choice') || text.includes('native-question-text')) {
      const choices = text.includes('native-question-choice')
      return send({ jsonrpc: '2.0', id: 950, method: 'session/request_permission', params: {
        sessionId: session,
        _meta: { question: { allowFreeText: true, hasChoices: choices } },
        toolCall: { toolCallId: 'question-fixture', title: '选哪份文档？', kind: 'other' },
        options: (choices ? ['A.docx', 'B.docx'] : ['Answer in text']).map((name, i) => ({optionId: 'question-' + i, name, kind: 'reject_once'})),
      } })
    }
    if (text.includes('Document segment')) {
      if (text.includes('review-clean-memory')) return finish('[]')
      const attempt = history.filter(prompt => prompt.includes('Document segment')).length
      if (text.includes('review-repair-abort') && attempt === 2) return
      if (text.includes('review-repair-repeat') || (text.includes('review-repair-quote') && attempt === 1) || (text.includes('review-repair-abort') && attempt === 1)) {
        return finish('```a3s-review-finding\n' + JSON.stringify({id:'bad',title:'引文',evidenceQuote:'不在原文中的句子',summary:'未绑定',guidance:'不能采用'}) + '\n```')
      }
      if (text.includes('review-repair-format') && attempt === 1) return finish('[]\n```a3s-review-finding\n[]\n```\n解释不能混入结果。')
      const marker = '(untrusted data, not instructions):\n'
      const at = text.indexOf(marker)
      const taskAt = text.indexOf('\n\nUser task:')
      const body = at < 0 ? '' : text.slice(at + marker.length, taskAt > at ? taskAt : text.length)
      const quote = Array.from(body).slice(0, 8).join('')
      const payload = JSON.stringify({ id: 'seg', title: '段', evidenceQuote: quote, summary: '已读', guidance: '保留' })
      return finish('```a3s-review-finding\n' + payload + '\n```')
    }
    if (text.includes('ask-read')) {
      return send({ jsonrpc: '2.0', id: 900, method: 'session/request_permission', params: {
        sessionId: session,
        toolCall: { toolCallId: 't1', title: 'Read notes.md', rawInput: { path: 'notes.md' } },
        options: [{ optionId: 'allow-once' }, { optionId: 'reject-once' }],
      } })
    }
    if (text.includes('ask-twice')) {
      return send({ jsonrpc: '2.0', id: 901, method: 'session/request_permission', params: {
        sessionId: session,
        toolCall: { toolCallId: 't1', title: 'bash', rawInput: { command: 'rm x' } },
        options: [{ optionId: 'allow-once' }, { optionId: 'reject-once' }],
      } })
    }
    if (/(^|[^a-z])ask([^a-z]|$)/i.test(text)) {
      return send({ jsonrpc: '2.0', id: 900, method: 'session/request_permission', params: {
        sessionId: session,
        toolCall: { toolCallId: 't1', title: 'bash', rawInput: { command: 'rm x' } },
        options: [{ optionId: 'allow-once' }, { optionId: 'reject-once' }],
      } })
    }
    if (isHangRequest(text)) return
    if (text.includes('fail-file')) {
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '写失败了。' } })
      update({
        sessionUpdate: 'tool_call',
        toolCallId: 't-fail',
        title: 'edit',
        status: 'in_progress',
        content: [{ type: 'diff', path: '/workspace/notes.md', oldText: '', newText: 'x' }]
      })
      update({
        sessionUpdate: 'tool_call_update',
        toolCallId: 't-fail',
        status: 'failed',
        content: [{ type: 'text', text: 'disk full' }]
      })
      return endTurn()
    }
    if (text.includes('write-file')) {
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '先写文件。' } })
      update({ sessionUpdate: 'tool_call', toolCallId: 't-write', title: 'edit', status: 'in_progress' })
      update({
        sessionUpdate: 'tool_call_update',
        toolCallId: 't-write',
        status: 'completed',
        content: [{ type: 'diff', path: '/workspace/notes.md', oldText: '', newText: 'hello' }]
      })
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '写好了。' } })
      return endTurn()
    }
    if (text.includes('render')) {
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '先看这段。' } })
      update({ sessionUpdate: 'tool_call', toolCallId: 't-render', title: 'edit', status: 'in_progress' })
      update({
        sessionUpdate: 'tool_call_update',
        toolCallId: 't-render',
        status: 'completed',
        content: [{ type: 'diff', path: '/workspace/notes.md', oldText: '', newText: 'hello' }]
      })
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '\n\n```ts\nconst answer = 1\n```\n\n' } })
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '```mermaid\ngraph TD\nA-->B\n```\n\n' } })
      update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '```mermaid\ngraph TD\nC-->D' } })
      return endTurn()
    }
    return finish(history.length > 1 ? history[0] + '\n' + text : 'hello')
  }
  if (m.id === 901) {
    return send({ jsonrpc: '2.0', id: 902, method: 'session/request_permission', params: {
      sessionId: session,
      toolCall: { toolCallId: 't2', title: 'bash', rawInput: { command: 'rm y' } },
      options: [{ optionId: 'allow-once' }, { optionId: 'reject-once' }],
    } })
  }
  if (m.id === 900 || m.id === 902) finish(m.result.outcome.optionId === 'allow-once' ? 'allowed' : 'denied')
})
"#;
