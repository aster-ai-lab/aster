/** A host transport the agent session can run. In-memory connectors are not one of these. */
export type CustomTransport = 'stdio' | 'sse' | 'streamable_http'

export type CustomConnectorBody = {
  name: string
  transport: CustomTransport
  command?: string
  args?: string[]
  url?: string
  values: Record<string, string>
}

export type CustomConnectorField = {
  key: string
  value: string
}

export type CustomConnectorInput = {
  name: string
  transport: string
  command?: string
  args?: string
  url?: string
  values?: readonly CustomConnectorField[]
}

const SLUG = /^[a-z0-9][a-z0-9-]{0,63}$/
const CONFIG_KEY = /^[A-Za-z_][A-Za-z0-9_-]{0,63}$/
const MAX_COMMAND = 1024
const MAX_ARG = 1024
const MAX_ARGS = 32
const MAX_URL = 2048
const MAX_VALUE = 4096
const MAX_FIELDS = 32

export type CustomConnectorIssueCode =
  | 'invalid_name'
  | 'missing_command'
  | 'missing_url'
  | 'unknown_transport'
  | 'invalid_url'
  | 'invalid_config'

/** One issue per visible field. `fields` lines up with `input.values`. */
export type CustomConnectorIssues = {
  name: 'invalid_name' | null
  command: 'missing_command' | 'invalid_config' | null
  args: 'invalid_config' | null
  url: 'missing_url' | 'invalid_url' | null
  transport: 'unknown_transport' | null
  config: 'invalid_config' | null
  fields: Array<'invalid_config' | null>
}

function bounded(text: string, max: number): boolean {
  return text.length > 0 && text.length <= max && !text.includes('\0')
}

function urlOk(url: string): boolean {
  if (!bounded(url, MAX_URL) || /\s/.test(url)) return false
  try {
    const parsed = new URL(url)
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return false
    return parsed.hostname.length > 0
  } catch {
    return false
  }
}

function emptyIssues(count: number): CustomConnectorIssues {
  return {
    name: null,
    command: null,
    args: null,
    url: null,
    transport: null,
    config: null,
    fields: Array.from({ length: count }, () => null)
  }
}

function blocked(issues: CustomConnectorIssues): boolean {
  return Boolean(
    issues.name ||
    issues.command ||
    issues.args ||
    issues.url ||
    issues.transport ||
    issues.config ||
    issues.fields.some(Boolean)
  )
}

function parseCustomConnector(input: CustomConnectorInput): {
  body: CustomConnectorBody | null
  issues: CustomConnectorIssues
} {
  const rows = input.values ?? []
  const issues = emptyIssues(rows.length)
  const name = input.name.trim()
  if (!SLUG.test(name)) issues.name = 'invalid_name'
  const known = input.transport === 'stdio' || input.transport === 'sse' || input.transport === 'streamable_http'
  if (!known) issues.transport = 'unknown_transport'

  const values: Record<string, string> = {}
  rows.forEach((row, index) => {
    const key = row.key.trim()
    if (!key && !row.value) return
    if (!CONFIG_KEY.test(key) || row.value.length > MAX_VALUE || row.value.includes('\0')) {
      issues.fields[index] = 'invalid_config'
      return
    }
    if (row.value) values[key] = row.value
  })
  if (Object.keys(values).length > MAX_FIELDS) issues.config = 'invalid_config'

  if (input.transport === 'stdio') {
    const command = (input.command ?? '').trim()
    if (!command) issues.command = 'missing_command'
    else if (!bounded(command, MAX_COMMAND)) issues.command = 'invalid_config'
    const args = (input.args ?? '')
      .split('\n')
      .map((item) => item.trim())
      .filter((item) => item.length > 0)
    if (args.length > MAX_ARGS || args.some((item) => !bounded(item, MAX_ARG))) issues.args = 'invalid_config'
    if (blocked(issues)) return { body: null, issues }
    return { body: { name, transport: 'stdio', command, args, values }, issues }
  }

  if (input.transport === 'sse' || input.transport === 'streamable_http') {
    const url = (input.url ?? '').trim()
    if (!url) issues.url = 'missing_url'
    else if (!urlOk(url)) issues.url = 'invalid_url'
    if (blocked(issues)) return { body: null, issues }
    return { body: { name, transport: input.transport, url, values }, issues }
  }

  return { body: null, issues }
}

/** Field-level reasons the draft cannot be saved. A null field is valid so far. */
export function customConnectorIssues(input: CustomConnectorInput): CustomConnectorIssues {
  return parseCustomConnector(input).issues
}

/**
 * The body the shell accepts, or null when the draft must not be saved.
 * Empty names, a stdio connector without a command, a remote connector without
 * an http(s) URL, and any other transport are refused.
 */
export function customConnectorBody(input: CustomConnectorInput): CustomConnectorBody | null {
  return parseCustomConnector(input).body
}

/** One stored custom connector, mapped back from `mcp.json` type `http` to streamable HTTP. */
export function customConnectorRecord(value: unknown): CustomConnectorBody | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.name !== 'string' || typeof row.transport !== 'string') return null
  const values = asValues(row.values)
  if (!values) return null
  if (row.transport === 'stdio') {
    const command = typeof row.command === 'string' ? row.command : ''
    const args = Array.isArray(row.args) ? row.args.filter((item): item is string => typeof item === 'string') : []
    return customConnectorBody({
      name: row.name,
      transport: 'stdio',
      command,
      args: args.join('\n'),
      values: Object.entries(values).map(([key, field]) => ({ key, value: field }))
    })
  }
  if (row.transport !== 'sse' && row.transport !== 'streamable_http') return null
  return customConnectorBody({
    name: row.name,
    transport: row.transport,
    url: typeof row.url === 'string' ? row.url : '',
    values: Object.entries(values).map(([key, field]) => ({ key, value: field }))
  })
}

function asValues(value: unknown): Record<string, string> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const out: Record<string, string> = {}
  for (const [key, raw] of Object.entries(value)) {
    if (typeof raw !== 'string') return null
    out[key] = raw
  }
  return out
}
