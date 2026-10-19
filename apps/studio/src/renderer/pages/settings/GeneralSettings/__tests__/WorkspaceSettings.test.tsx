import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'

import { WorkspaceSettings } from '../WorkspaceSettings'

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('@renderer/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' })
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { error: vi.fn() }
}))

const request = vi.fn()

vi.mock('@renderer/ipc', () => ({
  ipcApi: {
    request: (...args: unknown[]) => request(...args)
  }
}))

function selectFolderMock() {
  return window.api.file.selectFolder as ReturnType<typeof vi.fn>
}

async function enableChange() {
  await waitFor(() => expect(screen.getByRole('button', { name: 'settings.workspace.change' })).toBeEnabled())
}

describe('WorkspaceSettings', () => {
  beforeEach(() => {
    MockUsePreferenceUtils.resetMocks()
    request.mockReset()
    request.mockResolvedValue({ homePath: '/Users/roylin' })
    window.api.file.selectFolder = vi.fn().mockResolvedValue(null)
    vi.mocked(toast.error).mockClear()
  })

  it('shows the .a3s directory in the home folder', async () => {
    render(<WorkspaceSettings />)

    expect(await screen.findByText('/Users/roylin/.a3s')).toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(screen.getByText('settings.workspace.title')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.workspace.reset' })).not.toBeInTheDocument()
  })

  it('stores a chosen folder', async () => {
    selectFolderMock().mockResolvedValue('/Users/roylin/Work')
    const view = render(<WorkspaceSettings />)
    await screen.findByText('/Users/roylin/.a3s')
    await enableChange()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.change' }))

    await waitFor(() => {
      expect(MockUsePreferenceUtils.getPreferenceValue('app.workspace.path')).toBe('/Users/roylin/Work')
    })
    expect(selectFolderMock()).toHaveBeenCalledWith({
      title: 'settings.workspace.folder',
      defaultPath: '/Users/roylin/.a3s',
      properties: ['openDirectory', 'createDirectory']
    })
    view.rerender(<WorkspaceSettings />)
    expect(screen.getByText('/Users/roylin/Work')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'settings.workspace.reset' })).toBeInTheDocument()
  })

  it('stores an empty value when the chosen folder is the default', async () => {
    selectFolderMock().mockResolvedValue('/Users/roylin/.a3s/')
    render(<WorkspaceSettings />)
    await enableChange()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.change' }))

    await waitFor(() => {
      expect(MockUsePreferenceUtils.getPreferenceValue('app.workspace.path')).toBe('')
    })
  })

  it('keeps the stored folder when the dialog is cancelled', async () => {
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/keep')
    selectFolderMock().mockResolvedValue(null)
    render(<WorkspaceSettings />)
    expect(await screen.findByText('/tmp/keep')).toBeInTheDocument()
    await enableChange()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.change' }))

    await waitFor(() => expect(selectFolderMock()).toHaveBeenCalled())
    expect(MockUsePreferenceUtils.getPreferenceValue('app.workspace.path')).toBe('/tmp/keep')
  })

  it('rejects a filesystem root', async () => {
    selectFolderMock().mockResolvedValue('/')
    render(<WorkspaceSettings />)
    await enableChange()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.change' }))

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('settings.workspace.invalid')
    })
    expect(MockUsePreferenceUtils.getPreferenceValue('app.workspace.path')).toBe('')
  })

  it('reports a failed folder dialog', async () => {
    selectFolderMock().mockRejectedValue(new Error('no dialog'))
    render(<WorkspaceSettings />)
    await enableChange()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.change' }))

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('settings.workspace.select_failed')
    })
  })

  it('restores the default folder', async () => {
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/custom')
    const view = render(<WorkspaceSettings />)
    expect(await screen.findByText('/tmp/custom')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'settings.workspace.reset' }))

    await waitFor(() => {
      expect(MockUsePreferenceUtils.getPreferenceValue('app.workspace.path')).toBe('')
    })
    view.rerender(<WorkspaceSettings />)
    expect(screen.getByText('/Users/roylin/.a3s')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.workspace.reset' })).not.toBeInTheDocument()
  })
})
