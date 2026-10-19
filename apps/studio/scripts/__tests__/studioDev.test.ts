import { execFile } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'

import { createServer } from 'node:http'

import { describe, expect, it } from 'vitest'

import { directoryOwns, portOwnedBy } from '../renderer-owner.mjs'

const execute = promisify(execFile)
const launcher = path.resolve(__dirname, '../studio-dev.mjs')

describe.skipIf(process.platform === 'win32')('fork build selection (POSIX executable fixture)', () => {
  it.each(['default', 'absolute', 'relative', 'missing'] as const)(
    'uses the %s Cargo target without falling back to the shared build',
    async (mode) => {
      const root = mkdtempSync(path.join(tmpdir(), 'studio-fork-target-'))
      const server = createServer((_, response) => response.end('fixture renderer'))
      try {
        await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
        const address = server.address()
        if (!address || typeof address === 'string') throw new Error('Fixture renderer has no port')
        mkdirSync(path.join(root, 'scripts'))
        const cargo = path.join(root, 'scripts/cargo')
        writeFileSync(
          cargo,
          `#!/bin/sh\nnode -e 'process.stdout.write(JSON.stringify({target_directory: require("path").resolve(process.env.CARGO_TARGET_DIR || "target")}))'\n`,
          { mode: 0o700 }
        )
        copyFileSync(launcher, path.join(root, 'scripts/studio-dev.mjs'))
        copyFileSync(path.resolve(__dirname, '../renderer-owner.mjs'), path.join(root, 'scripts/renderer-owner.mjs'))
        const receipt = path.join(root, 'launched.txt')
        const install = (target: string, name: string) => {
          const binary = path.join(root, target, 'debug/Studio')
          mkdirSync(path.dirname(binary), { recursive: true })
          writeFileSync(binary, `#!/bin/sh\nprintf '%s' '${name}' > "$STUDIO_FORK_TEST_RECEIPT"\n`, { mode: 0o700 })
        }
        install('target', 'shared')
        writeFileSync(path.join(root, 'Cargo.toml'), '[workspace]\n')
        if (mode === 'absolute' || mode === 'relative') install('isolated target', 'isolated')
        const env: NodeJS.ProcessEnv = {
          ...process.env,
          PATH: path.join(root, 'scripts') + path.delimiter + process.env.PATH,
          A3S_STUDIO_INSTANCE: 'fork',
          STUDIO_RENDERER_PORT: String(address.port),
          STUDIO_RUNTIME_FILE: path.join(root, 'runtime.json'),
          STUDIO_FORK_TEST_RECEIPT: receipt
        }
        delete env.CARGO_TARGET_DIR
        if (mode !== 'default') {
          env.CARGO_TARGET_DIR = mode === 'relative' ? 'isolated target' : path.join(root, 'isolated target')
        }
        const run = execute(process.execPath, [path.join(root, 'scripts/studio-dev.mjs')], {
          cwd: root,
          env,
          timeout: 10_000
        })
        if (mode === 'missing') {
          await expect(run).rejects.toMatchObject({ code: 1, stderr: expect.stringContaining('no compiled Studio') })
          expect(existsSync(receipt)).toBe(false)
        } else {
          await run
          expect(readFileSync(receipt, 'utf8')).toBe(mode === 'default' ? 'shared' : 'isolated')
        }
      } finally {
        await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())))
        rmSync(root, { recursive: true, force: true })
      }
    }
  )
})

describe('renderer port owner', () => {
  it('accepts only the app directory that is listening', async () => {
    const server = createServer((_, response) => response.end('ok'))
    const owned = mkdtempSync(path.join(tmpdir(), 'studio-owner-'))
    const other = mkdtempSync(path.join(tmpdir(), 'studio-other-'))
    try {
      await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
      const address = server.address()
      if (!address || typeof address === 'string') throw new Error('no port')
      expect(directoryOwns(owned, owned)).toBe(true)
      expect(directoryOwns(path.join(owned, 'src'), owned)).toBe(true)
      expect(directoryOwns(other, owned)).toBe(false)
      expect(portOwnedBy(address.port, owned)).toBe(false)
      expect(portOwnedBy(address.port, process.cwd())).toBe(true)
    } finally {
      await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())))
      rmSync(owned, { recursive: true, force: true })
      rmSync(other, { recursive: true, force: true })
    }
  })
})
