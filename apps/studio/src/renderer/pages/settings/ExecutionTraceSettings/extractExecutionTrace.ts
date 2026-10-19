/** Projects local a3s-code transcripts into `a3s.rl_trajectory.v1` for post-training. */

export const RL_TRAJECTORY_SCHEMA = 'a3s.rl_trajectory.v1' as const

/** Mode `on` stores up to 1 MiB of text, matching `RlTrajectoryMode::On`. */
export const RL_TRAJECTORY_MAX_TEXT_BYTES = 1024 * 1024

/**
 * Books written before this revision omit per-step signals.
 * A lower or missing revision is reprojected once on the next scan.
 */
export const TRACE_SIGNAL_REVISION = 1

export type RlEventType =
  | 'execution_start'
  | 'llm_request'
  | 'llm_response'
  | 'tool_call'
  | 'tool_result'
  | 'context_compacted'
  | 'execution_end'

export interface CapturedText {
  byte_len: number
  sha256: string
  truncated: boolean
  text?: string
}

export interface TrainingRecord {
  schema: typeof RL_TRAJECTORY_SCHEMA
  sequence: number
  timestamp_ms: number
  event_type: RlEventType
  session_id: string
  mode: 'on'
  context: Record<string, never>
  payload: Record<string, unknown>
}

export interface StoredTrainingEvent {
  messageId: string
  record: TrainingRecord
}

export interface TraceMessage {
  id: string
  role?: string
  status?: string
  createdAt: string
  updatedAt: string
  parts: readonly unknown[]
  stats?: unknown
}

export interface TraceSessionSource {
  id: string
  name: string
  updatedAt: string
  workspacePath?: string
  messages: readonly TraceMessage[]
  /** The listed session time still covers the stored message watermark. */
  unchanged?: boolean
  /** Messages to count as skipped when `unchanged` is set. */
  messageCount?: number
}

export interface SessionTrace {
  sessionId: string
  name: string
  updatedAt: string
  events: StoredTrainingEvent[]
  /** Newest message `updatedAt` already read. Absent until a full read. */
  messageWatermark?: string
  /** Messages in the last full read. Absent until that read. */
  messageCount?: number
}

export interface TraceBook {
  version: 2
  schema: typeof RL_TRAJECTORY_SCHEMA
  /** message id → updatedAt already extracted. */
  seen: Record<string, string>
  sessions: SessionTrace[]
  /** Present on books that store per-step decision, duration, and feedback. */
  signalRevision?: number
}

export interface TraceStepSignals {
  decision: string
  durationMs?: number
  feedback: string
  stopReason: string
  exitCode?: number
  errorKind: string
}

export interface ScanDelta {
  book: TraceBook
  skipped: number
  added: number
}

interface CapturedMessage {
  index?: number
  role: string
  content: Record<string, unknown>[]
  reasoning_content?: CapturedText
}

interface Draft {
  messageId: string
  at: string
  eventType: RlEventType
  payload: Record<string, unknown>
}

const ARG_KEYS = ['command', 'cmd', 'path', 'file_path', 'filePath', 'query', 'pattern', 'url'] as const

export function emptyTraceBook(): TraceBook {
  return { version: 2, schema: RL_TRAJECTORY_SCHEMA, signalRevision: TRACE_SIGNAL_REVISION, seen: {}, sessions: [] }
}

export async function captureTrainingText(
  text: string,
  maxBytes = RL_TRAJECTORY_MAX_TEXT_BYTES
): Promise<CapturedText> {
  const redacted = redactTrainingText(text)
  const bytes = new TextEncoder().encode(redacted)
  const sha256 = await sha256Hex(bytes)
  const clipped = truncateUtf8(bytes, maxBytes)
  return {
    byte_len: bytes.length,
    sha256,
    truncated: clipped.truncated,
    text: clipped.text
  }
}

export function trainingEventTool(record: TrainingRecord): string {
  const tool = record.payload.tool
  return typeof tool === 'string' ? tool : ''
}

