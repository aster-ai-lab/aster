import { afterEach, describe, expect, it } from 'vitest'

import { officialWritingTaskText } from '../officialWriting'
import {
  armOfficialWritingDocument,
  commitOfficialWritingDocument,
  noteOfficialWritingAgentProse,
  planOfficialWritingFollowUp,
  readOfficialWritingSession,
  resetOfficialWritingSessionForTests,
  setOfficialWritingView,
  revealOfficialWritingFrame
} from '../officialWritingAgentEdit'

afterEach(() => {
  resetOfficialWritingSessionForTests()
})

describe('officialWritingTurns', () => {
  it('keeps the first document and rejects a rewrite that drops it', () => {
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    expect(task).toBeTruthy()
    armOfficialWritingDocument(task ?? '')
    const previous = readOfficialWritingSession()?.document ?? ''
    expect(previous).toContain('市教育局')
    expect(previous).toContain('各学校：')
    expect(previous).toContain('周五开会')

    const added = '请于周一前报送。'
    const next = `${previous}${added}`
    const plan = planOfficialWritingFollowUp(previous, next)
    expect(plan.ok).toBe(true)
    if (!plan.ok) return
    expect(plan.kept).toBe(previous)
    expect(plan.typing.caret).toBe(previous.length)
    expect(plan.typing.incoming).toBe(added)
    expect(plan.typing.frames.length).toBeGreaterThan(1)
    expect(revealOfficialWritingFrame(previous, plan.typing, plan.typing.frames.at(-1) ?? '')).toBe(next)
    expect(revealOfficialWritingFrame(previous, plan.typing, plan.typing.frames[0] ?? '').startsWith(previous)).toBe(
      true
    )

    expect(noteOfficialWritingAgentProse('另一份完全不同的稿')).toBe('rejected')
    expect(readOfficialWritingSession()?.document).toBe(previous)
    expect(readOfficialWritingSession()?.update).toBeNull()

    expect(noteOfficialWritingAgentProse(next)).toBe('applied')
    expect(readOfficialWritingSession()?.document).toBe(previous)
    expect(readOfficialWritingSession()?.update).toBe(next)
  })

  it('types a placeholder fill and rejects a draft that drops the document', () => {
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    expect(task).toBeTruthy()
    armOfficialWritingDocument(task ?? '')
    const previous = readOfficialWritingSession()?.document ?? ''
    const marker = '（待填写）'
    expect(previous).toContain(marker)
    const first = '请各校于周五下午开会。'
    const second = '会后报送参会名单。'
    const at1 = previous.indexOf(marker)
    const once = previous.slice(0, at1) + first + previous.slice(at1 + marker.length)
    const at2 = once.indexOf(marker)
    const next = once.slice(0, at2) + second + once.slice(at2 + marker.length)

    const plan = planOfficialWritingFollowUp(previous, next)
    expect(plan.ok).toBe(true)
    if (!plan.ok) return
    expect(plan.kept).toBe(previous)
    expect(plan.steps.map((step) => step.incoming)).toEqual([first, second])
    expect(plan.steps[0]?.caret).toBe(at1)
    expect(plan.steps[0]?.replaced).toBe(marker.length)
    expect(plan.steps[0]?.frames.length).toBeGreaterThan(1)
    expect(plan.steps.every((step) => !step.incoming.includes('具体事项'))).toBe(true)

    let shown = previous
    const firstFrame = plan.steps[0]?.frames[0] ?? ''
    const firstShown = revealOfficialWritingFrame(previous, plan.steps[0]!, firstFrame)
    expect(firstShown.startsWith(previous.slice(0, at1))).toBe(true)
    expect(firstShown.endsWith(previous.slice(at1 + marker.length))).toBe(true)
    expect(firstShown).toContain(firstFrame)
    expect(firstShown.slice(at1, at1 + marker.length)).not.toBe(marker)
    for (const step of plan.steps) {
      shown = revealOfficialWritingFrame(shown, step, step.incoming)
    }
    expect(shown).toBe(next)
    expect(shown).toContain('市教育局')
    expect(shown).toContain('各学校：')

    expect(noteOfficialWritingAgentProse('另一份完全不同的稿')).toBe('rejected')
    expect(readOfficialWritingSession()?.document).toBe(previous)
    expect(noteOfficialWritingAgentProse(next)).toBe('applied')
    expect(readOfficialWritingSession()?.document).toBe(previous)
    expect(readOfficialWritingSession()?.update).toBe(next)
  })

  it('rejects a prefix that drops the closer and keeps a finished placeholder fill', () => {
    const previous = '各学校：\n（待填写）\n特此通知'
    expect(planOfficialWritingFollowUp(previous, '各学校：\n周').ok).toBe(false)

    const filled = '各学校：\n周五开会。\n特此通知'
    const plan = planOfficialWritingFollowUp(previous, filled)
    expect(plan.ok).toBe(true)
    if (!plan.ok) return
    expect(plan.typing.incoming).toBe('周五开会。')
    expect(plan.typing.replaced).toBe('（待填写）'.length)
    expect(revealOfficialWritingFrame(previous, plan.typing, plan.typing.incoming)).toBe(filled)
    expect(filled).toContain('特此通知')

    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    armOfficialWritingDocument(task ?? '')
    const armed = readOfficialWritingSession()?.document ?? ''
    const markerAt = armed.indexOf('（待填写）')
    const snapshot = armed.slice(0, markerAt + 1)
    expect(planOfficialWritingFollowUp(armed, snapshot).ok).toBe(false)
    expect(noteOfficialWritingAgentProse(snapshot)).toBe('rejected')
    expect(readOfficialWritingSession()?.document).toBe(armed)
    expect(readOfficialWritingSession()?.update).toBeNull()
  })

  it('rejects an empty hunk and still plans the next edit of the kept paper', () => {
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    expect(task).toBeTruthy()
    armOfficialWritingDocument(task ?? '')
    const previous = readOfficialWritingSession()?.document ?? ''
    const marker = '（待填写）'
    const at = previous.indexOf(marker)
    expect(at).toBeGreaterThan(0)
    const sentence = '周五下午开会。'
    const filled = previous.slice(0, at) + sentence + previous.slice(at + marker.length)
    const accepted = planOfficialWritingFollowUp(previous, filled)
    expect(accepted.ok).toBe(true)
    if (!accepted.ok) return
    const firstFrame = accepted.typing.frames[0] ?? ''
    const firstShown = revealOfficialWritingFrame(previous, accepted.typing, firstFrame)
    expect(firstShown.startsWith(previous.slice(0, at))).toBe(true)
    expect(firstShown.endsWith(previous.slice(at + marker.length))).toBe(true)
    expect(revealOfficialWritingFrame(previous, accepted.typing, accepted.typing.incoming)).toBe(filled)
    commitOfficialWritingDocument(filled)
    expect(readOfficialWritingSession()?.document).toBe(filled)

    const emptied = filled.replace(sentence, '')
    expect(emptied).not.toBe(filled)
    expect(planOfficialWritingFollowUp(filled, emptied).ok).toBe(false)
    expect(noteOfficialWritingAgentProse('')).toBe('idle')
    expect(noteOfficialWritingAgentProse('另一份完全不同的稿')).toBe('rejected')
    const cut = filled.slice(0, filled.indexOf('特此通知'))
    expect(planOfficialWritingFollowUp(filled, cut).ok).toBe(false)
    expect(noteOfficialWritingAgentProse(cut)).toBe('rejected')
    expect(readOfficialWritingSession()?.document).toBe(filled)
    expect(readOfficialWritingSession()?.update).toBeNull()

    const nextAt = filled.indexOf(marker)
    expect(nextAt).toBeGreaterThan(at)
    const laterSentence = '地点在一号会议室。'
    const later = filled.slice(0, nextAt) + laterSentence + filled.slice(nextAt + marker.length)
    const follow = planOfficialWritingFollowUp(filled, later)
    expect(follow.ok).toBe(true)
    if (!follow.ok) return
    const rebuilt = revealOfficialWritingFrame(filled, follow.typing, follow.typing.incoming)
    expect(rebuilt).toBe(later)
    expect(rebuilt).toContain(sentence)
    expect(rebuilt).toContain(laterSentence)
    expect(rebuilt).toContain('特此通知')
    expect(rebuilt.startsWith(filled.slice(0, nextAt))).toBe(true)
    expect(noteOfficialWritingAgentProse(later)).toBe('applied')
    expect(readOfficialWritingSession()?.document).toBe(filled)
    expect(readOfficialWritingSession()?.update).toContain(sentence)
    expect(readOfficialWritingSession()?.update).toContain(laterSentence)
  })

  it('keeps another task from typing onto an armed official document', () => {
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    armOfficialWritingDocument(task ?? '', 'session-writing')
    const armed = readOfficialWritingSession()?.document ?? ''
    const next = `${armed}请于周一前报送。`
    expect(noteOfficialWritingAgentProse(next)).toBe('idle')
    expect(readOfficialWritingSession()?.update).toBeNull()
    setOfficialWritingView('session-writing')
    expect(noteOfficialWritingAgentProse(next)).toBe('applied')
    expect(readOfficialWritingSession()?.update).toBe(next)
  })
})
