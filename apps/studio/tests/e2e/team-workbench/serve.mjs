import { readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const studio = fileURLToPath(new URL('../../../', import.meta.url))
const require = createRequire(resolve(studio, '../../packages/intelligent-ui/package.json'))
const { build } = require('esbuild')
const result = await build({
  entryPoints: [fileURLToPath(new URL('fixture.tsx', import.meta.url))],
  bundle: true,
  write: false,
  format: 'esm',
  platform: 'browser',
  jsx: 'automatic',
  target: 'es2022',
  loader: { '.svg': 'dataurl' },
  alias: {
    '@renderer': resolve(studio, 'src/renderer'),
    '@shared': resolve(studio, 'src/shared'),
    react: resolve(studio, 'node_modules/react'),
    'react-dom': resolve(studio, 'node_modules/react-dom'),
    'react-i18next': resolve(studio, 'node_modules/react-i18next'),
    i18next: resolve(studio, 'node_modules/i18next')
  }
})
const html = await readFile(resolve(studio, '.studio-renderer/windows/main/index.html'), 'utf8')
const cssPaths = [...html.matchAll(/<link[^>]+href="([^"]+\.css)"[^>]*>/g)].map((match) => match[1])
const cssLinks = cssPaths.map((path) => `<link rel="stylesheet" href="${path}">`).join('')
// Keep the fixture independent of concurrent renderer rebuilds in the shared tree.
const styles = new Map(
  await Promise.all(
    cssPaths.map(async (path) => [path, await readFile(resolve(studio, '.studio-renderer', path.replace(/^\//, '')))])
  )
)
const token = 'ab'.repeat(32)
let items = []
let approver = false
const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://127.0.0.1')
  const json = (data, status = 200) => {
    res.writeHead(status, { 'content-type': 'application/json' })
    res.end(JSON.stringify({ code: 0, data }))
  }
  if (url.pathname === '/fixture.js') {
    res.setHeader('content-type', 'text/javascript')
    res.end(result.outputFiles.find((file) => file.path.endsWith('.js') || file.path === '<stdout>').text)
    return
  }
  if (url.pathname.endsWith('.css')) {
    res.setHeader('content-type', 'text/css')
    res.end(styles.get(url.pathname) ?? '')
    return
  }
  if (url.pathname === '/fixture/approver') {
    approver = true
    json({})
    return
  }
  if (url.pathname.startsWith('/api/member/')) {
    if (req.headers.authorization !== `Bearer ${token}`) {
      json(null, 401)
      return
    }
    if (url.pathname.endsWith('/summary')) {
      json({
        todo: approver ? items.filter((item) => item.status === 'pending').length : 0,
        done: items.filter((item) => item.status === 'approved').length,
        cc: 0,
        sent: items.length
      })
      return
    }
    if (req.method === 'POST') {
      let body = ''
      for await (const chunk of req) body += chunk
      const input = JSON.parse(body)
      if (url.pathname.endsWith('/decision')) {
        items = items.map((item) => ({
          ...item,
          status: input.decision === 'approve' ? 'approved' : 'rejected',
          actionable: false
        }))
        json(items[0])
        return
      }
      const item = {
        id: 'request-1',
        org_id: input.org_id,
        org_name: '测试团队',
        kind: input.kind,
        title: input.title,
        summary: input.summary,
        status: 'pending',
        initiator_name: '申请人',
        current_node: '审批',
        created_at: new Date().toISOString(),
        actionable: false,
        cancellable: true
      }
      items.push(item)
      json(item, 201)
      return
    }
    const box = url.searchParams.get('box')
    json(
      items
        .filter(
          (item) =>
            box === 'sent' ||
            (box === 'todo' && approver && item.status === 'pending') ||
            (box === 'done' && item.status !== 'pending')
        )
        .map((item) => ({
          ...item,
          actionable: approver && item.status === 'pending',
          approvable: approver && item.status === 'pending',
          rejectable: approver && item.status === 'pending',
          cancellable: !approver && item.status === 'pending'
        }))
    )
    return
  }
  const port = server.address().port
  res.setHeader('content-type', 'text/html')
  res.end(
    `<!doctype html><html><head><meta charset="utf-8">${cssLinks}<style>body{margin:0;background:white}</style></head><body><div id="root"></div><script>globalThis.__A3S_STUDIO_RUNTIME__={apiBaseUrl:'http://127.0.0.1:${port}',token:'${token}'}</script><script type="module" src="/fixture.js"></script></body></html>`
  )
})
server.listen(4388, '127.0.0.1', () => process.stdout.write('http://127.0.0.1:4388\n'))
