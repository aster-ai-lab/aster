import { createHash, webcrypto } from 'node:crypto'
import { readFileSync } from 'node:fs'
import process from 'node:process'

import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { readUIMessageStream, type UIMessageChunk } from 'ai'
import i18n from 'i18next'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import type { FormPartData } from '@shared/data/types/uiParts'
import { agentFormPartSchema } from '@shared/types/agentForm'

import { AgentFormSubmitContext, type AgentFormSubmit } from '../agentFormSubmit'
import { FormPartView } from '../FormPartView'

vi.unmock('@a3s-lab/ui')

function question(): FormPartData {
  const spec = JSON.stringify({
    question: '选择文档',
    choices: [{ id: 'question-1', label: 'B.docx' }],
    allowFreeText: true
  })
  const id = 'a'.repeat(64)
  return {
    id,
    spec,
    format: 'question',
    state: 'form-requested',
    binding: {
      protocolVersion: 1,
      requestId: id,
      revision: `sha256:${createHash('sha256').update(spec).digest('hex')}`,
      owner: {
        sessionId: 'session',
        executionId: 'model',
        attemptId: 1,
        anchorMessageId: 'assistant',
        acpSessionId: 'acp',
        promptId: 'prompt'
      }
    }
  }
}

function ui(data: FormPartData, channel: AgentFormSubmit) {
  return (
    <AgentFormSubmitContext value={channel}>
      <FormPartView part={{ data }} />
    </AgentFormSubmitContext>
  )
}

beforeEach(async () => {
  vi.stubGlobal('crypto', webcrypto)
  await i18n.changeLanguage('zh-CN')
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const nativeArtifact = process.env.A3S_STUDIO_QUESTION_RECOVERY_OUTPUT
it.skipIf(!nativeArtifact)('recovers a closed card from actual native expiry chunks through the SDK', async () => {
  const artifact = JSON.parse(readFileSync(nativeArtifact!, 'utf8')) as {
    requested: UIMessageChunk[]
    closed: UIMessageChunk[]
  }
  expect(artifact.closed).toHaveLength(1)
  const stream = new globalThis.ReadableStream({
    start(controller) {
      for (const chunk of [...artifact.requested, ...artifact.closed]) controller.enqueue(chunk)
      controller.close()
    }
  })
  let card: FormPartData | undefined
  for await (const message of readUIMessageStream({ stream })) {
    const part = message.parts.find((part) => part.type === 'data-form')
    if (part) card = agentFormPartSchema.parse((part as { data: unknown }).data)
  }
  expect(card?.state).toBe('closed')
  const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
  render(ui(card!, channel))
  expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.closed'))
  expect(screen.queryByRole('button')).not.toBeInTheDocument()
  expect(channel).not.toHaveBeenCalled()
})

it('keeps a closed native question noninteractive after remount and preserves its question text', () => {
  const data = { ...question(), state: 'closed' as const }
  const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
  const first = render(ui(data, channel))
  expect(screen.getByRole('heading', { name: '选择文档' })).toBeInTheDocument()
  expect(screen.queryByRole('button')).not.toBeInTheDocument()
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.closed'))
  first.unmount()
  render(ui(data, channel))
  expect(screen.queryByRole('button')).not.toBeInTheDocument()
  expect(channel).not.toHaveBeenCalled()
})

it('withdraws a sending question when it closes and ignores its late acknowledgment', async () => {
  let acknowledge!: (result: 'sent') => void
  const channel = vi.fn<AgentFormSubmit>().mockImplementation(
    () =>
      new Promise((resolve) => {
        acknowledge = resolve
      })
  )
  const data = question()
  const view = render(ui(data, channel))
  const user = userEvent.setup()
  await user.type(screen.getByRole('textbox'), 'B.docx')
  await user.click(screen.getByRole('button', { name: i18n.t('agent.askUserQuestion.submit') }))
  await waitFor(() => expect(channel.mock.calls).toHaveLength(1))
  view.rerender(ui({ ...data, state: 'closed' }, channel))
  expect(channel.mock.calls[0][1].aborted).toBe(true)
  await act(async () => acknowledge('sent'))
  expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.closed'))
  expect(screen.queryByRole('button')).not.toBeInTheDocument()
  expect(screen.queryByText(i18n.t('agent.form.answered'))).not.toBeInTheDocument()
})

it('restores a still-pending question after remount and requires an explicit answer for the same owner', async () => {
  const data = question()
  const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
  const initial = render(ui(data, channel))
  initial.unmount()
  const restored = render(ui(data, channel))
  expect(channel).not.toHaveBeenCalled()
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: '选择文档' }))
  await user.click(screen.getByRole('option', { name: 'B.docx' }))
  await user.click(screen.getByRole('button', { name: i18n.t('agent.askUserQuestion.submit') }))
  await waitFor(() =>
    expect(channel).toHaveBeenCalledWith(
      { binding: data.binding, values: { choice: 'question-1', answer: '' } },
      expect.any(globalThis.AbortSignal)
    )
  )
  restored.rerender(ui({ ...data, state: 'answered', values: { choice: 'question-1' } }, channel))
  expect(screen.getByText('B.docx')).toBeInTheDocument()
  expect(screen.queryByRole('button')).not.toBeInTheDocument()
})
