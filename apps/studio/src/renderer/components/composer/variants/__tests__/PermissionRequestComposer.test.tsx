import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type * as ReactI18next from 'react-i18next'
import { describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'
import type { NormalToolResponse } from '@renderer/types/mcpTool'
import type { CherryMessagePart } from '@shared/data/types/message'

import PermissionRequestComposer, { type PermissionRequestComposerRequest } from '../PermissionRequestComposer'

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof ReactI18next>()),
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const template =
        {
          'agent.toolPermission.addNote': 'Add a note',
          'agent.toolPermission.defaultDenyMessage': 'User denied permission for this tool.',
          'agent.toolPermission.confirmAction': 'Allow this action?',
          'agent.toolPermission.decisionTitle': 'Allow {{target}}?',
          'agent.toolPermission.runCommand': 'Run this command?',
          'agent.toolPermission.error.sendFailed': 'Failed to send your decision. Please try again.',
          'agent.toolPermission.reasonLabel': 'Reason for rejection (optional)',
          'agent.toolPermission.reasonPlaceholder': 'Tell the Agent what to do instead',
          'agent.toolPermission.sendingAllow': 'Allowing...',
          'agent.toolPermission.sendingDeny': 'Denying...',
          'agent.toolPermission.confirmation': 'Allow tool call?',
          'agent.toolPermission.inputPreview': 'Tool input preview',
          'agent.toolPermission.button.allow': 'Allow',
          'agent.toolPermission.button.deny': 'Deny',
          'agent.toolPermission.button.run': 'Run',
          'agent.toolPermission.waiting': 'Waiting for tool permission decision...',
          'message.processing': 'Processing',
          'message.tools.activity.checking': 'Checking',
          'message.tools.activity.projectChecks': 'project checks',
          'message.tools.activity.relatedContent': 'related content',
          'message.tools.activity.searching': 'Searching',
          'message.tools.activity.usingExtension': 'Bringing in an extension',
          'message.tools.labels.mcpServerTool': 'MCP Server Tool',
          'message.tools.labels.tool': 'Tool',
          'message.tools.sections.input': 'Input'
        }[key] ?? key

      if (!options) return template
      return template.replace(/\{\{(\w+)}}/g, (_match, name: string) => String(options[name] ?? ''))
    }
  })
}))

vi.mock('@renderer/components/CodeViewer', () => ({
  default: ({ maxHeight, value }: { maxHeight?: number; value: string }) => (
    <div data-max-height={maxHeight} data-testid="code-viewer">
      {value}
    </div>
  )
}))

const part = {
  type: 'tool-CustomTool',
  toolName: 'CustomTool',
  toolCallId: 'call-1',
  state: 'approval-requested',
  input: { command: 'pnpm test' },
  approval: { id: 'approval-1' }
} as unknown as CherryMessagePart

async function openNote(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Add a note' }))
  return screen.getByRole('textbox', { name: 'Reason for rejection (optional)' })
}

function makeRequest(overrides: Partial<PermissionRequestComposerRequest> = {}): PermissionRequestComposerRequest {
  const toolResponse: NormalToolResponse = {
    id: 'call-1',
    toolCallId: 'call-1',
    status: 'pending',
    arguments: { command: 'pnpm test' },
    tool: {
      id: 'call-1',
      name: 'CustomTool',
      type: 'builtin'
    }
  }

  return {
    messageId: 'message-1',
    toolCallId: 'call-1',
    approvalId: 'approval-1',
    title: 'CustomTool',
    toolResponse,
    match: {
      part,
      state: 'approval-requested',
      toolCallId: 'call-1',
      messageId: 'message-1',
      approvalId: 'approval-1',
      input: { command: 'pnpm test' }
    },
    ...overrides
  }
}

