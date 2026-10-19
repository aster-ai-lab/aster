import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { WorkbenchWorkspaceChrome } from '../WorkbenchWorkspaceChrome'

const panel = vi.hoisted(() => ({
  presentationOpen: false,
  presentationMaximized: false,
  tryOpen: vi.fn(() => true),
  canOpen: vi.fn((id: string) => id === 'files' || id === 'browser')
}))

vi.mock('@renderer/components/chat/panes/Shell', () => ({
  useRightPanelState: () => ({
    presentationOpen: panel.presentationOpen,
    presentationMaximized: panel.presentationMaximized
  }),
  useRightPanelActions: () => ({
    canOpen: panel.canOpen,
    tryOpen: panel.tryOpen
  })
}))

describe('WorkbenchWorkspaceChrome', () => {
  beforeEach(() => {
    panel.presentationOpen = false
    panel.presentationMaximized = false
    panel.tryOpen.mockClear()
    panel.canOpen.mockReset()
    panel.canOpen.mockImplementation((id: string) => id === 'files' || id === 'browser')
  })

  it('opens the files pane from the workspace button', () => {
    render(<WorkbenchWorkspaceChrome />)

    const open = screen.getByRole('button', { name: '打开任务工作区' })
    expect(open.querySelector('svg.lucide-panel-right-open')).toBeNull()
    expect(open.querySelector('path[d="M14.5 5v14"]')).not.toBeNull()
    expect(open.className).toContain('size-7')
    expect(open.className).toContain('rounded-[8px]')

    fireEvent.click(open)

    expect(panel.tryOpen).toHaveBeenCalledWith('files', { userInitiated: true })
  })

  it('opens the browser pane when files are not available', () => {
    panel.canOpen.mockImplementation((id: string) => id === 'browser')
    render(<WorkbenchWorkspaceChrome />)

    fireEvent.click(screen.getByRole('button', { name: '打开任务工作区' }))

    expect(panel.tryOpen).toHaveBeenCalledWith('browser', { userInitiated: true })
    expect(screen.getByRole('button', { name: '浏览器' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '终端' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '文件' })).toBeInTheDocument()
  })

  it('opens a pane from the shortcut rail', () => {
    render(<WorkbenchWorkspaceChrome />)

    const files = screen.getByRole('button', { name: '文件' })
    const browser = screen.getByRole('button', { name: '浏览器' })
    const terminal = screen.getByRole('button', { name: '终端' })
    expect(files.querySelector('svg.lucide-folder-open')).not.toBeNull()
    expect(browser.querySelector('svg.lucide-globe')).not.toBeNull()
    expect(terminal.querySelector('svg.lucide-terminal')).not.toBeNull()
    expect(files.className).toContain('size-7')
    expect(
      screen.getByRole('button', { name: '展开工作区快捷菜单' }).querySelector('svg.lucide-chevrons-left')
    ).not.toBeNull()

    fireEvent.click(files)
    fireEvent.click(browser)
    fireEvent.click(terminal)

    expect(panel.tryOpen).toHaveBeenNthCalledWith(1, 'files', { userInitiated: true })
    expect(panel.tryOpen).toHaveBeenNthCalledWith(2, 'browser', { userInitiated: true })
    expect(panel.tryOpen).toHaveBeenNthCalledWith(3, 'terminal', { userInitiated: true })
  })

  it('expands the shortcut menu and collapses it', async () => {
    render(<WorkbenchWorkspaceChrome />)

    fireEvent.click(screen.getByRole('button', { name: '展开工作区快捷菜单' }))

    expect(screen.getByRole('dialog', { name: '工作区快捷菜单' })).toBeInTheDocument()
    expect(screen.getByText('暂无打开的标签')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '文件' }))
    expect(panel.tryOpen).toHaveBeenCalledWith('files', { userInitiated: true })
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: '工作区快捷菜单' })).not.toBeInTheDocument()
    })

    fireEvent.click(screen.getByRole('button', { name: '展开工作区快捷菜单' }))
    fireEvent.click(screen.getByRole('button', { name: '折叠' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: '工作区快捷菜单' })).not.toBeInTheDocument()
    })
    expect(screen.getByRole('navigation', { name: '工作区快捷栏' })).toBeInTheDocument()
  })

  it('hides while the workspace pane is open', () => {
    panel.presentationOpen = true
    const { container } = render(<WorkbenchWorkspaceChrome />)

    expect(container).toBeEmptyDOMElement()
    expect(screen.queryByRole('button', { name: '打开任务工作区' })).not.toBeInTheDocument()
  })
})
