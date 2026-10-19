import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { initSentry } from '../sentry'

const sentrySource = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../sentry.ts'), 'utf8')

describe('renderer Sentry initialization', () => {
  it('does not load an Electron error SDK', () => {
    expect(sentrySource).not.toMatch(/@sentry\/electron/)
    expect(() => initSentry()).not.toThrow()
  })
})
