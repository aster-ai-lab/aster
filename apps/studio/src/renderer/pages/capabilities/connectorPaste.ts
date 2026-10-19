/**
 * Paste shape for Claude, Cursor, Claude Code, and VS Code mcp.json / JSONC.
 * Comments and trailing commas are accepted. `${env:}` and `${input:}` stay literal.
 * Every entry is checked with the form validator before any connector is returned.
 */
import {
  customConnectorBody,
  customConnectorIssues,
  type CustomConnectorBody,
  type CustomConnectorIssues,
  type CustomTransport
} from './customConnector'

const SLUG = /^[a-z0-9][a-z0-9-]{0,63}$/
const MAX_SERVERS = 32

export type ConnectorPasteCode =
  | 'invalid_json'
  | 'json_empty'
  | 'invalid_name'
  | 'missing_command'
  | 'missing_url'
  | 'unknown_transport'
  | 'invalid_url'
  | 'invalid_config'

export type ConnectorPasteResult =
  | { ok: true; connectors: CustomConnectorBody[] }
  | { ok: false; code: ConnectorPasteCode }

type JsonRecord = Record<string, unknown>

export function parseConnectorPaste(text: string, options?: { name?: string }): ConnectorPasteResult {
  let value: unknown
  try {
    value = parseJsonc(text)
  } catch (error) {
    return { ok: false, code: error instanceof JsoncEmpty ? 'json_empty' : 'invalid_json' }
  }
  return readDocument(value, options?.name)
}

/** Industry mcp.json for one connector the form can already save. */
export function formatConnectorPaste(body: CustomConnectorBody): string {
  const spec: JsonRecord =
    body.transport === 'stdio'
      ? {
          command: body.command ?? '',
          ...(body.args && body.args.length > 0 ? { args: body.args } : {}),
          ...(Object.keys(body.values).length > 0 ? { env: body.values } : {})
        }
      : {
          type: body.transport === 'sse' ? 'sse' : 'http',
          url: body.url ?? '',
          ...(Object.keys(body.values).length > 0 ? { headers: body.values } : {})
        }
  return JSON.stringify({ mcpServers: { [body.name]: spec } }, null, 2)
}

function readDocument(value: unknown, fallbackName: string | undefined): ConnectorPasteResult {
  const root = asRecord(value)
  if (!root) return { ok: false, code: 'invalid_config' }
  if ('mcpServers' in root) return readMap(root.mcpServers)
  if ('servers' in root) return readMap(root.servers)
  const mcp = asRecord(root.mcp)
  if ('mcp' in root) {
    if (!mcp || !('servers' in mcp)) return { ok: false, code: 'invalid_config' }
    return readMap(mcp.servers)
  }
  if (!isSpec(root)) return { ok: false, code: 'json_empty' }
  const named = specName(root, fallbackName)
  if (!named) return { ok: false, code: 'invalid_name' }
  return specToBody(named, root)
}

function readMap(value: unknown): ConnectorPasteResult {
  const map = asRecord(value)
  if (!map) return { ok: false, code: 'invalid_config' }
  const entries = Object.entries(map)
  if (entries.length === 0) return { ok: false, code: 'json_empty' }
  if (entries.length > MAX_SERVERS) return { ok: false, code: 'invalid_config' }
  const connectors: CustomConnectorBody[] = []
  const seen = new Set<string>()
  for (const [key, spec] of entries) {
    const name = normalizeSlug(key)
    if (!name) return { ok: false, code: 'invalid_name' }
    if (seen.has(name)) return { ok: false, code: 'invalid_config' }
    seen.add(name)
    const parsed = specToBody(name, spec)
    if (!parsed.ok) return parsed
    connectors.push(parsed.connectors[0])
  }
  return { ok: true, connectors }
}

function specToBody(name: string, spec: unknown): ConnectorPasteResult {
  const row = asRecord(spec)
  if (!row) return { ok: false, code: 'invalid_config' }
  const transport = resolveTransport(row)
  if (transport === 'bad' || transport === 'missing') return { ok: false, code: 'unknown_transport' }
  if (transport === 'stdio') return stdioBody(name, row)
  return remoteBody(name, transport, row)
}

function resolveTransport(row: JsonRecord): CustomTransport | 'bad' | 'missing' {
  if (!('type' in row) || row.type === undefined) {
    if (row.command !== undefined) return 'stdio'
    if (row.url !== undefined) return 'streamable_http'
    return 'missing'
  }
  if (typeof row.type !== 'string') return 'bad'
  const token = row.type.trim().toLowerCase().replace(/[_-]/g, '')
  if (token === 'stdio' || token === 'local' || token === 'standardio') return 'stdio'
  if (token === 'sse') return 'sse'
  if (token === 'http' || token === 'streamablehttp') return 'streamable_http'
  return 'bad'
}

