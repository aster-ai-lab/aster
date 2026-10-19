/* global console, process */
/**
 * Desktop smoke for the Studio host.
 * Run from the OS checkout (`node apps/studio/scripts/smoke-desktop.mjs`)
 * or from the studio checkout (`node scripts/smoke-desktop.mjs`).
 * The host binary is the one that reads A3S_STUDIO_DESKTOP_SMOKE.
 */
import { execFileSync, spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const studioRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const manifest = path.join(studioRoot, 'src-tauri', 'Cargo.toml')
const binaryName = process.platform === 'win32' ? 'Studio.exe' : 'Studio'
const metadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'], {
    cwd: studioRoot,
    encoding: 'utf8'
  })
)
const binary = path.join(metadata.target_directory, 'debug', binaryName)

const expectedPlatform = { win32: 'win32', linux: 'linux', darwin: 'darwin' }[process.platform]
const expectedProgram = { win32: 'rundll32.exe', linux: 'xdg-open', darwin: 'open' }[process.platform]

function emit(step, result) {
  console.log(`smoke-desktop ${step} ${result}`)
}

function stop(step, result) {
  emit(step, result)
  process.exit(1)
}

if (!expectedPlatform || !expectedProgram) stop('build', `unsupported-platform-${process.platform}`)

const build = spawnSync('cargo', ['build', '--locked', '--manifest-path', manifest, '--bin', 'Studio'], {
  cwd: studioRoot,
  stdio: 'inherit'
})
if (build.error) {
  console.error(build.error.message)
  stop('build', 'failed')
}
if (build.status !== 0) stop('build', 'failed')
emit('build', 'ok')

const host = spawnSync(binary, [], {
  cwd: studioRoot,
  env: { ...process.env, A3S_STUDIO_DESKTOP_SMOKE: '1' },
  encoding: 'utf8',
  timeout: 120_000,
  windowsHide: false
})
const output = `${host.stdout ?? ''}\n${host.stderr ?? ''}`
if (host.error) console.error(host.error.message)

const lines = output
  .split(/\r?\n/)
  .map((line) => line.trim())
  .filter(Boolean)
if (!lines.some((line) => line.startsWith('studio-webview-url '))) stop('platform', 'missing-url')

let capabilities
for (const line of lines) {
  if (!line.startsWith('{')) continue
  try {
    const parsed = JSON.parse(line)
    if (parsed && typeof parsed === 'object' && typeof parsed.platform === 'string') capabilities = parsed
  } catch {
    // Log lines that look like JSON but are not the capability object.
  }
}
if (!capabilities) stop('platform', 'missing')
if (capabilities.platform !== expectedPlatform) stop('platform', capabilities.platform)
emit('platform', capabilities.platform)

let windowResult = 'failed'
if (lines.includes('skipped-no-display')) windowResult = 'skipped-no-display'
else if (lines.includes('smoke-window closed')) windowResult = 'closed'
emit('window', windowResult)

const openLine = lines.find((line) => line.startsWith('open_http_launch '))
const program = openLine?.split(/\s+/)[1]
if (program !== expectedProgram) stop('open', program || 'missing')
emit('open', program)

const directory = lines.find((line) => line === 'cancelled' || line === 'desktop-capability-unavailable')
if (!directory) stop('directory', 'missing')
emit('directory', directory)

if (windowResult === 'failed') process.exit(host.status && host.status !== 0 ? host.status : 1)
process.exit(0)
