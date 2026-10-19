import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const require = createRequire(resolve(root, 'packages/intelligent-ui/package.json'))
const { build } = require('esbuild')
const result = await build({
  entryPoints: [fileURLToPath(new URL(process.argv[3] ?? 'fixture.tsx', import.meta.url))],
  bundle: true,
  write: false,
  format: 'esm',
  platform: 'browser',
  jsx: 'automatic',
  target: 'es2022',
  alias: {
    '@a3s-lab/intelligent-ui/openui': resolve(root, 'packages/intelligent-ui/src/openui.ts'),
    '@a3s-lab/intelligent-ui': resolve(root, 'packages/intelligent-ui/src/IntelligentUI.tsx'),
    '@a3s-lab/ui': resolve(root, 'packages/ui/src'),
    '@shared': resolve(root, 'apps/studio/src/shared'),
    react: resolve(root, 'apps/studio/node_modules/react'),
    'react-dom': resolve(root, 'apps/studio/node_modules/react-dom')
  }
})
const server = createServer((req, res) => {
  if (req.url === '/fixture.js') {
    res.setHeader('content-type', 'text/javascript')
    res.end(result.outputFiles[0].text)
  } else {
    res.setHeader('content-type', 'text/html')
    res.end(
      '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div><script type="module" src="/fixture.js"></script></body></html>'
    )
  }
})
server.listen(Number(process.argv[2] ?? 4338), '127.0.0.1', () =>
  process.stdout.write(`http://127.0.0.1:${server.address().port}\n`)
)