function stdioBody(name: string, row: JsonRecord): ConnectorPasteResult {
  const argsResult = row.args === undefined ? { ok: true as const, args: [] as string[] } : parseArgs(row.args)
  if (!argsResult.ok) return { ok: false, code: 'invalid_config' }
  let command = ''
  let args = argsResult.args
  if (row.command !== undefined) {
    const parsed = parseCommand(row.command, args)
    if (!parsed.ok) return { ok: false, code: 'invalid_config' }
    command = parsed.command
    args = parsed.args
  }
  const values =
    row.env === undefined ? { ok: true as const, values: {} as Record<string, string> } : stringMap(row.env)
  if (!values.ok) return { ok: false, code: 'invalid_config' }
  return finish({
    name,
    transport: 'stdio',
    command,
    args: args.join('\n'),
    values: toFields(values.values)
  })
}

function remoteBody(name: string, transport: 'sse' | 'streamable_http', row: JsonRecord): ConnectorPasteResult {
  if (row.url !== undefined && typeof row.url !== 'string') return { ok: false, code: 'invalid_config' }
  const values =
    row.headers === undefined ? { ok: true as const, values: {} as Record<string, string> } : stringMap(row.headers)
  if (!values.ok) return { ok: false, code: 'invalid_config' }
  return finish({
    name,
    transport,
    url: typeof row.url === 'string' ? row.url : '',
    values: toFields(values.values)
  })
}

function finish(input: {
  name: string
  transport: CustomTransport
  command?: string
  args?: string
  url?: string
  values: Array<{ key: string; value: string }>
}): ConnectorPasteResult {
  const body = customConnectorBody(input)
  if (body) return { ok: true, connectors: [body] }
  return { ok: false, code: issueCode(customConnectorIssues(input)) }
}

function issueCode(issues: CustomConnectorIssues): ConnectorPasteCode {
  return (
    issues.name ||
    issues.transport ||
    issues.command ||
    issues.args ||
    issues.url ||
    issues.config ||
    issues.fields.find((item): item is 'invalid_config' => Boolean(item)) ||
    'invalid_config'
  )
}

function parseArgs(value: unknown): { ok: true; args: string[] } | { ok: false } {
  if (typeof value === 'string') {
    return {
      ok: true,
      args: value
        .split(/\r?\n/)
        .map((item) => item.trim())
        .filter((item) => item.length > 0)
    }
  }
  if (!Array.isArray(value)) return { ok: false }
  const args: string[] = []
  for (const item of value) {
    if (typeof item !== 'string' || item.includes('\0') || item.includes('\n') || item.includes('\r')) {
      return { ok: false }
    }
    const trimmed = item.trim()
    if (trimmed) args.push(trimmed)
  }
  return { ok: true, args }
}

function parseCommand(value: unknown, args: string[]): { ok: true; command: string; args: string[] } | { ok: false } {
  if (typeof value === 'string') return { ok: true, command: value.trim(), args }
  if (!Array.isArray(value)) return { ok: false }
  const parts: string[] = []
  for (const item of value) {
    if (typeof item !== 'string' || item.includes('\0')) return { ok: false }
    const trimmed = item.trim()
    if (trimmed) parts.push(trimmed)
  }
  const [command, ...rest] = parts
  return { ok: true, command: command ?? '', args: [...rest, ...args] }
}

function stringMap(value: unknown): { ok: true; values: Record<string, string> } | { ok: false } {
  const row = asRecord(value)
  if (!row) return { ok: false }
  const values: Record<string, string> = {}
  for (const [key, raw] of Object.entries(row)) {
    if (typeof raw !== 'string') return { ok: false }
    if (!raw) continue
    Object.defineProperty(values, key, { value: raw, enumerable: true, writable: true, configurable: true })
  }
  return { ok: true, values }
}

function toFields(values: Record<string, string>): Array<{ key: string; value: string }> {
  return Object.entries(values).map(([key, value]) => ({ key, value }))
}

function isSpec(row: JsonRecord): boolean {
  return 'command' in row || 'args' in row || 'env' in row || 'url' in row || 'headers' in row || 'type' in row
}

function specName(row: JsonRecord, fallbackName: string | undefined): string | null {
  if ('name' in row && row.name !== undefined) {
    return typeof row.name === 'string' ? normalizeSlug(row.name) : null
  }
  return normalizeSlug(fallbackName)
}