export function trainingEventDetail(record: TrainingRecord): string {
  const payload = record.payload
  if (record.event_type === 'tool_call') return argText(payload.args)
  if (record.event_type === 'tool_result') return capturedBody(payload.output)
  if (record.event_type === 'execution_start') return capturedBody(payload.prompt)
  if (record.event_type === 'llm_response') return capturedBody(payload.response_text)
  if (record.event_type === 'execution_end')
    return capturedBody(payload.response_text) || capturedBody(payload.error_message)
  return ''
}

export function trainingEventState(record: TrainingRecord): 'done' | 'failed' | null {
  if (record.event_type === 'tool_result' || record.event_type === 'execution_end') {
    if (record.payload.success === false) return 'failed'
    if (record.payload.success === true) return 'done'
  }
  return null
}

export function trainingEventSignals(record: TrainingRecord): TraceStepSignals {
  const bag = asRecord(record.payload.signals) ?? {}
  const duration = numberField(bag, 'duration_ms')
  const payloadDuration = typeof record.payload.duration_ms === 'number' ? record.payload.duration_ms : undefined
  const exit = numberField(bag, 'exit_code')
  const payloadExit = typeof record.payload.exit_code === 'number' ? record.payload.exit_code : undefined
  return {
    decision: textOf(bag.decision) || derivedDecision(record),
    durationMs: duration ?? payloadDuration,
    feedback: textOf(bag.feedback),
    stopReason: textOf(bag.stop_reason) || textOf(record.payload.stop_reason),
    exitCode: exit ?? payloadExit,
    errorKind: textOf(bag.error_kind) || textOf(record.payload.error_kind)
  }
}

export function trainingEventUsage(record: TrainingRecord): { prompt: number; completion: number } | null {
  const usage = asRecord(record.payload.usage)
  if (!usage || typeof usage.prompt_tokens !== 'number' || typeof usage.completion_tokens !== 'number') return null
  return { prompt: usage.prompt_tokens, completion: usage.completion_tokens }
}

export async function scanSessionRecords(book: TraceBook, sources: readonly TraceSessionSource[]): Promise<ScanDelta> {
  const seen = { ...book.seen }
  const bySession = new Map<string, SessionTrace>(
    book.sessions.map((session) => [
      session.sessionId,
      { ...session, events: session.events.map((event) => ({ messageId: event.messageId, record: event.record })) }
    ])
  )
  let skipped = 0
  let added = 0

  for (const source of sources) {
    const session = bySession.get(source.id) ?? { sessionId: source.id, name: '', updatedAt: '', events: [] }
    const name = source.name.trim()
    if (name) session.name = name
    if (source.updatedAt) session.updatedAt = source.updatedAt
    if (source.unchanged) {
      skipped += source.messageCount ?? session.messageCount ?? 0
      bySession.set(source.id, session)
      continue
    }
    const messages = ordered(source.messages)
    const current = book.signalRevision === TRACE_SIGNAL_REVISION
    const dirtyAt =
      !current && messages.length > 0 ? 0 : messages.findIndex((message) => seen[message.id] !== message.updatedAt)
    session.messageWatermark = newestMessageTime(messages)
    session.messageCount = messages.length
    if (dirtyAt === -1) {
      skipped += messages.length
      bySession.set(source.id, session)
      continue
    }
    skipped += dirtyAt
    const events = await projectSession(source, messages)
    const dirtyIds = new Set(messages.slice(dirtyAt).map((message) => message.id))
    added += events.filter((event) => dirtyIds.has(event.messageId)).length
    for (const message of messages) seen[message.id] = message.updatedAt
    session.events = events
    bySession.set(source.id, session)
  }

  const sessions = [...bySession.values()]
    .filter((session) => session.events.length > 0)
    .sort(
      (left, right) => right.updatedAt.localeCompare(left.updatedAt) || left.sessionId.localeCompare(right.sessionId)
    )

  return {
    book: { version: 2, schema: RL_TRAJECTORY_SCHEMA, signalRevision: TRACE_SIGNAL_REVISION, seen, sessions },
    skipped,
    added
  }
}

