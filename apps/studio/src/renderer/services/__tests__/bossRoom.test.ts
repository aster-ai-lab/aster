import { describe, expect, it } from 'vitest'

import { encodeBossWire } from '@renderer/im/quote'
import {
  acceptBossCatchUp,
  BOSS_CATCHUP_MS,
  bossAskSettled,
  bossMembership,
  bossRoomNeedsWarm,
  bossRoomPrompt,
  bossRoomSessionId,
  bossTranscript,
  isBossRoomPrompt,
  isBossRoomTurnAnchor,
  omitBossRoomTurns,
  completeBossJob,
  emptyBossLane,
  offerBossJob,
  openBossAsks,
  readBossGroups,
  teamBotMentionLabels,
  resetBossRuntimeForTests,
  type BossJob,
  type BossRoomMessage
} from '@renderer/services/bossRoom'

function line(id: string, text: string, createdAt: number, senderId = 'a'): BossRoomMessage {
  return { id, createdAt, recalled: false, senderId, senderName: senderId === 'a' ? '周予安' : '陈可', text }
}

function job(senderId: string, ask: string, conversationId = 'room'): BossJob {
  return { key: `${conversationId}:${senderId}:${ask}`, conversationId, senderId, ask, prompt: ask }
}

describe('boss room prompt', () => {
  it('keeps the room, the ask, and drops the oldest lines', () => {
    const lines = Array.from({ length: 30 }, (_, index) => ({ speaker: `s${index}`, text: `line ${index}` }))
    const prompt = bossRoomPrompt({ room: '项目组', asker: '周予安', ask: '你说', lines })
    expect(prompt).toContain('Group: 项目组')
    expect(prompt).toContain('Do not continue an earlier private reply')
    expect(prompt).toContain('s29: line 29')
    expect(prompt).toContain('周予安 asked: 你说')
    expect(prompt).not.toContain('s0: line 0')
    expect(prompt).not.toContain('s5: line 5')
    const ask = '问'.repeat(600)
    expect(bossRoomPrompt({ room: 'r', asker: 'a', ask, lines: [] })).toContain(`a asked: ${'问'.repeat(500)}`)
    expect(bossRoomPrompt({ room: 'r', asker: 'a', ask, lines: [] })).not.toContain('问'.repeat(501))
  })

  it('names ToBoss on a reply and skips join, leave, and recalled rows', () => {
    const messages = [
      line('1', 'hello', 1),
      line('2', encodeBossWire('', 'join'), 2),
      line('3', encodeBossWire('先前的回答', 'reply'), 3),
      { ...line('4', 'secret', 4), recalled: true },
      line('5', encodeBossWire('', 'leave'), 5)
    ]
    expect(bossTranscript(messages, 'ToBoss')).toEqual([
      { speaker: '周予安', text: 'hello' },
      { speaker: 'ToBoss', text: '先前的回答' }
    ])
    expect(bossMembership(messages)).toBe('out')
    expect(bossMembership([line('1', encodeBossWire('', 'leave'), 1), line('2', encodeBossWire('', 'join'), 2)])).toBe(
      'in'
    )
    expect(bossMembership([line('1', 'hello', 1)])).toBeNull()
  })
})

describe('boss room session', () => {
  it('names one session per group and leaves the private id unchanged', () => {
    const room = '8016bcd8-7f31-4abc-bd53-1f50ba7d2d05'
    const member = '225e2510-5521-4aa7-b96c-e0b0c0da6037'
    expect(bossRoomSessionId('toboss', room)).toBe(`toboss-room-${room}`)
    expect(bossRoomSessionId(`toboss-${member}`, room)).toBe(`toboss-${member}-room-${room}`)
    expect(bossRoomSessionId(`toboss-${member}`, room)).not.toBe(`toboss-${member}`)
    expect(bossRoomSessionId(`toboss-${member}`, 'not-a-group')).toBeNull()
  })
})

describe('boss private transcript', () => {
  it('drops a group turn and keeps the private lines around it', () => {
    const prompt = bossRoomPrompt({
      room: '项目组',
      asker: '周予安',
      ask: '你说说',
      lines: [{ speaker: '周予安', text: '群里的一句' }]
    })
    expect(isBossRoomPrompt(prompt)).toBe(true)
    expect(isBossRoomPrompt('你好')).toBe(false)
    expect(isBossRoomPrompt('@ToBoss 你说说')).toBe(false)
    const lines = [
      { id: 'u1', role: 'user', text: '私聊问题' },
      { id: 'a1', role: 'assistant', text: '私聊回答' },
      { id: 'u2', role: 'user', text: prompt },
      { id: 'a2', role: 'assistant', text: '群里的表格' },
      { id: 'u3', role: 'user', text: '再问一句' }
    ]
    expect(omitBossRoomTurns(lines).map((line) => line.text)).toEqual(['私聊问题', '私聊回答', '再问一句'])
    expect(isBossRoomTurnAnchor(lines, 'a2')).toBe(true)
    expect(isBossRoomTurnAnchor(lines, 'a1')).toBe(false)
    expect(isBossRoomTurnAnchor(lines.slice(0, 3), 'live-assistant')).toBe(true)
  })
})

