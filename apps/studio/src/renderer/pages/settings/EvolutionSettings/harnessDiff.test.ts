import { describe, expect, it } from 'vitest'

import { diffComponents } from './harnessDiff'
import { HARNESS_SNAPSHOTS } from './harnessSnapshots'

const byId = (id: string) => HARNESS_SNAPSHOTS.find((item) => item.id === id)?.components ?? []

describe('diffComponents', () => {
  it('marks compact as removed from the lean assemble', () => {
    const rows = diffComponents(byId('default'), byId('lean'))
    expect(rows.find((row) => row.text === 'compact')?.kind).toBe('removed')
    expect(rows.filter((row) => row.kind === 'same').map((row) => row.text)).toEqual([
      'system',
      'tools',
      'budget',
      'infer'
    ])
  })

  it('marks the host mount as an insert', () => {
    const rows = diffComponents(byId('default'), byId('host'))
    const insert = rows.find((row) => row.text === 'host:intent_stamp')
    expect(insert?.kind).toBe('added')
    expect(rows.map((row) => row.text)).toEqual(['system', 'tools', 'host:intent_stamp', 'budget', 'compact', 'infer'])
  })
})