function newestMessageTime(messages: readonly TraceMessage[]): string {
  let newest = ''
  for (const message of messages) {
    if (message.updatedAt > newest) newest = message.updatedAt
  }
  return newest
}

async function projectSession(
  source: TraceSessionSource,
  messages: readonly TraceMessage[]
): Promise<StoredTrainingEvent[]> {
  const drafts: Draft[] = []
  const history: CapturedMessage[] = []
  let systemText = ''
  let open = false
  let sawAssistant = false
  let expectResponse = false
  let turn = 0
  let toolCalls = 0
  let toolResults = 0
  let toolFailed = false
  let lastStatus = ''
  let lastResponse = ''
  let lastUsage: Record<string, number> | undefined
  let anchor: TraceMessage | undefined
  const errors: string[] = []

  const resetRun = () => {
    sawAssistant = false
    expectResponse = false
    turn = 0
    toolCalls = 0
    toolResults = 0
    toolFailed = false
    lastStatus = ''
    lastResponse = ''
    lastUsage = undefined
    errors.length = 0
  }

  const closeRun = async () => {
    if (!open) return
    const pendingTools = toolCalls !== toolResults
    const pendingStatus = lastStatus === 'pending' || lastStatus === 'running'
    if (sawAssistant && !pendingTools && !pendingStatus) {
      const failed = toolFailed || errors.length > 0 || lastStatus === 'error' || lastStatus === 'failed'
      const response = lastResponse ? await captureTrainingText(lastResponse) : undefined
      const errorMessage = errors.length > 0 ? await captureTrainingText(errors.join('\n')) : undefined
      push(drafts, anchor, 'execution_end', {
        success: !failed,
        response_text: response,
        usage: lastUsage,
        tool_calls_count: toolCalls,
        error_message: errorMessage,
        signals: stepSignals({ decision: failed ? 'stopped' : 'finished', success: !failed })
      })
    }
    open = false
  }

  for (const message of messages) {
    const kind = roleOf(message)
    if (kind === 'system') {
      const text = joinText(message.parts)
      if (text) systemText = systemText ? `${systemText}\n${text}` : text
      await pushAnnotations(drafts, message)
      continue
    }
    if (kind === 'user') {
      await closeRun()
      const text = joinText(message.parts)
      const content = await textBlocks(message.parts)
      if (!text && content.length === 0) {
        await pushAnnotations(drafts, message)
        continue
      }
      resetRun()
      open = true
      anchor = message
      const prior = indexed(history)
      push(drafts, message, 'execution_start', {
        workspace: source.workspacePath || undefined,
        prompt: text ? await captureTrainingText(text) : undefined,
        history_message_count: prior.length,
        history: prior,
        system_prompt: systemText ? await captureTrainingText(systemText) : undefined
      })
      history.push({ role: 'user', content })
      turn = 1
      expectResponse = true
      push(drafts, message, 'llm_request', {
        turn,
        messages_count: history.length,
        messages: indexed(history),
        system_prompt: systemText ? await captureTrainingText(systemText) : undefined
      })
      await pushAnnotations(drafts, message)
      continue
    }
    if (kind !== 'assistant') {
      await pushAnnotations(drafts, message)
      continue
    }
    const built = await assistantContent(message)
    if (!built.productive) {
      await pushAnnotations(drafts, message)
      const found = errorTexts(message.parts)
      if (found.length === 0) continue
      if (!open) {
        resetRun()
        open = true
        anchor = message
        push(drafts, message, 'execution_start', {
          workspace: source.workspacePath || undefined,
          history_message_count: history.length,
          history: indexed(history),
          system_prompt: systemText ? await captureTrainingText(systemText) : undefined
        })
      }
      sawAssistant = true
      anchor = message
      if (message.status?.trim()) lastStatus = message.status.trim()
      for (const error of found) errors.push(error)
      continue
    }
    const bare = !open
    if (bare) {
      resetRun()
      open = true
      anchor = message
      push(drafts, message, 'execution_start', {
        workspace: source.workspacePath || undefined,
        history_message_count: history.length,
        history: indexed(history),
        system_prompt: systemText ? await captureTrainingText(systemText) : undefined
      })
      turn = 1
    } else if (!expectResponse) {
      turn += 1
      push(drafts, message, 'llm_request', {
        turn,
        messages_count: history.length,
        messages: indexed(history),
        system_prompt: systemText ? await captureTrainingText(systemText) : undefined
      })
    }
    expectResponse = false
    sawAssistant = true
    anchor = message
    lastStatus = message.status?.trim() || ''
    if (built.responseText) lastResponse = built.responseText
    const usage = usageOf(message.stats)
    if (usage) lastUsage = usage
    const messageBody: CapturedMessage = {
      role: 'assistant',
      content: [...built.content, ...built.toolUses]
    }
    if (built.reasoning) messageBody.reasoning_content = await captureTrainingText(built.reasoning)
    const replyDuration = durationMs(message.stats)
    const stopReason = stopReasonOf(message)
    push(drafts, message, 'llm_response', {
      turn,
      message: messageBody,
      response_text: built.responseText ? await captureTrainingText(built.responseText) : undefined,
      reasoning_content: messageBody.reasoning_content,
      tool_calls: built.calls.length
        ? built.calls.map((call) => ({ tool_call_id: call.tool_call_id, tool: call.tool, args: call.args }))
        : undefined,
      usage: lastUsage,
      stop_reason: stopReason,
      duration_ms: replyDuration,
      signals: stepSignals({
        decision: modelDecision(built),
        durationMs: replyDuration,
        stopReason
      })
    })
    history.push(messageBody)
    const resultBlocks: Record<string, unknown>[] = []
    for (const call of built.calls) {
      toolCalls += 1
      push(drafts, message, 'tool_call', {
        turn,
        tool_call_id: call.tool_call_id,
        tool: call.tool,
        args: call.args,
        signals: stepSignals({
          decision: call.decision,
          durationMs: call.waitMs,
          feedback: call.feedback
        })
      })
      if (!call.terminal) continue
      toolResults += 1
      if (!call.success) toolFailed = true
      const output = call.output === undefined ? undefined : await captureTrainingText(call.output)
      push(drafts, message, 'tool_result', {
        turn,
        tool_call_id: call.tool_call_id,
        tool: call.tool,
        success: call.success,
        exit_code: call.exitCode,
        duration_ms: call.durationMs,
        output,
        error_kind: call.errorKind,
        signals: stepSignals({
          decision: call.success ? 'succeeded' : 'failed',
          durationMs: call.durationMs,
          feedback: call.feedback,
          exitCode: call.exitCode,
          errorKind: call.errorKind,
          success: call.success
        })
      })
      if (output) {
        resultBlocks.push({
          type: 'tool_result',
          tool_use_id: call.tool_call_id,
          content: output,
          is_error: !call.success
        })
      }
    }
    if (resultBlocks.length > 0) history.push({ role: 'user', content: resultBlocks })
    await pushAnnotations(drafts, message)
    for (const error of errorTexts(message.parts)) errors.push(error)
  }
  await closeRun()
  return drafts.map((draft, index) => envelope(source.id, draft, index + 1))
}

