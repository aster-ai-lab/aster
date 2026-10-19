import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

describe('InfoDrawer motion', () => {
  it('fades in place and does not wait for a transform transition', () => {
    const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../MessagesPage.tsx'), 'utf8')
    const start = source.indexOf('function InfoDrawer')
    const end = source.indexOf('function tobotGroupStorageKey')
    const body = source.slice(start, end)

    expect(start).toBeGreaterThan(-1)
    expect(body).not.toContain('translate-x-full')
    expect(body).not.toContain("propertyName !== 'transform'")
    expect(body).toContain('duration-200')
  })
})
