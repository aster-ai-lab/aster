import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { PluginOkfConfig } from '../PluginOkfConfig'

const api = vi.hoisted(() => ({
  listPluginOkfPages: vi.fn(),
  readPluginOkfPage: vi.fn(),
  savePluginOkfPage: vi.fn()
}))
const toastSuccess = vi.hoisted(() => vi.fn())
const toastError = vi.hoisted(() => vi.fn())

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('@data/hooks/usePreference', () => ({
  usePreference: () => ['/Users/roy/work', vi.fn()]
}))

vi.mock('@shared/studioRuntime', () => ({
  readStudioHomePath: () => '/Users/roy'
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { success: toastSuccess, error: toastError }
}))

vi.mock('../pluginOkf', () => api)

vi.mock('@a3s-lab/office/react', async () => {
  const { createElement } = await import('react')
  return {
    MarkdownEditor: (props: {
      content: { markdown: string }
      presentation?: string
      onSave?: (markdown: string) => void | Promise<void>
    }) =>
      createElement(
        'div',
        {
          'data-editor': 'markdown',
          'data-presentation': props.presentation,
          'data-markdown': props.content.markdown
        },
        createElement(
          'button',
          {
            type: 'button',
            onClick: () => props.onSave?.('---\ntitle: 领域\n---\n甲方付款已修订\n')
          },
          '保存'
        )
      )
  }
})

describe('plugin OKF config', () => {
  beforeEach(() => {
    api.listPluginOkfPages.mockReset().mockResolvedValue([{ path: 'okf/domain/index.md', title: '领域' }])
    api.readPluginOkfPage.mockReset().mockResolvedValue({
      path: 'okf/domain/index.md',
      title: '领域',
      markdown: '---\ntitle: 领域\n---\n甲方付款条款\n'
    })
    api.savePluginOkfPage.mockReset().mockResolvedValue(1)
    toastSuccess.mockReset()
    toastError.mockReset()
  })

  it('opens the encyclopedia editor and saves the revised wiki page', async () => {
    const user = userEvent.setup()
    render(<PluginOkfConfig slug="domain-knowledge" name="领域知识" onBack={vi.fn()} />)
    const open = await screen.findByRole('button', { name: /plugins\.okf\.edit/ })
    expect(screen.getByText('okf/domain/index.md')).toBeInTheDocument()

    await user.click(open)
    const dialog = await screen.findByRole('dialog')
    expect(dialog).toHaveAttribute('data-ui', 'plugins.okf.editor')
    const editor = await waitFor(() => dialog.querySelector('[data-editor="markdown"]'))
    expect(editor).toHaveAttribute('data-presentation', 'note')
    expect(editor).toHaveAttribute('data-markdown', '---\ntitle: 领域\n---\n甲方付款条款\n')

    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() =>
      expect(api.savePluginOkfPage).toHaveBeenCalledWith({
        slug: 'domain-knowledge',
        path: 'okf/domain/index.md',
        markdown: '---\ntitle: 领域\n---\n甲方付款已修订\n',
        workspaceWiki: '/Users/roy/work/wiki'
      })
    )
    expect(toastSuccess).toHaveBeenCalledWith('plugins.okf.saved')
  })
})