async function pushAnnotations(drafts: Draft[], message: TraceMessage): Promise<void> {
  for (const part of message.parts) {
    const record = asRecord(part)
    if (!record) continue
    const type = typeof record.type === 'string' ? record.type : ''
    if (type !== 'data-compact' && type !== 'data-compaction-anchor') continue
    const data = asRecord(record.data) ?? record
    if (type === 'data-compaction-anchor' && typeof data.status === 'string' && data.status !== 'done') continue
    const payload: Record<string, unknown> = {}
    const before = numberField(data, 'foldedCount', 'before_messages', 'beforeMessages')
    const after = numberField(data, 'after_messages', 'afterMessages')
    const percent = numberField(data, 'percent_before', 'percentBefore')
    if (before !== undefined) payload.before_messages = before
    if (after !== undefined) payload.after_messages = after
    if (percent !== undefined) payload.percent_before = percent
    const summary = firstString(data, 'compactedContent', 'content', 'summary')
    if (summary) {
      const role = message.role === 'user' || message.role === 'system' ? message.role : 'assistant'
      payload.messages = indexed([{ role, content: [{ type: 'text', text: await captureTrainingText(summary) }] }])
    }
    push(drafts, message, 'context_compacted', payload)
  }
}

interface BuiltCall {
  tool_call_id: string
  tool: string
  args: unknown
  terminal: boolean
  success: boolean
  output?: string
  exitCode?: number
  durationMs?: number
  waitMs?: number
  errorKind?: string
  decision: string
  feedback?: string
}

