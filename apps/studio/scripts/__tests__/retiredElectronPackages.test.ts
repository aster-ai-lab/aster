import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const ROOT_DIR = resolve(__dirname, '..', '..')
const WORKFLOWS = [
  '.github/workflows/nightly-build.yml',
  '.github/workflows/preview-release.yml',
  '.github/workflows/release.yml',
  '.github/workflows/sync-to-gitcode.yml',
  '.github/workflows/desktop-smoke.yml'
]

describe('retired Electron package scripts', () => {
  it.each(WORKFLOWS)('%s does not call the removed package scripts', (relativePath) => {
    const text = readFileSync(resolve(ROOT_DIR, relativePath), 'utf8')
    expect(text).not.toMatch(/build:(win|mac|linux)/)
  })
})
