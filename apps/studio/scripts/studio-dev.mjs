/* global console, process, setTimeout */
/**
 * Starts the Tauri window and the Cherry renderer on 127.0.0.1:4318.
 * Port ownership and the runtime file use Node so the launch does not require a Unix /tmp.
 */
import { execFileSync, spawn } from 'node:child_process'
import { existsSync, openSync, readFileSync, rmSync } from 'node:fs'
import http from 'node:http'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { listenerCwd, portOwnedBy } from './renderer-owner.mjs'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
process.chdir(root)

const port = Number(process.env.STUDIO_RENDERER_PORT || 4318)
const rendererUrl = `http://127.0.0.1:${port}/windows/main/index.html`
const rsbuild = path.join('node_modules', '@rsbuild', 'core', 'bin', 'rsbuild.js')

function runtimePath() {
  if (process.env.STUDIO_RUNTIME_FILE) return process.env.STUDIO_RUNTIME_FILE
  const slug = process.env.A3S_TENANT_SLUG || 'studio'
  if (process.env.A3S_STUDIO_INSTANCE === 'fork') {
    return path.join(os.tmpdir(), `a3s-studio-runtime-${slug}-fork-${process.pid}.json`)
  }
  return path.join(os.tmpdir(), 'a3s-studio-runtime.json')
}

const runtime = runtimePath()
process.env.STUDIO_RUNTIME_FILE = runtime

const rendererLog = process.env.STUDIO_RENDERER_LOG || path.join(os.tmpdir(), 'a3s-studio-renderer.log')

function fail(message) {
  console.error(message)
  process.exit(1)
}

function probe(url, timeoutMs) {
  return new Promise((resolve) => {
    const request = http.get(url, (response) => {
      response.resume()
      resolve(response.statusCode !== undefined && response.statusCode < 500)
    })
    request.setTimeout(timeoutMs, () => {
      request.destroy()
      resolve(false)
    })
    request.on('error', () => resolve(false))
  })
}

function commandOutput(file, args) {
  try {
    return execFileSync(file, args, { encoding: 'utf8', windowsHide: true })
  } catch (error) {
    return typeof error?.stdout === 'string' ? error.stdout : ''
  }
}

function processArgs(pid) {
  if (process.platform === 'win32') {
    return commandOutput('powershell.exe', [
      '-NoProfile',
      '-Command',
      `(Get-CimInstance Win32_Process -Filter "ProcessId=${Number(pid)}").CommandLine`
    ]).trim()
  }
  return commandOutput('ps', ['-p', String(pid), '-o', 'args=']).trim()
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}

function portBusyMessage() {
  const label = (process.env.A3S_STUDIO_INSTANCE || '') === 'fork' ? 'just fork' : 'just studio'
  return `${label}: 127.0.0.1:${port} is in use`
}

function recordedPid() {
  try {
    const parsed = JSON.parse(readFileSync(runtime, 'utf8'))
    const pid = Number(parsed?.pid)
    return Number.isInteger(pid) && pid > 1 ? pid : 0
  } catch {
    return 0
  }
}

function portBusy() {
  return new Promise((resolve) => {
    const server = net.createServer()
    server.once('error', () => resolve(true))
    server.once('listening', () => {
      server.close(() => resolve(false))
    })
    server.listen({ port, host: '127.0.0.1', exclusive: true })
  })
}

function killRecorded(pid) {
  if (process.platform === 'win32') commandOutput('taskkill', ['/PID', String(pid), '/T', '/F'])
  else {
    try {
      process.kill(pid, 'SIGTERM')
    } catch {
      // The process can exit between the command-line check and the signal.
    }
  }
}

async function ensurePortFree() {
  if (!(await portBusy())) return
  const pid = recordedPid()
  const command = pid ? processArgs(pid) : ''
  if (!pid || !command.includes('Studio')) fail(portBusyMessage())
  killRecorded(pid)
  const started = Date.now()
  while (Date.now() - started < 10_000) {
    if (!(await portBusy())) return
    sleep(250)
  }
  fail(portBusyMessage())
}

function tailLog() {
  try {
    const lines = readFileSync(rendererLog, 'utf8').split(/\r?\n/)
    console.error(lines.slice(-40).join('\n'))
  } catch {
    // The server can exit before it writes a log.
  }
}

function openLog() {
  // Node 26 rejects a WriteStream whose fd is still null. Open the file first.
  return openSync(rendererLog, 'w')
}

function spawnRenderer(args) {
  const output = openLog()
  return spawn(process.execPath, [rsbuild, ...args], {
    cwd: root,
    stdio: ['ignore', output, output],
    windowsHide: true
  })
}

function alive(child) {
  return child.exitCode === null && child.signalCode === null
}