async function assistantContent(message: TraceMessage): Promise<{
  productive: boolean
  content: Record<string, unknown>[]
  toolUses: Record<string, unknown>[]
  calls: BuiltCall[]
  responseText: string
  reasoning: string
}> {
  const content: Record<string, unknown>[] = []
  const toolUses: Record<string, unknown>[] = []
  const calls: BuiltCall[] = []
  const texts: string[] = []
  const reasoning: string[] = []
  message.parts.forEach((part, index) => {
    const record = asRecord(part)
    if (!record) return
    const type = typeof record.type === 'string' ? record.type : ''
    if (type === 'text' && typeof record.text === 'string') {
      texts.push(record.text)
      return
    }
    if (type === 'data-code') {
      const data = asRecord(record.data) ?? record
      const code = firstString(data, 'content')
      if (code) texts.push(code)
      return
    }
    if (type === 'reasoning' && typeof record.text === 'string') {
      reasoning.push(record.text)
      return
    }
    const call = toolCall(record, message, index)
    if (!call) return
    calls.push(call)
    toolUses.push({ type: 'tool_use', id: call.tool_call_id, name: call.tool, input: call.args })
  })
  for (const text of texts) content.push({ type: 'text', text: await captureTrainingText(text) })
  return {
    productive: content.length > 0 || calls.length > 0 || reasoning.length > 0,
    content,
    toolUses,
    calls,
    responseText: texts.join(''),
    reasoning: reasoning.join('')
  }
}

function toolCall(part: Record<string, unknown>, message: TraceMessage, index: number): BuiltCall | null {
  const named = toolName(part)
  if (!named) return null
  const state = typeof part.state === 'string' ? part.state : ''
  const gate = approvalGate(part)
  const denied = gate.decision === 'denied'
  const terminal = state === 'output-available' || state === 'output-error' || denied
  const exitCode = exitCodeOf(part)
  const success = exitCode !== undefined ? exitCode === 0 : denied ? false : state === 'output-available'
  const output = observedOutput(part) ?? (denied ? gate.feedback : undefined)
  const errorKind = errorKindOf(part)
  const toolCallId = typeof part.toolCallId === 'string' && part.toolCallId ? part.toolCallId : `${message.id}:${index}`
  return {
    tool_call_id: toolCallId,
    tool: named,
    args: part.input === undefined ? {} : redactValue(part.input),
    terminal,
    success,
    output,
    exitCode,
    durationMs: spanDuration(message.stats, toolCallId, 'tool-execution'),
    waitMs: spanDuration(message.stats, toolCallId, 'approval-wait'),
    errorKind,
    decision: gate.decision ?? 'invoked',
    feedback: gate.feedback
  }
}

function approvalGate(part: Record<string, unknown>): {
  decision?: 'awaiting_user' | 'approved' | 'denied'
  feedback?: string
} {
  const approval = asRecord(part.approval)
  const state = typeof part.state === 'string' ? part.state : ''
  const reason =
    approval && typeof approval.reason === 'string' && approval.reason.trim() ? approval.reason.trim() : undefined
  if (approval && approval.approved === false) return { decision: 'denied', feedback: reason }
  if (approval && approval.approved === true) return { decision: 'approved', feedback: reason }
  if (state === 'approval-requested') return { decision: 'awaiting_user', feedback: reason }
  return { feedback: reason }
}

