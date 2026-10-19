import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { McpServer } from '@shared/data/types/mcpServer'

import QuickCreateMcpServerDialog from '../QuickCreateMcpServerDialog'

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<object>()

  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => key
    })
  }
})

const existing = [{ id: '1', name: 'taken' }] as McpServer[]

function setup(onCreate = vi.fn().mockResolvedValue(undefined)) {
  const onOpenChange = vi.fn()
  const { rerender } = render(
    <QuickCreateMcpServerDialog open onOpenChange={onOpenChange} existingServers={existing} onCreate={onCreate} />
  )
  const setOpen = (open: boolean) =>
    rerender(
      <QuickCreateMcpServerDialog
        open={open}
        onOpenChange={onOpenChange}
        existingServers={existing}
        onCreate={onCreate}
      />
    )

  return { onCreate, onOpenChange, setOpen, user: userEvent.setup() }
}

const submit = () => screen.getByRole('button', { name: 'common.add' })

describe('QuickCreateMcpServerDialog', () => {
  it('marks the current required fields', async () => {
    const { user } = setup()

    expect(screen.getByLabelText(/settings.mcp.name/)).toBeRequired()
    expect(screen.getByLabelText(/settings.mcp.command/)).toBeRequired()
    // required 标记由关联 input 携带（FormLabel 渲染 * 但不透传 required 属性；
    // type 是 Select，其必选语义由表单校验层承载，无 DOM required 属性）。
    expect(screen.getByLabelText(/settings.mcp.name/)).toHaveAttribute('required')
    expect(screen.getByLabelText(/settings.mcp.command/)).toHaveAttribute('required')

    // Radix Select：trigger 以 keyDown（ArrowDown/Space/Enter）开启
    const typeTrigger = screen.getAllByRole('combobox')[0]
    typeTrigger.focus()
    fireEvent.keyDown(typeTrigger, { key: 'ArrowDown' })
    const sseOption = await screen.findByRole('option', { name: 'settings.mcp.sse' })
    // userEvent 的坐标命中会落到 dialog 外（portal 无布局）→ 关闭对话框；fireEvent 绕开命中检测
    fireEvent.click(sseOption)

    expect(await screen.findByLabelText(/settings.mcp.url/)).toBeRequired()
  })

  it('blocks submission until the required fields are filled', async () => {
    const { onCreate, user } = setup()

    await user.click(submit())

    await waitFor(() => expect(document.querySelectorAll('[data-slot="form-message"]').length).toBeGreaterThan(0))
    expect(onCreate).not.toHaveBeenCalled()
  })

  it('creates a stdio server with args available before the advanced section', async () => {
    const { onCreate, onOpenChange, user } = setup()

    await user.type(screen.getByLabelText(/settings.mcp.name/), 'my-server')
    await user.type(screen.getByLabelText(/settings.mcp.command/), 'npx')
    await user.type(screen.getAllByLabelText(/settings.mcp.args/)[0], '-y{enter}mcp-server-example')

    await user.click(submit())

    await waitFor(() => expect(onCreate).toHaveBeenCalledTimes(1))
    expect(onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'my-server',
        type: 'stdio',
        command: 'npx',
        args: ['-y', 'mcp-server-example'],
        isActive: false,
        installSource: 'manual'
      })
    )
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('rejects a duplicate name instead of creating', async () => {
    const { onCreate, user } = setup()

    await user.type(screen.getByLabelText(/settings.mcp.name/), 'taken')
    await user.type(screen.getByLabelText(/settings.mcp.command/), 'npx')
    await user.click(submit())

    expect(await screen.findByText('settings.mcp.addServer.importFrom.nameExists')).toBeInTheDocument()
    expect(onCreate).not.toHaveBeenCalled()
  })

  it('rejects a whitespace-only name with inline validation', async () => {
    const { onCreate, user } = setup()

    await user.type(screen.getByLabelText(/settings.mcp.name/), '   ')
    await user.type(screen.getByLabelText(/settings.mcp.command/), 'npx')
    await user.click(submit())

    await waitFor(() => expect(document.querySelector('[data-slot="form-message"]')).not.toBeNull())
    expect(document.querySelector('[data-slot="form-message"]')?.textContent ?? '').toContain('common.name')
    expect(onCreate).not.toHaveBeenCalled()
  })

  it('resets registry options when reopened', async () => {
    const { setOpen, user } = setup()

    await user.type(screen.getByLabelText(/settings.mcp.command/), 'npx')
    await user.click(screen.getByText('settings.mcp.addServer.advanced'))
    expect(screen.getByText('settings.mcp.registry')).toBeInTheDocument()

    setOpen(false)
    setOpen(true)
    await user.click(screen.getByText('settings.mcp.addServer.advanced'))

    expect(screen.getByLabelText(/settings.mcp.command/)).toHaveValue('')
    expect(screen.queryByText('settings.mcp.registry')).not.toBeInTheDocument()
  })

  it('clears the selected registry when switching command families', async () => {
    const { onCreate, user } = setup()

    await user.type(screen.getByLabelText(/settings.mcp.name/), 'python-server')
    const commandInput = screen.getByLabelText(/settings.mcp.command/)
    await user.type(commandInput, 'npx')
    await user.click(screen.getByText('settings.mcp.addServer.advanced'))
    await user.click(screen.getByRole('radio', { name: 'settings.mcp.registryOptions.npmTaobao' }))
    expect(screen.getByRole('radio', { name: 'settings.mcp.registryOptions.npmTaobao' })).toBeChecked()

    await user.clear(commandInput)
    await user.type(commandInput, 'uvx')
    await user.click(submit())

    await waitFor(() => expect(onCreate).toHaveBeenCalledTimes(1))
    expect(onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        command: 'uvx',
        registryUrl: ''
      })
    )
  })
})
