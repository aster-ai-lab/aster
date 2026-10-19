import { describe, expect, it } from 'vitest'

import { jsonObject } from '@renderer/components/composer/agentParameterPrefill'

describe('jsonObject', () => {
  it('reads a JSON object and a fenced object', () => {
    expect(jsonObject('{"days":5}')).toEqual({ days: 5 })
    expect(jsonObject('```json\n{"kind":"a"}\n```')).toEqual({ kind: 'a' })
    expect(jsonObject('not json')).toBeNull()
  })
})