function modelDecision(built: {
  calls: readonly unknown[]
  responseText: string
  reasoning: string
}): string | undefined {
  if (built.calls.length > 0) return 'tool_use'
  if (built.responseText) return 'reply'
  if (built.reasoning) return 'reasoning'
  return undefined
}

function stopReasonOf(message: TraceMessage): string | undefined {
  for (const part of message.parts) {
    const record = asRecord(part)
    if (!record) continue
    const direct = firstString(record, 'finishReason', 'stopReason', 'stop_reason')
    if (direct) return direct
    const data = asRecord(record.data)
    const nested = data ? firstString(data, 'finishReason', 'stopReason', 'stop_reason') : ''
    if (nested) return nested
  }
  const stats = asRecord(message.stats)
  if (!stats) return undefined
  return firstString(stats, 'finishReason', 'stopReason', 'stop_reason') || undefined
}

function stepSignals(input: {
  decision?: string
  durationMs?: number
  feedback?: string
  stopReason?: string
  exitCode?: number
  errorKind?: string
  success?: boolean
}): Record<string, unknown> | undefined {
  const signals: Record<string, unknown> = {}
  if (input.decision) signals.decision = input.decision
  if (typeof input.durationMs === 'number') signals.duration_ms = input.durationMs
  if (input.feedback) {
    const redacted = redactTrainingText(input.feedback)
    signals.feedback = truncateUtf8(new TextEncoder().encode(redacted), RL_TRAJECTORY_MAX_TEXT_BYTES).text
  }
  if (input.stopReason) signals.stop_reason = input.stopReason
  if (typeof input.exitCode === 'number') signals.exit_code = input.exitCode
  if (input.errorKind) signals.error_kind = input.errorKind
  if (typeof input.success === 'boolean') signals.success = input.success
  return Object.keys(signals).length > 0 ? signals : undefined
}

function derivedDecision(record: TrainingRecord): string {
  const payload = record.payload
  if (record.event_type === 'llm_response') {
    const calls = payload.tool_calls
    if (Array.isArray(calls) && calls.length > 0) return 'tool_use'
    if (capturedBody(payload.response_text)) return 'reply'
    if (capturedBody(payload.reasoning_content)) return 'reasoning'
    return ''
  }
  if (record.event_type === 'tool_call') return 'invoked'
  if (record.event_type === 'tool_result') {
    if (payload.success === false) return 'failed'
    if (payload.success === true) return 'succeeded'
    return ''
  }
  if (record.event_type === 'execution_end') {
    if (payload.success === false) return 'stopped'
    if (payload.success === true) return 'finished'
  }
  return ''
}

