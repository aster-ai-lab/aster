import { existsSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import type { CherryMessagePart } from '@shared/data/types/message'

import { projectTaskConversationTurn } from '../turn'

const partsFile = join(tmpdir(), 'a3s-studio-task-m10-parts.json')

function commandBodies(parts: readonly CherryMessagePart[]): string[] {
  const bodies: string[] = []
  for (const part of parts) {
    if (!part || typeof part !== 'object' || !('input' in part)) continue
    const input = part.input
    if (!input || typeof input !== 'object' || Array.isArray(input)) continue
    const record = input as Record<string, unknown>
    for (const key of ['command', 'cmd'] as const) {
      const value = record[key]
      if (typeof value === 'string' && value.trim()) bodies.push(value)
    }
  }
  return bodies
}

function mentionsOut(part: CherryMessagePart | undefined): boolean {
  return Boolean(part && JSON.stringify(part).includes('out.txt'))
}

describe('local acp parts', () => {
  const required = process.env.A3S_STUDIO_TASK_PARTS === '1'
  const present = existsSync(partsFile)
  const test = !present && !required ? it.skip : it

  test('projects the M10 file and does not draw the token as a command', () => {
    if (!present) throw new Error('M10 left no message parts')
    const saved = JSON.parse(readFileSync(partsFile, 'utf8')) as { parts?: CherryMessagePart[]; token?: string }
    const parts = saved.parts ?? []
    const token = saved.token ?? ''
    expect(token.length).toBeGreaterThan(0)
    const items = projectTaskConversationTurn(parts, false)
    const pointed = items.some((item) => {
      if (item.kind === 'tool' && item.partIndex !== undefined) return mentionsOut(parts[item.partIndex])
      if (item.kind === 'block') return mentionsOut(parts[item.index])
      return false
    })
    expect(pointed).toBe(true)
    expect(commandBodies(parts).filter((body) => body.includes(token))).toEqual([])
  })
})
