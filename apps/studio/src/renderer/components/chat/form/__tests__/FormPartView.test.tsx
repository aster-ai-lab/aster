import { createHash, webcrypto } from 'node:crypto'

import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { FormPartData } from '@shared/data/types/uiParts'
import type { AgentFormBinding } from '@shared/types/agentForm'

import { IntelligentUiSubmitContext, type IntelligentUiSubmitOutcome } from '../../messages/markdown/intelligentUiHost'
import { AgentFormSubmitContext, type AgentFormSubmit } from '../agentFormSubmit'
import { FormPartView } from '../FormPartView'

vi.unmock('@a3s-lab/ui')

const source = [
  'root = Card("Details", [count, submit])',
  'count = Input("count", "Count", 3, "number")',
  'submit = Button("submit", "Continue")'
].join('\n')
function form(spec = source, requestId = 'a'.repeat(64)): FormPartData {
  const binding: AgentFormBinding = {
    protocolVersion: 1,
    requestId,
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
  return { id: requestId, binding, format: 'openui', spec, state: 'form-requested' }
}

function ui(data: FormPartData, channel?: AgentFormSubmit, nextTurn = vi.fn(async () => 'sent' as const)) {
  return (
    <IntelligentUiSubmitContext value={nextTurn}>
      <AgentFormSubmitContext value={channel}>
        <FormPartView part={{ data }} />
      </AgentFormSubmitContext>
    </IntelligentUiSubmitContext>
  )
}

beforeEach(() => vi.stubGlobal('crypto', webcrypto))
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('owned pending forms', () => {
  it('keeps malformed typed questions noninteractive even when their binding is valid', () => {
    const spec = JSON.stringify({ question: 'Which?', choices: [], allowFreeText: false })
    render(ui({ ...form(spec), format: 'question' }, vi.fn<AgentFormSubmit>().mockResolvedValue('sent')))
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.invalid'))
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })

  it('waits for an explicit document choice and submits B with the native binding', async () => {
    const user = userEvent.setup()
    const spec = JSON.stringify({
      question: '选哪份文档？',
      allowFreeText: true,
      choices: [
        { id: 'question-0', label: 'A.docx' },
        { id: 'question-1', label: 'B.docx' }
      ]
    })
    const data: FormPartData = { ...form(spec), format: 'question' }
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    const nextTurn = vi.fn(async () => 'sent' as const)
    render(ui(data, channel, nextTurn))
    await user.click(screen.getByRole('button', { name: i18n.t('agent.askUserQuestion.submit') }))
    expect(channel).not.toHaveBeenCalled()
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.required'))
    await user.click(screen.getByRole('button', { name: '选哪份文档？' }))
    await user.click(screen.getByRole('option', { name: 'B.docx' }))
    await user.click(screen.getByRole('button', { name: i18n.t('agent.askUserQuestion.submit') }))
    await waitFor(() =>
      expect(channel).toHaveBeenCalledWith(
        { binding: data.binding, values: { choice: 'question-1', answer: '' } },
        expect.any(globalThis.AbortSignal)
      )
    )
    expect(nextTurn).not.toHaveBeenCalled()
  })

  it('accepts a text-only answer and displays the actual selected label after settling', async () => {
    const user = userEvent.setup()
    const question = { question: '请指定文档', allowFreeText: true, choices: [] }
    const data: FormPartData = { ...form(JSON.stringify(question)), format: 'question' }
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    const rendered = render(ui(data, channel))
    expect(screen.queryByRole('button', { name: '请指定文档' })).not.toBeInTheDocument()
    await user.type(screen.getByRole('textbox'), '请审查 B.docx')
    await user.click(screen.getByRole('button', { name: i18n.t('agent.askUserQuestion.submit') }))
    await waitFor(() => expect(channel.mock.calls[0][0].values).toEqual({ choice: '', answer: '请审查 B.docx' }))
    const settled = {
      ...data,
      spec: JSON.stringify({ ...question, allowFreeText: false, choices: [{ id: 'question-1', label: 'B.docx' }] }),
      state: 'answered' as const,
      values: { choice: 'question-1' }
    }
    rendered.rerender(ui(settled, channel))
    expect(screen.getByText('B.docx')).toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('keeps a retired plugin-named source tag read-only even with a valid native binding', () => {
    render(ui({ ...form(), format: 'aix-ui' }, vi.fn<AgentFormSubmit>().mockResolvedValue('sent')))
    expect(screen.getByText(source, { normalizer: (value) => value })).toBeInTheDocument()
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })
  it('submits typed values with the exact native binding without starting a second turn', async () => {
    const user = userEvent.setup()
    const data = form()
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    const nextTurn = vi.fn(async () => 'sent' as const)
    render(ui(data, channel, nextTurn))
    await user.clear(screen.getByRole('spinbutton', { name: 'Count' }))
    await user.type(screen.getByRole('spinbutton', { name: 'Count' }), '0')
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() =>
      expect(channel).toHaveBeenCalledWith(
        { binding: data.binding, values: { count: 0 } },
        expect.any(globalThis.AbortSignal)
      )
    )
    expect(await screen.findByRole('button', { name: i18n.t('agent.form.submitted') })).toBeDisabled()
    expect(nextTurn).not.toHaveBeenCalled()
  })

  it('blocks an empty required date and submits the filled field on the same turn', async () => {
    const user = userEvent.setup()
    const spec = [
      'root = Card("期", [when, submit])',
      'when = Textarea("when", "日期", "", true)',
      'submit = Button("submit", "提交")'
    ].join('\n')
    const data = form(spec)
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    const nextTurn = vi.fn(async () => 'sent' as const)
    render(ui(data, channel, nextTurn))
    await user.click(screen.getByRole('button', { name: '提交' }))
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.required'))
    expect(channel).not.toHaveBeenCalled()
    expect(nextTurn).not.toHaveBeenCalled()
    await user.type(screen.getByRole('textbox', { name: '日期' }), '下周三 15:00')
    await user.click(screen.getByRole('button', { name: '提交' }))
    await waitFor(() =>
      expect(channel).toHaveBeenCalledWith(
        { binding: data.binding, values: { when: '下周三 15:00' } },
        expect.any(globalThis.AbortSignal)
      )
    )
    expect(nextTurn).not.toHaveBeenCalled()
  })

  it('does not fall back to conversation continuation when the form submit channel is unavailable', async () => {
    const user = userEvent.setup()
    const nextTurn = vi.fn(async () => 'sent' as const)
    render(ui(form(), undefined, nextTurn))
    const button = screen.getByRole('button', { name: 'Continue' })
    expect(button).toBeDisabled()
    await user.click(button)
    expect(nextTurn).not.toHaveBeenCalled()
  })

  it('allows numeric editing through an empty draft but never submits an old initial value', async () => {
    const user = userEvent.setup()
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    render(ui(form(), channel))
    await user.clear(screen.getByRole('spinbutton', { name: 'Count' }))
    expect(screen.getByRole('spinbutton', { name: 'Count' })).toHaveValue(null)
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.required'))
    expect(channel).not.toHaveBeenCalled()
    await user.type(screen.getByRole('spinbutton', { name: 'Count' }), '0')
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(channel.mock.calls[0][0].values).toEqual({ count: 0 }))
  })

  it('keeps unbound historical forms source-only even with a continuation provider', () => {
    const data = form()
    delete data.binding
    render(ui(data))
    expect(screen.getByText(source, { normalizer: (value) => value })).toBeInTheDocument()
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('refuses submitted values when the displayed content does not match the native revision', async () => {
    const data = { ...form(), spec: source.replace('Details', 'Changed') }
    const channel = vi.fn<AgentFormSubmit>().mockResolvedValue('sent')
    render(ui(data, channel))
    await userEvent.setup().click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(i18n.t('agent.form.sendFailed')))
    expect(channel).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled()
  })

  it('cancels the old card and resets edits when an owner is replaced, ignoring its late completion', async () => {
    const user = userEvent.setup()
    let complete!: (outcome: IntelligentUiSubmitOutcome) => void
    const channel = vi
      .fn<AgentFormSubmit>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            complete = resolve
          })
      )
      .mockResolvedValue('sent')
    const initial = form()
    const rendered = render(ui(initial, channel))
    await user.clear(screen.getByRole('spinbutton', { name: 'Count' }))
    await user.type(screen.getByRole('spinbutton', { name: 'Count' }), '9')
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(channel.mock.calls).toHaveLength(1))
    const next = form(source.replace('Count", 3', 'Count", 2'), 'b'.repeat(64))
    rendered.rerender(ui(next, channel))
    expect(channel.mock.calls[0][1].aborted).toBe(true)
    expect(screen.getByRole('spinbutton', { name: 'Count' })).toHaveValue(2)
    await act(async () => complete('sent'))
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled()
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() =>
      expect(channel).toHaveBeenLastCalledWith(
        { binding: next.binding, values: { count: 2 } },
        expect.any(globalThis.AbortSignal)
      )
    )
  })

  it('fences a source ABA even when a host request identity is unchanged', async () => {
    const user = userEvent.setup()
    let complete!: (outcome: IntelligentUiSubmitOutcome) => void
    const channel = vi.fn<AgentFormSubmit>().mockImplementation(
      () =>
        new Promise((resolve) => {
          complete = resolve
        })
    )
    const initial = form()
    const rendered = render(ui(initial, channel))
    await user.type(screen.getByRole('spinbutton', { name: 'Count' }), '9')
    await user.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(channel.mock.calls).toHaveLength(1))
    rendered.rerender(ui({ ...initial, spec: source.replace('Details', 'Changed') }, channel))
    rendered.rerender(ui(initial, channel))
    expect(channel.mock.calls[0][1].aborted).toBe(true)
    expect(screen.getByRole('spinbutton', { name: 'Count' })).toHaveValue(3)
    await act(async () => complete('sent'))
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled()
  })

  it('retains zero, false and text in answered read-only summaries', () => {
    render(ui({ ...form(), state: 'answered', values: { count: 0, urgent: false, topic: 'Work' } }))
    for (const value of ['0', i18n.t('agent.form.no'), 'Work']) expect(screen.getByText(value)).toBeInTheDocument()
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('labels answered values with the form title and field labels, and marks an empty answer', () => {
    render(ui({ ...form(), state: 'answered', values: { count: '' } }))
    expect(screen.getByRole('heading', { name: 'Details' })).toBeInTheDocument()
    expect(screen.getByText('Count')).toBeInTheDocument()
    expect(screen.queryByText('count')).not.toBeInTheDocument()
    expect(screen.getByText(i18n.t('agent.form.empty'))).toBeInTheDocument()
  })

  it('stacks the fields of a gridded form one per row', () => {
    const spec = [
      'root = Card("Confirm", [row, go])',
      'row = Grid(2, [include, name])',
      'include = Checkbox("include", "Include", true)',
      'name = Input("name", "Name")',
      'go = Button("submit", "Continue")'
    ].join('\n')
    const { container } = render(ui(form(spec), vi.fn<AgentFormSubmit>().mockResolvedValue('sent')))
    expect(screen.getByRole('checkbox', { name: 'Include' })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Name' })).toBeInTheDocument()
    expect(container.querySelector('[style*="grid-template-columns"]')).toBeNull()
  })
})
