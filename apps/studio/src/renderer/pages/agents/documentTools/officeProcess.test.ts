import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import process from 'node:process'

import { describe, expect, it, vi } from 'vitest'

import { runOwnedOfficeProcess } from './officeProcess'

const { performance, setTimeout, AbortController } = globalThis

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

async function pidAt(path: string): Promise<number> {
  const deadline = performance.now() + 5000
  while (performance.now() < deadline) {
    try {
      return Number(await readFile(path, 'utf8'))
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 20))
    }
  }
  throw new Error('owned child did not start')
}

describe('owned Office process', () => {
  it('reaps a stalled child before reporting timeout', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'a3s-office-timeout-'))
    const file = join(dir, 'pid')
    try {
      const result = runOwnedOfficeProcess(
        spawn,
        process.execPath,
        ['-e', `require('fs').writeFileSync(process.argv[1],String(process.pid));setInterval(()=>{},1000)`, file],
        performance.now() + 1000
      )
      await expect(result).rejects.toThrow('office-timeout')
      expect(alive(Number(await readFile(file, 'utf8')))).toBe(false)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('waits for close and escalates cancellation when the child ignores SIGTERM', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'a3s-office-cancel-'))
    const file = join(dir, 'pid')
    const controller = new AbortController()
    let settled = false
    try {
      const result = runOwnedOfficeProcess(
        spawn,
        process.execPath,
        [
          '-e',
          `process.on('SIGTERM',()=>{});require('fs').writeFileSync(process.argv[1],String(process.pid));setInterval(()=>{},1000)`,
          file
        ],
        performance.now() + 10_000,
        controller.signal
      )
      const observed = result.catch((error: Error) => {
        settled = true
        return error.message
      })
      const pid = await pidAt(file)
      controller.abort()
      await Promise.resolve()
      expect(settled).toBe(false)
      expect(alive(pid)).toBe(true)
      expect(await observed).toBe('abort')
      expect(alive(pid)).toBe(false)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('kills a flooding child instead of waiting for EOF or retaining its output', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'a3s-office-flood-'))
    const file = join(dir, 'pid')
    try {
      const result = runOwnedOfficeProcess(
        spawn,
        process.execPath,
        [
          '-e',
          `require('fs').writeFileSync(process.argv[1],String(process.pid));const b=Buffer.alloc(4096);setInterval(()=>process.stdout.write(b),1)`,
          file
        ],
        performance.now() + 5000,
        undefined,
        1024
      )
      await expect(result).rejects.toThrow('office-output-limit')
      expect(alive(Number(await readFile(file, 'utf8')))).toBe(false)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it('consumes noisy stderr and preserves UTF-8 output', async () => {
    const result = await runOwnedOfficeProcess(
      spawn,
      process.execPath,
      ['-e', `process.stderr.write(Buffer.alloc(100000));process.stdout.write('转换完成')`],
      performance.now() + 5000
    )
    expect(result).toBe('转换完成')
    await expect(
      runOwnedOfficeProcess(
        spawn,
        process.execPath,
        ['-e', `process.stdout.write(Buffer.from([255]))`],
        performance.now() + 5000
      )
    ).rejects.toThrow('unreadable')
  })

  it('refuses pre-abort and an expired operation before launching another child', async () => {
    const launch = vi.fn(spawn)
    const controller = new AbortController()
    controller.abort()
    await expect(
      runOwnedOfficeProcess(launch, process.execPath, [], performance.now() + 5000, controller.signal)
    ).rejects.toThrow('abort')
    await expect(runOwnedOfficeProcess(launch, process.execPath, [], performance.now())).rejects.toThrow(
      'office-timeout'
    )
    expect(launch).not.toHaveBeenCalled()
  })
})
