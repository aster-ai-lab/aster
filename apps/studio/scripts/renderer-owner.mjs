import { execFileSync } from 'node:child_process'
import path from 'node:path'

export function directoryOwns(cwd, appRoot) {
  if (!cwd || !appRoot) return false
  const expected = path.resolve(appRoot)
  const actual = path.resolve(cwd)
  return actual === expected || actual.startsWith(expected + path.sep)
}

function commandOutput(file, args) {
  try {
    return execFileSync(file, args, { encoding: 'utf8', windowsHide: true })
  } catch (error) {
    return typeof error?.stdout === 'string' ? error.stdout : ''
  }
}

export function listenerCwd(port) {
  if (process.platform === 'win32') return ''
  const pid = commandOutput('lsof', ['-nP', `-iTCP:${port}`, '-sTCP:LISTEN', '-t'])
    .split(/\s+/)
    .find(Boolean)
  if (!pid) return ''
  const text = commandOutput('lsof', ['-a', '-p', pid, '-d', 'cwd', '-Fn'])
  const line = text.split(/\n/).find((item) => item.startsWith('n'))
  return line ? line.slice(1) : ''
}

export function portOwnedBy(port, appRoot) {
  return directoryOwns(listenerCwd(port), appRoot)
}