function textOf(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function roleOf(message: TraceMessage): 'user' | 'assistant' | 'system' | 'other' {
  const role = message.role?.trim().toLowerCase()
  if (role === 'user' || role === 'assistant' || role === 'system') return role
  if (
    message.parts.some(
      (part) => toolName(asRecord(part) ?? {}) || partType(part) === 'reasoning' || partType(part) === 'data-error'
    )
  ) {
    return 'assistant'
  }
  if (message.parts.some((part) => partType(part) === 'text' || partType(part) === 'data-code')) return 'user'
  return 'other'
}

function toolName(part: Record<string, unknown>): string {
  const type = typeof part.type === 'string' ? part.type : ''
  if (type === 'dynamic-tool') {
    return typeof part.toolName === 'string' && part.toolName.trim() ? part.toolName.trim() : 'tool'
  }
  if (type.startsWith('tool-')) return type.slice('tool-'.length) || 'tool'
  return ''
}

function observedOutput(part: Record<string, unknown>): string | undefined {
  if (typeof part.output === 'string') return part.output
  const output = asRecord(part.output)
  if (output) {
    if (typeof output.value === 'string') return output.value
    if (output.value !== undefined) return JSON.stringify(output.value)
  }
  if (typeof part.errorText === 'string') return part.errorText
  if (typeof part.result === 'string') return part.result
  return undefined
}

function exitCodeOf(part: Record<string, unknown>): number | undefined {
  if (typeof part.exitCode === 'number') return part.exitCode
  if (typeof part.exit_code === 'number') return part.exit_code
  const output = asRecord(part.output)
  if (!output) return undefined
  if (typeof output.exitCode === 'number') return output.exitCode
  if (typeof output.exit_code === 'number') return output.exit_code
  return undefined
}

function errorKindOf(part: Record<string, unknown>): string | undefined {
  if (typeof part.errorKind === 'string' && part.errorKind) return part.errorKind
  const error = asRecord(part.error)
  if (error && typeof error.name === 'string' && error.name) return error.name
  return undefined
}

function errorTexts(parts: readonly unknown[]): string[] {
  const texts: string[] = []
  for (const part of parts) {
    if (partType(part) !== 'data-error') continue
    const record = asRecord(part)
    if (!record) continue
    const data = asRecord(record.data) ?? record
    const text = firstString(data, 'message', 'error') || (typeof record.errorText === 'string' ? record.errorText : '')
    if (text) texts.push(text)
  }
  return texts
}

function usageOf(stats: unknown): Record<string, number> | undefined {
  const record = asRecord(stats)
  if (!record) return undefined
  if (
    typeof record.inputTokens !== 'number' ||
    typeof record.outputTokens !== 'number' ||
    typeof record.totalTokens !== 'number'
  ) {
    return undefined
  }
  const usage: Record<string, number> = {
    prompt_tokens: record.inputTokens,
    completion_tokens: record.outputTokens,
    total_tokens: record.totalTokens
  }
  const details = asRecord(record.inputTokenDetails)
  if (details && typeof details.cacheReadTokens === 'number') usage.cache_read_tokens = details.cacheReadTokens
  if (details && typeof details.cacheWriteTokens === 'number') usage.cache_write_tokens = details.cacheWriteTokens
  return usage
}

function durationMs(stats: unknown): number | undefined {
  const record = asRecord(stats)
  if (!record) return undefined
  const timing = asRecord(record.runtimeTiming)
  if (
    timing &&
    typeof timing.startedAt === 'number' &&
    typeof timing.completedAt === 'number' &&
    timing.completedAt >= timing.startedAt
  ) {
    return timing.completedAt - timing.startedAt
  }
  if (typeof record.timeCompletionMs === 'number' && record.timeCompletionMs >= 0) return record.timeCompletionMs
  const performance = asRecord(record.providerPerformance)
  if (performance && typeof performance.generationDurationMs === 'number' && performance.generationDurationMs >= 0) {
    return performance.generationDurationMs
  }
  return undefined
}

function spanDuration(
  stats: unknown,
  toolCallId: string,
  kind: 'tool-execution' | 'approval-wait'
): number | undefined {
  if (!toolCallId) return undefined
  const timing = asRecord(asRecord(stats)?.runtimeTiming)
  const spans = timing?.spans
  if (!Array.isArray(spans)) return undefined
  for (const span of spans) {
    const record = asRecord(span)
    if (!record || record.kind !== kind || record.toolCallId !== toolCallId) continue
    if (
      typeof record.startedAt === 'number' &&
      typeof record.completedAt === 'number' &&
      record.completedAt >= record.startedAt
    ) {
      return record.completedAt - record.startedAt
    }
  }
  return undefined
}

function joinText(parts: readonly unknown[]): string {
  const texts: string[] = []
  for (const part of parts) {
    const record = asRecord(part)
    if (!record) continue
    const type = typeof record.type === 'string' ? record.type : ''
    if (type === 'text' && typeof record.text === 'string') texts.push(record.text)
    if (type === 'data-code') {
      const code = firstString(asRecord(record.data) ?? record, 'content')
      if (code) texts.push(code)
    }
  }
  return texts.join('')
}

async function textBlocks(parts: readonly unknown[]): Promise<Record<string, unknown>[]> {
  const blocks: Record<string, unknown>[] = []
  for (const part of parts) {
    const record = asRecord(part)
    if (!record) continue
    const type = typeof record.type === 'string' ? record.type : ''
    const text =
      type === 'text' && typeof record.text === 'string'
        ? record.text
        : type === 'data-code'
          ? firstString(asRecord(record.data) ?? record, 'content')
          : ''
    if (!text) continue
    blocks.push({ type: 'text', text: await captureTrainingText(text) })
  }
  return blocks
}

function envelope(sessionId: string, draft: Draft, sequence: number): StoredTrainingEvent {
  const timestamp = Date.parse(draft.at)
  return {
    messageId: draft.messageId,
    record: {
      schema: RL_TRAJECTORY_SCHEMA,
      sequence,
      timestamp_ms: Number.isFinite(timestamp) ? timestamp : 0,
      event_type: draft.eventType,
      session_id: sessionId,
      mode: 'on',
      context: {},
      payload: defined(draft.payload)
    }
  }
}

function push(
  drafts: Draft[],
  message: TraceMessage | undefined,
  eventType: RlEventType,
  payload: Record<string, unknown>
): void {
  if (!message?.id) return
  drafts.push({ messageId: message.id, at: message.createdAt || message.updatedAt, eventType, payload })
}

function ordered(messages: readonly TraceMessage[]): TraceMessage[] {
  return messages
    .filter((message) => message.id)
    .slice()
    .sort((left, right) => left.createdAt.localeCompare(right.createdAt) || left.id.localeCompare(right.id))
}

function indexed(messages: readonly CapturedMessage[]): CapturedMessage[] {
  return messages.map((message, index) => ({ ...message, index }))
}

function defined(fields: Record<string, unknown>): Record<string, unknown> {
  const payload: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(fields)) {
    if (value !== undefined) payload[key] = value
  }
  return payload
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  return value as Record<string, unknown>
}

