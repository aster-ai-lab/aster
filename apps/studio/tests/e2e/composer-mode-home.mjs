// Mouse-safe drive of the homepage composer mode. It runs the renderer
// test, which injects events inside the page. It does not move the pointer.
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const env = { ...process.env }
delete env.A3S_GUI_ALLOW_MOUSE

const vitest = resolve(studio, 'node_modules/vitest/vitest.mjs')
const command = existsSync(vitest) ? process.execPath : 'pnpm'
const args = existsSync(vitest) ? [vitest] : ['exec', 'vitest']
args.push('run', '--project', 'renderer', 'src/renderer/pages/agents/__tests__/composerModeHome.e2e.test.tsx')

const result = spawnSync(command, args, { cwd: studio, env, encoding: 'utf8' })

const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`
for (const line of output.split('\n')) {
  if (line.startsWith('oracle.')) console.log(line)
}
const exitCode = result.status ?? 1
console.log(`oracle.vitestExit=${exitCode}`)
if (!output.includes('oracle.modelOutlineRequests=0')) {
  console.error('oracle.modelOutlineRequests=missing')
  process.exit(exitCode === 0 ? 1 : exitCode)
}
process.exit(exitCode)