async function waitForRenderer(child, dev) {
  const budget = dev ? 30_000 : 1_000
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (await probe(rendererUrl, budget)) return
    if (!alive(child)) {
      console.error('just studio: the Cherry main window server exited')
      tailLog()
      process.exit(1)
    }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  console.error('just studio: timed out waiting for the Cherry main window')
  tailLog()
  process.exit(1)
}

function reportDevErrors() {
  try {
    const text = readFileSync(rendererLog, 'utf8')
    if (!/^error /m.test(text)) return
    console.error('just studio: the client dev build reported errors; the window shows the same overlay')
    const lines = text.split(/\r?\n/)
    const start = lines.findIndex((line) => line.startsWith('error '))
    console.error(lines.slice(start, start + 40).join('\n'))
  } catch {
    // A quiet log is a clean compile.
  }
}

const instance = process.env.A3S_STUDIO_INSTANCE || ''
const dev = process.env.STUDIO_RENDERER_DEV === '1'
let renderer = null

if (instance !== 'fork') {
  const cli = path.join('node_modules', 'tsx', 'dist', 'cli.mjs')
  const checker = existsSync(cli) ? cli : path.join('node_modules', '.bin', 'tsx')
  try {
    execFileSync(process.execPath, [checker, 'scripts/check-dependency-lockstep.ts'], {
      cwd: root,
      stdio: 'ignore',
      windowsHide: true
    })
  } catch {
    fail('just studio: dependencies disagree. Align package.json, then run pnpm install.')
  }
}

if (instance === 'fork') {
  if (!(await probe(rendererUrl, 1_000))) {
    await ensurePortFree()
    if (!existsSync(path.join('.studio-renderer', 'windows', 'main', 'index.html'))) {
      fail('just fork: no compiled client. Run just studio first.')
    }
    renderer = spawnRenderer(['preview', '--host', '127.0.0.1', '--port', String(port)])
  }
} else if (dev) {
  if ((await probe(rendererUrl, 1_000)) && portOwnedBy(port, root)) {
    console.error(`just studio: reusing the client dev server on 127.0.0.1:${port}`)
  } else if (await probe(rendererUrl, 1_000)) {
    const cwd = listenerCwd(port)
    fail(
      `just studio: 127.0.0.1:${port} belongs to another app${cwd ? ` (${cwd})` : ''}. ToWork uses this port.`
    )
  } else {
    await ensurePortFree()
    console.error(`just studio: client dev server on 127.0.0.1:${port} (hot reload)`)
    renderer = spawnRenderer(['dev', '--host', '127.0.0.1', '--port', String(port)])
  }
} else {
  console.error('just studio: rebuilding the client')
  try {
    const output = openLog()
    execFileSync(process.execPath, [rsbuild, 'build'], {
      cwd: root,
      stdio: ['ignore', output, output],
      windowsHide: true
    })
  } catch {
    console.error('just studio: the Cherry main window build failed')
    tailLog()
    process.exit(1)
  }
  await ensurePortFree()
  renderer = spawnRenderer(['preview', '--host', '127.0.0.1', '--port', String(port)])
}

const debugName = process.platform === 'win32' ? 'Studio.exe' : 'Studio'
const metadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'], {
    cwd: root,
    encoding: 'utf8'
  })
)
const targetDir = metadata.target_directory
const debugBinary = path.join(targetDir, 'debug', debugName)

if (instance === 'fork') {
  if (!existsSync(debugBinary)) fail('just fork: no compiled Studio app. Run just studio first.')
} else {
  execFileSync('cargo', ['build', '--locked', '-p', 'a3s-code-acp', '-p', 'a3s-office-cli'], {
    cwd: root,
    stdio: 'inherit',
    windowsHide: true
  })
  execFileSync('cargo', ['build', '--locked', '--manifest-path', 'src-tauri/Cargo.toml'], {
    cwd: root,
    stdio: 'inherit',
    windowsHide: true
  })
}

if (renderer) {
  await waitForRenderer(renderer, dev)
  if (dev) reportDevErrors()
}

if (!process.env.A3S_ACP_AGENT_BIN) {
  const name = process.platform === 'win32' ? 'a3s-code-acp.exe' : 'a3s-code-acp'
  const workspaceAgent = path.join(targetDir, 'debug', name)
  if (existsSync(workspaceAgent)) process.env.A3S_ACP_AGENT_BIN = workspaceAgent
}

rmSync(runtime, { force: true })
const studio = spawn(debugBinary, [], { cwd: root, stdio: 'inherit', env: process.env })
studio.on('exit', (code, signal) => {
  if (renderer && alive(renderer)) renderer.kill()
  if (signal) process.kill(process.pid, signal)
  process.exit(code ?? 1)
})