function partType(part: unknown): string {
  const record = asRecord(part)
  return record && typeof record.type === 'string' ? record.type : ''
}

function firstString(record: Record<string, unknown>, ...keys: string[]): string {
  for (const key of keys) {
    const value = record[key]
    if (typeof value === 'string' && value) return value
  }
  return ''
}

function numberField(record: Record<string, unknown>, ...keys: string[]): number | undefined {
  for (const key of keys) {
    const value = record[key]
    if (typeof value === 'number') return value
  }
  return undefined
}

function argText(value: unknown): string {
  const record = asRecord(value)
  if (!record) return typeof value === 'string' ? value : ''
  for (const key of ARG_KEYS) {
    const item = record[key]
    if (typeof item === 'string' && item.trim()) return item
  }
  return ''
}

function capturedBody(value: unknown): string {
  const record = asRecord(value)
  if (!record || typeof record.text !== 'string') return ''
  return record.text
}

function redactTrainingText(value: string): string {
  return value.replace(/\b(?:sk-[A-Za-z0-9]{8,}|[a-f0-9]{32,})\b/gi, '…')
}

function redactValue(value: unknown): unknown {
  if (typeof value === 'string') return redactTrainingText(value)
  if (Array.isArray(value)) return value.map((item) => redactValue(item))
  const record = asRecord(value)
  if (!record) return value
  const redacted: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(record)) redacted[key] = redactValue(item)
  return redacted
}

function truncateUtf8(bytes: Uint8Array, maxBytes: number): { text: string; truncated: boolean } {
  if (bytes.length <= maxBytes) return { text: new TextDecoder().decode(bytes), truncated: false }
  let end = maxBytes
  while (end > 0 && (bytes[end] & 0xc0) === 0x80) end -= 1
  return { text: new TextDecoder().decode(bytes.slice(0, end)), truncated: true }
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}