describe('boss lane', () => {
  it('starts, queues, coalesces, and refuses a burst', () => {
    let lane = emptyBossLane()
    const started = offerBossJob(lane, job('a', 'one'), 0)
    expect(started.decision).toBe('start')
    lane = started.lane
    const queued = offerBossJob(lane, job('b', 'two'), 1_000)
    expect(queued.decision).toBe('queued')
    lane = queued.lane
    const replaced = offerBossJob(lane, job('b', 'three'), 1_500)
    expect(replaced.decision).toBe('replaced')
    expect(replaced.lane.queue.map((item) => item.ask)).toEqual(['three'])
    expect(replaced.lane.acceptedAt['room:b']).toBe(1_000)
    lane = replaced.lane
    expect(offerBossJob(lane, job('a', 'one'), 2_000).decision).toBe('duplicate')
    expect(offerBossJob(lane, job('a', 'later'), 2_000)).toMatchObject({ decision: 'soon', lane })
    lane = offerBossJob(lane, job('c', 'four'), 20_000).lane
    lane = offerBossJob(lane, job('d', 'five'), 20_000).lane
    expect(offerBossJob(lane, job('e', 'six'), 20_000).decision).toBe('full')
    const step = completeBossJob(lane, 50_000)
    expect(step.job?.ask).toBe('three')
    expect(step.lane.running?.ask).toBe('three')
    expect(bossAskSettled(step.lane, 'room', 'a', 'one', 50_000)).toBe(true)
    expect(bossAskSettled(step.lane, 'room', 'a', 'one', 50_001)).toBe(false)
    const missed = completeBossJob(lane, 50_000, false)
    expect(bossAskSettled(missed.lane, 'room', 'a', 'one', 50_000)).toBe(false)
    expect(missed.job?.ask).toBe('three')
  })
})

describe('open boss asks', () => {
  it('stops at the latest answer and ignores asks older than the catch-up window', () => {
    const now = 1_000_000_000
    const messages = [
      line('old', '@ToBoss old', now - BOSS_CATCHUP_MS - 1),
      line('hello', 'group hello', now - 4_000),
      line('one', '@ToBoss one', now - 3_000, 'b'),
      line('join', encodeBossWire('', 'join'), now - 2_000),
      line('reply', encodeBossWire('done', 'reply'), now - 1_000),
      line('two', '@ToBoss two', now - 500),
      { ...line('gone', '@ToBoss recalled', now - 400), recalled: true },
      line('three', '@ToBoss three', now)
    ]
    expect(openBossAsks(messages, ['ToBoss'], now).map((item) => item.id)).toEqual(['two', 'three'])
    expect(openBossAsks([line('stale', '@ToBoss stale', now - BOSS_CATCHUP_MS - 1)], ['ToBoss'], now)).toEqual([])
    expect(
      openBossAsks([line('fresh', '@ToBoss fresh', now - BOSS_CATCHUP_MS + 1_000)], ['ToBoss'], now).map(
        (item) => item.id
      )
    ).toEqual(['fresh'])
    const burst = Array.from({ length: 4 }, (_, index) =>
      line(`m${index}`, `@ToBoss ${index}`, now - (4 - index) * 100, index % 2 === 0 ? 'a' : 'b')
    )
    expect(openBossAsks(burst, ['ToBoss'], now).map((item) => item.id)).toEqual(['m1', 'm2', 'm3'])
  })

  it('hears both ToBot and an older @ToBoss', () => {
    const now = 1_000_000_000
    const labels = teamBotMentionLabels('ToBot', 'ToBot')
    expect(openBossAsks([line('old-name', '@ToBoss one', now)], labels, now).map((item) => item.id)).toEqual([
      'old-name'
    ])
    expect(openBossAsks([line('new-name', '@ToBot two', now)], labels, now).map((item) => item.id)).toEqual([
      'new-name'
    ])
  })
})

describe('boss group memory', () => {
  it('reads a list saved under another session id', () => {
    const org = '11111111-1111-4111-8111-111111111111'
    globalThis.localStorage.clear()
    globalThis.localStorage.setItem(`messages.toboss.groups.toboss-member.${org}`, JSON.stringify(['room']))
    expect(readBossGroups(org, 'toboss')).toEqual(['room'])
    globalThis.localStorage.clear()
  })

  it('warms a named @ and a saved group, and leaves @所有人 cold', () => {
    expect(bossRoomNeedsWarm({ kind: 'group', previewText: '@ToBoss 你说说' }, false, [], ['ToBoss'], 'g')).toBe(true)
    expect(bossRoomNeedsWarm({ kind: 'group', previewText: '@所有人 你说说' }, false, [], ['ToBoss'], 'g')).toBe(false)
    expect(bossRoomNeedsWarm({ kind: 'group', previewText: 'hello' }, false, ['g'], ['ToBoss'], 'g')).toBe(true)
    expect(bossRoomNeedsWarm({ kind: 'group', previewText: '@ToBoss 你说说' }, true, ['g'], ['ToBoss'], 'g')).toBe(
      false
    )
  })

  it('starts one catch-up job for a fresh named mention', () => {
    resetBossRuntimeForTests()
    const now = Date.now()
    const input = {
      now,
      labels: ['ToBoss'],
      bossName: 'ToBoss',
      bossListed: true,
      memberGroupIds: ['room'],
      rooms: [
        {
          id: 'room',
          title: '项目组',
          messages: [line('m1', 'group hello', now - 1000), line('m2', '@ToBoss 你说说', now)]
        }
      ]
    }
    const first = acceptBossCatchUp(input)
    expect(first.started).toHaveLength(1)
    expect(first.started[0]?.prompt).toContain('group hello')
    expect(first.started[0]?.prompt).toContain('你说说')
    expect(first.started[0]?.prompt).toContain('Do not continue an earlier private reply')
    expect(acceptBossCatchUp(input).started).toEqual([])
    resetBossRuntimeForTests()
  })
})
