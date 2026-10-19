import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { once } from 'node:events'
import { createInterface } from 'node:readline'

interface Reply {
  id: number
  status: number
  value?: unknown
}

/** Test transport only; every document command executes the actual native Rust modules. */
export class NativeWorkspaceDocuments {
  private readonly process: ChildProcessWithoutNullStreams
  private readonly pending = new Map<number, { resolve: (value: Reply) => void; reject: (error: Error) => void }>()
  private serial = 0
  private stderr = ''
  private failure?: Error
  readonly activeLeases = new Set<string>()

  constructor() {
    const executable = process.env.A3S_STUDIO_DOCUMENT_STDIO
    if (!executable) throw new Error('Run scripts/test-workspace-documents.ts to build the native fixture')
    this.process = spawn(executable, [], { stdio: 'pipe' })
    createInterface({ input: this.process.stdout }).on('line', (line) => {
      const value = JSON.parse(line) as Reply
      const pending = this.pending.get(value.id)
      this.pending.delete(value.id)
      pending?.resolve(value)
    })
    this.process.stderr.on('data', (chunk: Buffer) => {
      this.stderr = (this.stderr + chunk.toString()).slice(-64 * 1024)
    })
    this.process.on('error', (error) => this.reject(error))
    this.process.on('exit', (code) => this.reject(new Error(`Native fixture exited (${code}): ${this.stderr}`)))
  }

  command(request: unknown, bearer?: string): Promise<Reply> {
    return this.send({ request, bearer })
  }

  readonly fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const url = new URL(String(input))
    if (['/api/workspace-documents', '/api/plugin-documents'].includes(url.pathname) && init?.method === 'POST') {
      const command = JSON.parse(String(init.body)) as { operation: string; lease?: string }
      const bearer =
        url.pathname === '/api/plugin-documents'
          ? (new Headers(init.headers).get('Authorization')?.replace(/^Bearer /, '') ?? '')
          : undefined
      const reply = await this.command(command, bearer)
      if (reply.status === 200 && ['open', 'delegate', 'delegate_diagnostics'].includes(command.operation))
        this.activeLeases.add((reply.value as { lease: string }).lease)
      if (
        reply.status === 200 &&
        command.operation === 'close' &&
        (reply.value as { closed: boolean }).closed &&
        command.lease
      )
        this.activeLeases.delete(command.lease)
      if (reply.status === 404 && command.operation === 'close' && command.lease)
        this.activeLeases.delete(command.lease)
      return new globalThis.Response(JSON.stringify(reply.value ?? { error: 'refused' }), {
        status: reply.status,
        headers: { 'Content-Type': 'application/json' }
      })
    }
    if (url.pathname === '/api/workspace-file/snapshot' && !init?.method) {
      const reply = await this.send({
        disk: { root: url.searchParams.get('root'), path: url.searchParams.get('path') }
      })
      if (reply.status !== 200) return new globalThis.Response('', { status: reply.status })
      const value = reply.value as { bytes: number[]; revision: string }
      const bytes = Uint8Array.from(value.bytes)
      return new globalThis.Response(bytes, {
        headers: { ETag: `"${value.revision}"`, 'Content-Length': String(bytes.byteLength) }
      })
    }
    throw new Error(`Unqualified native fixture route: ${url.pathname}`)
  }

  async close(): Promise<void> {
    if (this.failure) throw this.failure
    const exited = once(this.process, 'exit')
    this.process.stdin.end()
    const deadline = globalThis.setTimeout(() => this.process.kill('SIGKILL'), 2000)
    try {
      const [code, signal] = await exited
      if (code !== 0 || signal) throw new Error(`Native fixture did not acknowledge exit: ${code}/${signal}`)
    } finally {
      globalThis.clearTimeout(deadline)
    }
  }

  private send(message: object): Promise<Reply> {
    if (this.failure) return Promise.reject(this.failure)
    const id = ++this.serial
    const reply = new Promise<Reply>((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
    })
    this.process.stdin.write(`${JSON.stringify({ id, ...message })}\n`, (error) => {
      if (error) this.reject(error)
    })
    return reply
  }

  private reject(error: Error): void {
    this.failure = error
    for (const pending of this.pending.values()) pending.reject(error)
    this.pending.clear()
  }
}
