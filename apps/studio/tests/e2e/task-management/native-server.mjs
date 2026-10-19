/** Production renderer plus an owned real Rust agent plane; no task API replies are mocked. */
import assert from 'node:assert/strict'
import { Buffer } from 'node:buffer'
import { spawn } from 'node:child_process'
import { appendFileSync, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { extname, resolve, sep } from 'node:path'
import process from 'node:process'
import { setTimeout as delay } from 'node:timers/promises'
import { URL } from 'node:url'

const [executable, rendererDirectory, rawPort = '4596'] = process.argv.slice(2)
assert(executable && rendererDirectory, 'Usage: native-server.mjs <native-test-executable> <renderer-directory> [port]')
const port = Number(rawPort)
assert(Number.isSafeInteger(port) && port > 0 && port < 65536, 'Invalid loopback port')
const directory = mkdtempSync(resolve(tmpdir(), 'a3s-task-archive-'))
const descriptor = resolve(directory, 'runtime.json')
const native = spawn(executable, ['--exact', 'agent_plane::session_archive_tests::serve_archive_e2e', '--ignored'], {
  env: { ...process.env, A3S_CLOUD_TEST_ARCHIVE_RUNTIME: descriptor },
  stdio: ['pipe', 'ignore', 'inherit']
})
native.on('error', (error) => {
  process.stderr.write(`${error.message}\n`)
  process.exitCode = 1
})
const deadline = Date.now() + 15_000
while (!existsSync(descriptor)) {
  assert(native.exitCode === null && Date.now() < deadline, 'Native fixture failed to start')
  await delay(25)
}
const runtime = JSON.parse(readFileSync(descriptor, 'utf8'))
const nativeOrigin = new URL(runtime.baseUrl)
assert(nativeOrigin.hostname === '127.0.0.1' && nativeOrigin.protocol === 'http:', 'Fixture must be loopback')
const origin = `http://127.0.0.1:${port}`
const renderer = resolve(rendererDirectory)
const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
  '.json': 'application/json'
}
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, origin)
    if (url.pathname === '/api/agent-plane') {
      response.setHeader('content-type', 'application/json')
      response.end(JSON.stringify({ baseUrl: origin }))
      return
    }
    if (url.pathname.startsWith('/api/')) {
      const headers = new globalThis.Headers()
      if (request.headers.authorization) headers.set('authorization', request.headers.authorization)
      if (request.headers['content-type']) headers.set('content-type', request.headers['content-type'])
      const method = request.method ?? 'GET'
      const upstream = await globalThis.fetch(runtime.baseUrl + url.pathname + url.search, {
        method,
        headers,
        ...(method === 'GET' || method === 'HEAD' ? {} : { body: request, duplex: 'half' })
      })
      appendFileSync(
        resolve(directory, 'requests.jsonl'),
        JSON.stringify({ method, path: url.pathname, status: upstream.status }) + '\n'
      )
      response.writeHead(upstream.status, {
        'content-type': upstream.headers.get('content-type') ?? 'application/json'
      })
      if (url.pathname === '/api/agent-events') {
        const reader = upstream.body.getReader()
        response.on('close', () => {
          void reader.cancel()
        })
        while (!response.destroyed) {
          const chunk = await reader.read()
          if (chunk.done) break
          response.write(chunk.value)
        }
        response.end()
      } else response.end(Buffer.from(await upstream.arrayBuffer()))
      return
    }
    const relative = url.pathname === '/' ? '/windows/main/index.html' : decodeURIComponent(url.pathname)
    const file = resolve(renderer, `.${relative}`)
    if (!file.startsWith(renderer + sep)) {
      response.writeHead(404).end()
      return
    }
    let bytes = readFileSync(file)
    if (extname(file) === '.html') {
      const bootstrap = { apiBaseUrl: origin, token: runtime.token, homePath: runtime.homePath }
      bytes = Buffer.from(
        bytes
          .toString()
          .replace(
            '<head>',
            `<head><script>globalThis.__A3S_STUDIO_RUNTIME__=${JSON.stringify(bootstrap)};localStorage.setItem('a3s.settings.preferences',JSON.stringify({'app.language':'zh-CN'}));</script>`
          )
      )
    }
    response.setHeader('content-type', types[extname(file)] ?? 'application/octet-stream')
    response.end(bytes)
  } catch {
    if (!response.headersSent) response.writeHead(404)
    response.end()
  }
})
await new Promise((accept, reject) => {
  server.once('error', reject)
  server.listen(port, '127.0.0.1', accept)
})
process.stdout.write(`Archive E2E ready at ${origin}; fixture PID: ${process.pid}; isolated state: ${directory}\n`)
const close = () => {
  server.closeAllConnections()
  server.close()
  native.stdin.end()
}
process.once('SIGTERM', close)
process.once('SIGINT', close)
process.once('exit', () => {
  native.stdin.destroy()
  rmSync(descriptor, { force: true })
})
