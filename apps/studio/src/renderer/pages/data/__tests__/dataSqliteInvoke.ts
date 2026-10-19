import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { runDataSqliteCommand } from '../sqliteQuery'

const COMMAND = 'data_sqlite_query'
const PERMISSION = 'allow-data-sqlite-query'

const dataSqliteInvocations: string[] = []
const dataSqliteCalls: Array<{ command: string; engine: unknown; sql: unknown }> = []

export function readDataSqliteInvocations(): readonly string[] {
  return dataSqliteInvocations
}

export function readDataSqliteCalls(): readonly { command: string; engine: unknown; sql: unknown }[] {
  return dataSqliteCalls
}

type Internals = {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), '../../../../../src-tauri')
}

/** The shipped permission and both main-window capabilities grant the command. */
export function dataSqliteCommandPermitted(): boolean {
  const root = tauriRoot()
  const granted = readdirSync(join(root, 'permissions')).some((name) => {
    if (!name.endsWith('.toml')) return false
    const text = readFileSync(join(root, 'permissions', name), 'utf8')
    return text.includes(`identifier = "${PERMISSION}"`) && text.includes(`"${COMMAND}"`)
  })
  if (!granted) return false
  for (const capability of ['capabilities/default.json', 'capabilities/renderer.json']) {
    const file = JSON.parse(readFileSync(join(root, capability), 'utf8')) as { permissions?: string[] }
    if (!file.permissions?.includes(PERMISSION)) return false
  }
  return true
}

/** Stand in for the Tauri shell: the same command name, allowed only by the shipped ACL. */
export function installDataSqliteInvoke(): () => void {
  dataSqliteInvocations.length = 0
  dataSqliteCalls.length = 0
  const holder = globalThis as typeof globalThis & { __TAURI_INTERNALS__?: Internals }
  const previous = holder.__TAURI_INTERNALS__
  holder.__TAURI_INTERNALS__ = {
    invoke(command, args) {
      dataSqliteInvocations.push(command)
      dataSqliteCalls.push({ command, engine: args?.engine ?? 'sqlite', sql: args?.sql })
      if (command !== COMMAND || !dataSqliteCommandPermitted()) {
        return Promise.reject(new Error('command data_sqlite_query is not allowed'))
      }
      return runDataSqliteCommand(args?.path, args?.sql, args?.engine)
    }
  }
  return () => {
    if (previous === undefined) delete holder.__TAURI_INTERNALS__
    else holder.__TAURI_INTERNALS__ = previous
  }
}
