import { act, render } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { officialWritingDocumentFromTask, officialWritingTaskText } from '../officialWriting'
import {
  armOfficialWritingDocument,
  noteOfficialWritingAgentProse,
  offerOfficialWritingAgentDocument,
  planOfficialWritingTyping,
  readOfficialWritingSession,
  resetOfficialWritingSessionForTests,
  revealOfficialWritingFrame
} from '../officialWritingAgentEdit'
import {
  OfficialWritingLiveDocument,
  OfficialWritingSessionPaper,
  useOfficialWritingSession
} from '../OfficialWritingLiveDocument'

function WritingHost() {
  const writing = useOfficialWritingSession()
  if (!writing) return null
  return <OfficialWritingLiveDocument document={writing.document} update={writing.update} opening={writing.opening} />
}

async function tick() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(24)
  })
}

afterEach(() => {
  resetOfficialWritingSessionForTests()
  vi.useRealTimers()
})

describe('officialWritingTyping', () => {
  it('reveals a multi-character insert one frame at a time at the caret', () => {
    const document = '各学校：通知。'
    const insertion = { at: '各学校：'.length, text: '周五开会' }
    const typing = planOfficialWritingTyping(document, insertion)

    expect(typing.caret).toBe(insertion.at)
    expect(typing.incoming).toBe(insertion.text)
    expect(typing.frames.length).toBeGreaterThan(1)
    expect(typing.frames[0]).not.toBe(typing.incoming)
    for (const frame of typing.frames) expect(typing.incoming.startsWith(frame)).toBe(true)
    for (let index = 1; index < typing.frames.length; index += 1) {
      expect(typing.frames[index]?.startsWith(typing.frames[index - 1] ?? '')).toBe(true)
      expect((typing.frames[index] ?? '').length).toBeGreaterThan((typing.frames[index - 1] ?? '').length)
    }
    expect(typing.frames.at(-1)).toBe(typing.incoming)
    for (const frame of typing.frames.slice(0, -1)) expect(frame.length).toBeLessThan(typing.incoming.length)

    for (const frame of typing.frames) {
      const shown = revealOfficialWritingFrame(document, typing, frame)
      expect(shown.startsWith('各学校：')).toBe(true)
      expect(shown.endsWith('通知。')).toBe(true)
      expect(shown).toBe(`各学校：${frame}通知。`)
    }
  })

  it('paints the caret and one typing frame on the same document node', () => {
    vi.useFakeTimers()
    const documentText = '各学校：'
    const { rerender } = render(<OfficialWritingLiveDocument document={documentText} update={null} />)
    const editor = document.querySelector('[data-writing-editor="live"]')
    expect(editor?.textContent).toBe(documentText)

    rerender(<OfficialWritingLiveDocument document={documentText} update={`${documentText}周五开会`} />)
    act(() => {
      vi.advanceTimersByTime(24)
    })

    const caret = document.querySelector('[data-writing-caret]')
    expect(caret).toHaveAttribute('data-writing-caret', String(documentText.length))
    expect(caret).toHaveAttribute('data-writing-frame', '周')
    expect(editor?.textContent).toContain('各学校：周')
    expect(document.querySelector('[data-writing-editor="live"]')).toBe(editor)

    act(() => {
      vi.advanceTimersByTime(24)
    })
    expect(document.querySelector('[data-writing-frame]')).toHaveAttribute('data-writing-frame', '周五')
    expect(editor?.textContent).toContain('各学校：周五')
    expect(document.querySelector('[data-writing-editor="live"]')).toBe(editor)
  })

  it('replaces a placeholder in place and keeps the surrounding text', () => {
    vi.useFakeTimers()
    const documentText = '各学校：\n（待填写）\n特此通知'
    const marker = '（待填写）'
    const update = documentText.replace(marker, '周五开会')
    const { rerender } = render(<OfficialWritingLiveDocument document={documentText} update={null} />)
    const editor = document.querySelector('[data-writing-editor="live"]')

    rerender(<OfficialWritingLiveDocument document={documentText} update={update} />)
    act(() => {
      vi.advanceTimersByTime(24)
    })

    const caret = document.querySelector('[data-writing-caret]')
    expect(caret).toHaveAttribute('data-writing-caret', String(documentText.indexOf(marker)))
    expect(caret).toHaveAttribute('data-writing-frame', '周')
    expect(editor?.textContent).toContain('各学校：')
    expect(editor?.textContent).toContain('特此通知')
    expect(editor?.textContent).toContain('周')
    expect(editor?.textContent).not.toContain(marker)
    expect(document.querySelector('[data-writing-editor="live"]')).toBe(editor)
  })

  it('types the opening draft one glyph at a time, then the queued follow-up', async () => {
    vi.useFakeTimers()
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局' })
    const documentText = officialWritingDocumentFromTask(task ?? '')
    const incoming = '请于周一前报送材料。'
    expect(documentText.includes(incoming)).toBe(false)
    const editor = () => document.querySelector('[data-writing-editor="live"]')
    render(<WritingHost />)
    await act(async () => {
      armOfficialWritingDocument(task ?? '')
      offerOfficialWritingAgentDocument(`${documentText}${incoming}`)
    })
    expect(readOfficialWritingSession()?.document).toBe(documentText)
    expect(readOfficialWritingSession()?.update).toBe(`${documentText}${incoming}`)
    expect(readOfficialWritingSession()?.opening).toBe(true)
    expect(editor()?.textContent).toBe('')
    const host = editor()

    for (let index = 0; index < documentText.length; index += 1) {
      await tick()
      const caret = document.querySelector('[data-writing-caret]')
      expect(caret).toHaveAttribute('data-writing-caret', '0')
      expect(caret).toHaveAttribute('data-writing-frame', documentText.slice(0, index + 1))
      expect(editor()).toBe(host)
    }
    expect(editor()?.textContent).toBe(documentText)
    expect(readOfficialWritingSession()?.opening).toBe(true)

    await tick()
    expect(document.querySelector('[data-writing-caret]')).toBeNull()
    expect(readOfficialWritingSession()?.opening).toBe(false)
    expect(readOfficialWritingSession()?.document).toBe(documentText)

    await tick()
    expect(document.querySelector('[data-writing-caret]')).toHaveAttribute(
      'data-writing-caret',
      String(documentText.length)
    )
    expect(document.querySelector('[data-writing-frame]')).toHaveAttribute('data-writing-frame', incoming.slice(0, 1))

    for (let index = 1; index < incoming.length; index += 1) await tick()
    expect(document.querySelector('[data-writing-frame]')).toHaveAttribute('data-writing-frame', incoming)
    expect(editor()?.textContent).toBe(`${documentText}${incoming}`)

    await tick()
    expect(document.querySelector('[data-writing-caret]')).toBeNull()
    expect(editor()?.textContent).toBe(`${documentText}${incoming}`)
    expect(readOfficialWritingSession()?.document).toBe(`${documentText}${incoming}`)
    expect(readOfficialWritingSession()?.update).toBeNull()
    expect(editor()).toBe(host)
  })

  it('keeps a rejected follow-up off the paper while the opening caret is moving', async () => {
    vi.useFakeTimers()
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局' })
    const documentText = officialWritingDocumentFromTask(task ?? '')
    render(<WritingHost />)
    await act(async () => {
      armOfficialWritingDocument(task ?? '')
    })
    expect(noteOfficialWritingAgentProse('另一份完全不同的稿')).toBe('rejected')
    expect(readOfficialWritingSession()?.update).toBeNull()
    await tick()
    expect(document.querySelector('[data-writing-frame]')).toHaveAttribute(
      'data-writing-frame',
      documentText.slice(0, 1)
    )
    expect(document.querySelector('[data-writing-editor="live"]')?.textContent).not.toContain('另一份')
  })

  it('types the opening draft with a caret only on the armed task', async () => {
    vi.useFakeTimers()
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局' })
    const documentText = officialWritingDocumentFromTask(task ?? '')
    render(<OfficialWritingSessionPaper sessionId="other-task" />)
    await act(async () => {
      armOfficialWritingDocument(task ?? '', 'session-writing')
    })
    expect(document.querySelector('[data-writing-editor="live"]')).toBeNull()

    render(<OfficialWritingSessionPaper sessionId="session-writing" />)
    const editor = document.querySelector('[data-writing-editor="live"]')
    expect(editor?.textContent).toBe('')
    await tick()
    const caret = document.querySelector('[data-writing-caret]')
    expect(caret).toHaveAttribute('data-writing-caret', '0')
    expect(caret).toHaveAttribute('data-writing-frame', documentText.slice(0, 1))
    expect(caret).toHaveClass('writing-agent-caret')
  })
})
