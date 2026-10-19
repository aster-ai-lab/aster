import { workspaceFileImportFixture } from '@test-mocks/renderer/workspaceFileImport'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createInstance } from 'i18next'
import { I18nextProvider } from 'react-i18next'
import { afterEach, describe, expect, it, vi } from 'vitest'

import zhCn from '@renderer/i18n/locales/zh-cn.json'

import { WorkbenchSessionFiles } from '../WorkbenchSessionFiles'

const root = '/tmp/export-workspace'
const source = `${root}/result.pdf`
const bytes = new Uint8Array([37, 80, 68, 70, 0, 255])
const bridgeApi = window.api

describe('workspace file save as', () => {
  afterEach(() => vi.unstubAllGlobals())

  it.each(['saved', 'cancelled', 'read-error', 'write-error'] as const)(
    'keeps source bytes and reports the %s outcome',
    async (outcome) => {
      const i18n = createInstance()
      await i18n.init({ lng: 'zh-CN', keySeparator: false, resources: { 'zh-CN': { translation: zhCn } } })
      const fixture = workspaceFileImportFixture(root, [])
      fixture.stored.set(source, bytes.slice())
      const save = vi.fn(async () => {
        if (outcome === 'write-error') throw new Error('destination refused')
        return outcome === 'cancelled' ? null : '/tmp/exported/result.pdf'
      })
      vi.stubGlobal('api', { ...bridgeApi, file: { ...bridgeApi.file, save } })
      vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:4318', token: 'ab'.repeat(32) })
      vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = new URL(String(input))
        if (url.pathname === '/api/workspace-file/raw')
          return outcome === 'read-error' ? new Response(null, { status: 403 }) : new Response(bytes.slice())
        return fixture.fetch(input, init)
      })
      render(
        <I18nextProvider i18n={i18n}>
          <WorkbenchSessionFiles root={root} />
        </I18nextProvider>
      )
      const user = userEvent.setup()
      // Recent and tree rows share names; the maintained tree marker identifies the file menu owner.
      const file = await waitFor(() => {
        const row = screen
          .getAllByRole('button', { name: 'result.pdf' })
          .find((element) => element.hasAttribute('data-workspace-session-file'))
        if (!row) throw new Error('File tree is not ready')
        return row
      })
      await user.click(file)
      await user.keyboard('{Shift>}{F10}{/Shift}')
      await user.click(await screen.findByRole('menuitem', { name: '另存为' }))
      if (outcome.endsWith('error')) {
        expect(await screen.findByRole('alert')).toHaveTextContent('保存失败')
      } else if (outcome === 'saved') {
        expect(await screen.findByRole('alert')).toHaveTextContent('已保存')
      } else {
        await waitFor(() => expect(save).toHaveBeenCalledOnce())
        expect(screen.queryByRole('alert')).toBeNull()
      }
      if (outcome === 'read-error') expect(save).not.toHaveBeenCalled()
      else expect(save).toHaveBeenCalledWith('result.pdf', bytes)
      expect(fixture.stored.get(source)).toEqual(bytes)
    }
  )
})
