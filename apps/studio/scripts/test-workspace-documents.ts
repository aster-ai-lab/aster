import { spawnSync } from 'node:child_process'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const build = spawnSync(
  'rustup',
  [
    'run',
    '1.97.1',
    'cargo',
    'test',
    '--offline',
    '--lib',
    '--test',
    'workspace_document_stdio',
    '--no-run',
    '--message-format=json'
  ],
  {
    cwd: path.join(root, 'src-tauri'),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
    maxBuffer: 64 * 1024 * 1024
  }
)
if (build.error || build.status !== 0) {
  if (build.error) process.stderr.write(`${build.error.message}\n`)
  process.exit(build.status ?? 1)
}
let executable: string | undefined
let nativeTests: string | undefined
for (const line of build.stdout.split('\n')) {
  if (!line.trim()) continue
  const item = JSON.parse(line) as { reason?: string; target?: { name?: string }; executable?: string | null }
  if (item.reason === 'compiler-artifact' && item.target?.name === 'workspace_document_stdio' && item.executable)
    executable = item.executable
  if (item.reason === 'compiler-artifact' && item.target?.name === 'a3s_studio_lib' && item.executable)
    nativeTests = item.executable
}
if (!executable) throw new Error('Native document probe executable missing')
if (!nativeTests) throw new Error('Native authority test executable missing')
const result = spawnSync(
  process.execPath,
  [
    path.join(root, 'node_modules/vitest/vitest.mjs'),
    'run',
    '--config',
    'vitest.config.workspace-documents.ts',
    '--no-file-parallelism',
    ...process.argv.slice(2)
  ],
  {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, A3S_STUDIO_DOCUMENT_STDIO: executable, A3S_STUDIO_NATIVE_TEST: nativeTests }
  }
)
if (result.error) process.stderr.write(`${result.error.message}\n`)
process.exitCode = result.status ?? 1