function normalizeSlug(raw: string | undefined): string | null {
  if (typeof raw !== 'string') return null
  const trimmed = raw.trim()
  if (SLUG.test(trimmed)) return trimmed
  const lower = trimmed.toLowerCase()
  if (SLUG.test(lower)) return lower
  return null
}

function asRecord(value: unknown): JsonRecord | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  return value as JsonRecord
}

class JsoncEmpty extends Error {}

class JsoncParser {
  private index = 0

  constructor(private readonly source: string) {}

  parse(): unknown {
    this.skip()
    if (this.done()) throw new JsoncEmpty()
    const value = this.value()
    this.skip()
    if (!this.done()) throw new Error('trailing')
    return value
  }

  private done(): boolean {
    return this.index >= this.source.length
  }

  private peek(): string {
    return this.source[this.index] ?? ''
  }

  private skip(): void {
    while (!this.done()) {
      const char = this.peek()
      if (char === ' ' || char === '\n' || char === '\r' || char === '\t') {
        this.index += 1
        continue
      }
      if (char === '/' && this.source[this.index + 1] === '/') {
        this.index += 2
        while (!this.done() && this.peek() !== '\n') this.index += 1
        continue
      }
      if (char === '/' && this.source[this.index + 1] === '*') {
        const end = this.source.indexOf('*/', this.index + 2)
        if (end < 0) throw new Error('comment')
        this.index = end + 2
        continue
      }
      return
    }
  }

  private value(): unknown {
    const char = this.peek()
    if (char === '{') return this.object()
    if (char === '[') return this.array()
    if (char === '"') return this.string()
    if (char === 't') return this.word('true', true)
    if (char === 'f') return this.word('false', false)
    if (char === 'n') return this.word('null', null)
    if (char === '-' || (char >= '0' && char <= '9')) return this.number()
    throw new Error('value')
  }

  private object(): JsonRecord {
    this.index += 1
    const out: JsonRecord = {}
    this.skip()
    if (this.peek() === '}') {
      this.index += 1
      return out
    }
    for (;;) {
      if (this.peek() !== '"') throw new Error('key')
      const key = this.string()
      this.skip()
      if (this.peek() !== ':') throw new Error('colon')
      this.index += 1
      this.skip()
      this.assign(out, key, this.value())
      this.skip()
      if (this.peek() === ',') {
        this.index += 1
        this.skip()
        if (this.peek() === '}') {
          this.index += 1
          return out
        }
        continue
      }
      if (this.peek() === '}') {
        this.index += 1
        return out
      }
      throw new Error('object')
    }
  }

  private array(): unknown[] {
    this.index += 1
    const out: unknown[] = []
    this.skip()
    if (this.peek() === ']') {
      this.index += 1
      return out
    }
    for (;;) {
      out.push(this.value())
      this.skip()
      if (this.peek() === ',') {
        this.index += 1
        this.skip()
        if (this.peek() === ']') {
          this.index += 1
          return out
        }
        continue
      }
      if (this.peek() === ']') {
        this.index += 1
        return out
      }
      throw new Error('array')
    }
  }

  private string(): string {
    this.index += 1
    let out = ''
    while (!this.done()) {
      const char = this.peek()
      if (char === '"') {
        this.index += 1
        return out
      }
      if (char === '\\') {
        this.index += 1
        const escaped = this.peek()
        this.index += 1
        if (escaped === '"' || escaped === '\\' || escaped === '/') out += escaped
        else if (escaped === 'b') out += '\b'
        else if (escaped === 'f') out += '\f'
        else if (escaped === 'n') out += '\n'
        else if (escaped === 'r') out += '\r'
        else if (escaped === 't') out += '\t'
        else if (escaped === 'u') {
          const hex = this.source.slice(this.index, this.index + 4)
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) throw new Error('hex')
          out += String.fromCharCode(Number.parseInt(hex, 16))
          this.index += 4
        } else throw new Error('escape')
        continue
      }
      if (char.charCodeAt(0) < 0x20) throw new Error('control')
      out += char
      this.index += 1
    }
    throw new Error('string')
  }

  private word(expected: string, value: unknown): unknown {
    if (this.source.slice(this.index, this.index + expected.length) !== expected) throw new Error('word')
    this.index += expected.length
    return value
  }

  private number(): number {
    const match = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(this.source.slice(this.index))
    if (!match) throw new Error('number')
    this.index += match[0].length
    const parsed = JSON.parse(match[0]) as number
    if (!Number.isFinite(parsed)) throw new Error('number')
    return parsed
  }

  private assign(target: JsonRecord, key: string, value: unknown): void {
    Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true })
  }
}

function parseJsonc(text: string): unknown {
  return new JsoncParser(text.replace(/^\uFEFF/, '')).parse()
}
