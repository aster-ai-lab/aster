import { describe, expect, it } from 'vitest'

import { diagnosticPublication } from '../workspaceDiagnostics'

const snapshot = {
  documentId: 'native-document',
  uri: 'file:///workspace/note.txt',
  version: 2,
  text: '🙂x\r\nz\n',
  dirty: true,
  diskConflict: false
}
function publication() {
  return {
    publisherId: '018f1470-6629-778a-84e2-879605885ff1',
    sequence: 1,
    version: 2,
    items: [
      {
        start: { line: 0, character: 0 },
        end: { line: 0, character: 2 },
        severity: 'error' as const,
        message: 'finding'
      }
    ]
  }
}

describe('version-bound diagnostic observations', () => {
  it('keeps accepted observations immutable and uses UTF-16 positions', () => {
    const source = publication()
    const accepted = diagnosticPublication(source, snapshot)
    source.items[0].message = 'replaced'
    source.items[0].end.character = 1
    expect(accepted.items[0].message).toBe('finding')
    expect(accepted.items[0].end.character).toBe(2)
    expect(Object.isFrozen(accepted.items[0].end)).toBe(true)
    expect(() => diagnosticPublication(source, snapshot)).toThrow('range')
  })

  it('refuses stale versions, forged fields and unsafe publication identities', () => {
    for (const change of [
      { version: 1 },
      { publisherId: 'invented' },
      { sequence: 0 },
      { sequence: Number.MAX_SAFE_INTEGER + 1 },
      { authority: 'forged' }
    ])
      expect(() => diagnosticPublication({ ...publication(), ...change }, snapshot)).toThrow('publication')
    expect(() => diagnosticPublication(null, snapshot)).toThrow('publication')
  })

  it('refuses reversed, out-of-document and malformed ranges without clamping', () => {
    const source = publication()
    for (const change of [
      { end: { line: 0, character: 1 } },
      { end: { line: 0, character: 4 } },
      { end: { line: 3, character: 0 } },
      { start: { line: 0, character: 3 } },
      { end: { line: 0, character: -1 } },
      { severity: 'fatal' },
      { source: 'forged' }
    ]) {
      expect(() => diagnosticPublication({ ...source, items: [{ ...source.items[0], ...change }] }, snapshot)).toThrow(
        'range'
      )
    }
  })

  it('refuses oversized observations and embedded nul messages', () => {
    const source = publication()
    expect(() => diagnosticPublication({ ...source, items: Array(129).fill(source.items[0]) }, snapshot)).toThrow(
      'publication'
    )
    for (const message of ['x'.repeat(2049), 'embedded\0nul', ''])
      expect(() => diagnosticPublication({ ...source, items: [{ ...source.items[0], message }] }, snapshot)).toThrow(
        'range'
      )
    expect(() => diagnosticPublication(source, { ...snapshot, text: 'x'.repeat(64 * 1024 + 1) })).toThrow('publication')
  })
})
