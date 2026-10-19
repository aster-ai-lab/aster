const COMMAND_FIELDS = ['command', 'cmd'] as const

/** A permission title that is already a question, not a tool id. */
export function isProseToolLabel(name: string): boolean {
  const text = name.trim()
  if (!text) return false
  return /[?？]/.test(text) || /是否|请确认|请选择/.test(text)
}

/** Shell text carried in arguments. */
export function commandFromArgs(args: unknown): string | undefined {
  if (!args || typeof args !== 'object' || Array.isArray(args)) return undefined
  const record = args as Record<string, unknown>
  for (const field of COMMAND_FIELDS) {
    const value = record[field]
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return undefined
}

/** Command named inside a question, such as the text after the last colon. */
export function commandInText(text: string): string | undefined {
  const trimmed = text.trim()
  if (!trimmed) return undefined
  const tick = trimmed.match(/`([^`\n]+)`/)
  if (tick?.[1]?.trim()) return tick[1].trim()
  const labeled = trimmed.match(/[：:]\s*([^：:?？\n]+?)\s*[?？]?\s*$/)
  const candidate = labeled?.[1]?.trim()
  if (!candidate || candidate.length > 120 || /[。！]/.test(candidate)) return undefined
  return candidate
}
