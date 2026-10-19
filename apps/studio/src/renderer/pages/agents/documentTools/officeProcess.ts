import type { ChildProcess, SpawnOptions } from 'node:child_process'

type Spawn = (command: string, args: readonly string[], options: SpawnOptions) => ChildProcess
type OfficeSignal = InstanceType<typeof globalThis.AbortController>['signal']
const { performance, setTimeout, clearTimeout, TextDecoder } = globalThis

export const OFFICE_OPERATION_MS = 60_000
export const OFFICE_OUTPUT_LIMIT = 25 * 1024 * 1024

/** Resolve cancellation only after this owned process closes. */
export function runOwnedOfficeProcess(
  spawn: Spawn,
  bin: string,
  args: string[],
  deadline: number,
  signal?: OfficeSignal,
  limit = OFFICE_OUTPUT_LIMIT
): Promise<string> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(new Error('abort'))
    if (performance.now() >= deadline) return reject(new Error('office-timeout'))
    const child = spawn(bin, args, { stdio: ['ignore', 'pipe', 'pipe'] })
    let failure: string | undefined
    let killTimer: ReturnType<typeof setTimeout> | undefined
    const chunks: Uint8Array[] = []
    let size = 0
    const stop = (reason: string) => {
      if (failure) return
      failure = reason
      child.kill('SIGTERM')
      killTimer = setTimeout(() => child.kill('SIGKILL'), 2000)
    }
    const abort = () => stop('abort')
    const timer = setTimeout(() => stop('office-timeout'), Math.max(1, deadline - performance.now()))
    signal?.addEventListener('abort', abort, { once: true })
    child.stdout?.on('data', (chunk: Uint8Array) => {
      if (failure) return
      size += chunk.byteLength
      if (size > limit) stop('office-output-limit')
      else chunks.push(chunk)
    })
    child.stdout?.on('error', () => stop('office'))
    child.stderr?.on('error', () => stop('office'))
    child.stderr?.resume()
    child.on('error', () => {
      failure ??= 'office'
    })
    child.on('close', (code) => {
      clearTimeout(timer)
      clearTimeout(killTimer)
      signal?.removeEventListener('abort', abort)
      if (failure) return reject(new Error(failure))
      if (code !== 0) return reject(new Error('unreadable'))
      const bytes = new Uint8Array(size)
      let offset = 0
      for (const chunk of chunks) {
        bytes.set(chunk, offset)
        offset += chunk.byteLength
      }
      try {
        resolve(new TextDecoder('utf-8', { fatal: true }).decode(bytes))
      } catch {
        reject(new Error('unreadable'))
      }
    })
    // An abort can arrive between admission and installing the listener.
    if (signal?.aborted) abort()
  })
}