describe('PermissionRequestComposer', () => {
  it('marks the root panel as a composer viewport inset target', () => {
    const { container } = render(<PermissionRequestComposer request={makeRequest()} onRespond={vi.fn()} />)

    expect(container.firstElementChild).toHaveAttribute('data-composer-viewport-inset-target', '')
  })

  it('submits an approval decision', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(
      <PermissionRequestComposer
        request={makeRequest({ title: 'Allow CustomTool to run focused tests?' })}
        onRespond={onRespond}
      />
    )

    expect(screen.getByRole('heading', { name: 'Allow CustomTool?' })).toBeInTheDocument()
    expect(screen.getByText('Allow CustomTool to run focused tests?')).toBeInTheDocument()
    expect(screen.queryByText('Tool input preview')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Allow' }))

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: true
    })
  })

  it('submits a denial decision with the default deny reason', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    fireEvent.click(screen.getByRole('button', { name: 'Deny' }))

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'User denied permission for this tool.'
    })
  })

  it('trims and submits user feedback without changing the denial decision', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    await user.type(await openNote(user), '  use a copy instead  ')
    await user.click(screen.getByRole('button', { name: 'Deny' }))

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'use a copy instead'
    })
  })

  it('withdraws a typed note when the receiver supports only a boolean decision', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    const request = makeRequest()
    const view = render(<PermissionRequestComposer request={request} onRespond={onRespond} />)
    await user.type(await openNote(user), 'use a copy instead')
    view.rerender(<PermissionRequestComposer request={request} onRespond={onRespond} allowRejectionReason={false} />)
    expect(screen.queryByRole('textbox', { name: 'Reason for rejection (optional)' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Add a note' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Deny' }))
    expect(onRespond).toHaveBeenCalledWith({ match: request.match, approved: false })
  })

  it('renders MCP tool name with the argument preview', () => {
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: 'lookup_docs',
          toolResponse: {
            id: 'mcp-call-1',
            toolCallId: 'mcp-call-1',
            status: 'pending',
            arguments: { query: 'composer' },
            tool: {
              id: 'docs-server__lookup_docs',
              name: 'lookup_docs',
              description: 'Search project documentation.',
              type: 'mcp',
              serverId: 'docs-server',
              serverName: 'Docs',
              inputSchema: { type: 'object', properties: {}, required: [] }
            }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    expect(screen.getByRole('heading', { name: 'Allow lookup_docs?' })).toBeInTheDocument()
    expect(screen.getByText('Search project documentation.')).toBeInTheDocument()
    expect(screen.getByTestId('permission-preview')).not.toHaveClass('overflow-y-auto')
    expect(screen.getByTestId('permission-mcp-args-scroll')).toHaveClass('max-h-60', 'overflow-y-auto')
    expect(screen.queryByText('Docs : lookup_docs')).not.toBeInTheDocument()
    expect(screen.getByText('query')).toBeInTheDocument()
    expect(screen.getByText('composer')).toBeInTheDocument()
  })

  it('shows a command question once, as the command', () => {
    const question = '是否执行这条命令：printf a3s-code-prompt-ok ?'
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: question,
          toolResponse: {
            id: 'call-1',
            toolCallId: 'call-1',
            status: 'pending',
            arguments: {},
            tool: { id: 'call-1', name: question, type: 'builtin' }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    expect(screen.getByRole('heading', { name: 'Run this command?' })).toBeInTheDocument()
    expect(screen.getByTestId('permission-command')).toHaveTextContent('printf a3s-code-prompt-ok')
    expect(screen.getAllByText('printf a3s-code-prompt-ok')).toHaveLength(1)
    expect(screen.queryByText(question)).not.toBeInTheDocument()
    expect(screen.queryByText('Tool')).not.toBeInTheDocument()
  })

  it('bounds builtin previews that do not own their own scroll region', () => {
    render(<PermissionRequestComposer request={makeRequest()} onRespond={vi.fn()} />)

    expect(screen.getByTestId('permission-preview')).not.toHaveClass('overflow-y-auto')
    expect(screen.getByTestId('permission-builtin-body-scroll')).toHaveClass('max-h-60', 'overflow-y-auto')
  })

  it('renders the ExitPlanMode plan in the approval preview', () => {
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: 'ExitPlanMode',
          toolResponse: {
            id: 'exit-plan-call-1',
            toolCallId: 'exit-plan-call-1',
            status: 'pending',
            arguments: { plan: '# Release plan\n\n1. Run the focused tests' },
            tool: { id: 'ExitPlanMode', name: 'ExitPlanMode', type: 'builtin' }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    const preview = screen.getByTestId('permission-preview')
    expect(preview).toHaveTextContent('Release plan')
    expect(preview).toHaveTextContent('Run the focused tests')
  })

  it('does not add a fallback body scroller when the tool content owns scrolling', () => {
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: 'Write',
          toolResponse: {
            id: 'write-call-1',
            toolCallId: 'write-call-1',
            status: 'pending',
            arguments: {
              file_path: '/tmp/cherry-approval-long-preview-note.md',
              content: '# Long approval preview\n\nA long document body.'
            },
            tool: {
              id: 'Write',
              name: 'Write',
              type: 'builtin'
            }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    expect(screen.getByTestId('code-viewer')).toHaveAttribute('data-max-height', '240')
    expect(screen.queryByTestId('permission-builtin-body-scroll')).not.toBeInTheDocument()
  })

  it('uses the streaming tool icon and semantic title for the approval header', () => {
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: 'Bash',
          toolResponse: {
            id: 'bash-call-1',
            toolCallId: 'bash-call-1',
            status: 'pending',
            arguments: { command: 'pnpm test' },
            tool: {
              id: 'Bash',
              name: 'Bash',
              type: 'builtin'
            }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    const heading = screen.getByRole('heading', { name: 'Allow Bash?' })
    expect(heading.querySelector('.lucide-square-terminal')).toBeInTheDocument()
    expect(screen.queryByText('Allow tool call?')).not.toBeInTheDocument()
  })

  it('hides the request subtitle when it only repeats the tool name', () => {
    render(<PermissionRequestComposer request={makeRequest()} onRespond={vi.fn()} />)

    expect(screen.getByRole('heading', { name: 'Allow CustomTool?' })).toBeInTheDocument()
    expect(screen.getAllByText('CustomTool')).toHaveLength(1)
  })

  it('approves when Enter is pressed outside editable controls', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    fireEvent.keyDown(document, { key: 'Enter' })

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: true
    })
  })

  it('denies when Escape is pressed outside editable controls', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    fireEvent.keyDown(document, { key: 'Escape' })

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'User denied permission for this tool.'
    })
  })

  it('denies with the typed reason when Enter is pressed inside the rejection reason', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    const reason = await openNote(user)
    await user.click(reason)
    await user.keyboard('use a copy instead{Enter}')

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'use a copy instead'
    })
    expect(reason).toHaveValue('use a copy instead')
  })

  it('approves when Enter is pressed inside an empty rejection reason', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    const reason = await openNote(user)
    await user.click(reason)
    await user.keyboard('{Enter}')

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: true
    })
  })

  it('denies when Escape is pressed inside the rejection reason', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    const reason = await openNote(user)
    await user.click(reason)
    await user.keyboard('{Escape}')

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'User denied permission for this tool.'
    })
  })

  it.each(['Enter', 'Escape'])('leaves IME %s in the note to the input method', async (key) => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)
    const note = await openNote(userEvent.setup())

    fireEvent.compositionStart(note)
    expect(fireEvent.keyDown(note, { key, keyCode: key === 'Enter' ? 13 : 27, isComposing: false })).toBe(true)
    expect(onRespond).not.toHaveBeenCalled()

    fireEvent.compositionEnd(note)
    expect(fireEvent.keyDown(note, { key, keyCode: 229, isComposing: false })).toBe(true)
    expect(onRespond).not.toHaveBeenCalled()

    fireEvent.keyDown(note, { key })
    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith(expect.objectContaining({ approved: key === 'Enter' }))
  })

  it.each(['Enter', 'Escape'])('ignores a legacy IME %s in the global permission hotkey', async (key) => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    expect(fireEvent.keyDown(document, { key, keyCode: 229, isComposing: false })).toBe(true)
    expect(onRespond).not.toHaveBeenCalled()

    fireEvent.keyDown(document, { key })
    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith(expect.objectContaining({ approved: key === 'Enter' }))
  })

  it('keeps Shift+Enter as a newline inside the rejection reason', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    const reason = await openNote(user)
    await user.click(reason)
    await user.keyboard('use a copy{Shift>}{Enter}{/Shift}instead')

    expect(onRespond).not.toHaveBeenCalled()
    expect(reason).toHaveValue('use a copy\ninstead')
  })

  it('denies rather than approves when Enter is pressed outside a filled rejection reason', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    await user.type(await openNote(user), 'use a copy instead')
    fireEvent.keyDown(document, { key: 'Enter' })

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(onRespond).toHaveBeenCalledWith({
      match: makeRequest().match,
      approved: false,
      reason: 'use a copy instead'
    })
  })

  it('moves the Enter hint onto Deny once a rejection reason is typed', async () => {
    const user = userEvent.setup()
    render(<PermissionRequestComposer request={makeRequest()} onRespond={vi.fn()} />)

    expect(screen.getByRole('button', { name: /Allow/ })).toHaveTextContent('Enter')
    expect(screen.getByRole('button', { name: /Deny/ })).toHaveTextContent('Esc')

    await user.type(await openNote(user), 'use a copy instead')

    expect(screen.getByRole('button', { name: /Allow/ })).not.toHaveTextContent('Enter')
    expect(screen.getByRole('button', { name: /Deny/ })).toHaveTextContent('Enter')
  })

  it('does not carry a rejection reason into the next approval request', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn().mockResolvedValue(undefined)
    const { rerender } = render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    await user.type(await openNote(user), 'use a copy instead')

    rerender(
      <PermissionRequestComposer
        request={makeRequest({ approvalId: 'approval-2', match: { ...makeRequest().match, approvalId: 'approval-2' } })}
        onRespond={onRespond}
      />
    )

    expect(screen.queryByRole('textbox', { name: 'Reason for rejection (optional)' })).not.toBeInTheDocument()
    expect(await openNote(user)).toHaveValue('')
  })

  it('shows progress and resets actions when the next approval becomes active', async () => {
    const user = userEvent.setup()
    const onRespond = vi.fn(() => new Promise<void>(() => undefined))
    const { rerender } = render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    await user.click(screen.getByRole('button', { name: 'Allow' }))

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    expect(screen.getByRole('status')).toHaveTextContent('Allowing...')
    expect(screen.queryByRole('button', { name: 'Allow' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Deny' })).not.toBeInTheDocument()

    const nextPart = {
      ...part,
      toolCallId: 'call-2',
      approval: { id: 'approval-2' }
    } as unknown as CherryMessagePart
    const nextRequest = makeRequest({
      toolCallId: 'call-2',
      approvalId: 'approval-2',
      match: {
        ...makeRequest().match,
        part: nextPart,
        toolCallId: 'call-2',
        approvalId: 'approval-2'
      }
    })
    rerender(<PermissionRequestComposer request={nextRequest} onRespond={onRespond} />)

    expect(screen.getByRole('status')).toBeEmptyDOMElement()
    expect(screen.getByRole('button', { name: 'Allow' })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: 'Deny' })).not.toBeDisabled()
  })

  it('re-enables the request when submitting the response fails', async () => {
    const onRespond = vi.fn().mockRejectedValue(new Error('failed'))
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    fireEvent.click(screen.getByRole('button', { name: 'Allow' }))

    await waitFor(() => expect(onRespond).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Failed to send your decision. Please try again.'))
    expect(screen.getByRole('status')).toBeEmptyDOMElement()
    expect(screen.getByRole('button', { name: 'Allow' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Deny' })).toBeInTheDocument()
  })

  it('names the denial while that decision is sending', async () => {
    const onRespond = vi.fn(() => new Promise<void>(() => undefined))
    render(<PermissionRequestComposer request={makeRequest()} onRespond={onRespond} />)

    fireEvent.click(screen.getByRole('button', { name: 'Deny' }))

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Denying...'))
    expect(screen.queryByRole('button', { name: 'Allow' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Deny' })).not.toBeInTheDocument()
  })

  it('asks to allow the named skill and shows what will run', () => {
    const prompt = 'Explain what an agent is'
    render(
      <PermissionRequestComposer
        request={makeRequest({
          title: prompt,
          toolResponse: {
            id: 'skill-call-1',
            toolCallId: 'skill-call-1',
            status: 'pending',
            arguments: { skill: 'stc-answer', args: prompt },
            tool: { id: 'Skill', name: 'Skill', type: 'builtin' }
          }
        })}
        onRespond={vi.fn()}
      />
    )

    expect(screen.getByRole('heading', { name: 'Allow stc-answer?' })).toBeInTheDocument()
    expect(screen.getByTestId('permission-preview')).toHaveTextContent('stc-answer')
    expect(screen.getByTestId('permission-skill-run')).toHaveTextContent(prompt)
    expect(screen.queryByText('Skill')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add a note' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Allow/ })).toHaveTextContent('Enter')
    expect(screen.getByRole('button', { name: /Deny/ })).toHaveTextContent('Esc')
  })
})
