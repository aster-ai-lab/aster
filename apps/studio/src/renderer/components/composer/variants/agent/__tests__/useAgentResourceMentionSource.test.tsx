import { renderHook } from '@testing-library/react'
import type { Editor } from '@tiptap/core'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { AbsoluteFilePath } from '@shared/types/file'

import { parentBrowseDir, parseBrowseQuery, useAgentResourceMentionSource } from '../useAgentResourceMentionSource'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('../../../composerDraft', () => ({
  serializeComposerDocument: () => ({ tokens: [] })
}))

const root = '/work/a3s' as AbsoluteFilePath
const listDirectoryEntries = vi.fn()

describe('useAgentResourceMentionSource', () => {
  beforeEach(() => {
    listDirectoryEntries.mockResolvedValue([
      { path: '/work/a3s/apps', isDirectory: true },
      { path: '/work/a3s/apps/studio/src/main.ts', isDirectory: false }
    ])
    vi.stubGlobal('api', { file: { listDirectoryEntries } })
    Object.assign(window, { api: { file: { listDirectoryEntries } } })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    listDirectoryEntries.mockReset()
  })

  it('shows entry names with their parent folder in the compact label style', async () => {
    const { result } = renderHook(() =>
      useAgentResourceMentionSource({
        accessiblePaths: [root],
        files: [],
        setFiles: vi.fn(),
        enabled: true,
        title: 'Project files',
        labelStyle: 'name'
      })
    )
    const [source] = result.current

    expect(source.title).toBe('Project files')
    const items = await source.items({ query: '', editor: {} as Editor })
    expect(items.map((item) => [item.label, item.description])).toEqual([
      ['apps', undefined],
      ['main.ts', 'apps/studio/src']
    ])
    expect(items[1].filterText).toContain('apps/studio/src/main.ts')
  })

  it('keeps the relative path and absolute path by default', async () => {
    const { result } = renderHook(() =>
      useAgentResourceMentionSource({ accessiblePaths: [root], files: [], setFiles: vi.fn(), enabled: true })
    )
    const items = await result.current[0].items({ query: '', editor: {} as Editor })

    expect(items[1]).toEqual(
      expect.objectContaining({ label: 'apps/studio/src/main.ts', description: '/work/a3s/apps/studio/src/main.ts' })
    )
  })

  it('parses folder browse queries and rejects unsafe segments', () => {
    expect(parseBrowseQuery('apps/studio/')).toEqual({ dir: 'apps/studio', filter: '' })
    expect(parseBrowseQuery('apps/St')).toEqual({ dir: 'apps', filter: 'st' })
    expect(parseBrowseQuery('apps')).toBeNull()
    expect(parseBrowseQuery('/apps')).toBeNull()
    expect(parseBrowseQuery('apps/../etc/')).toBeNull()
    expect(parentBrowseDir('apps/studio')).toBe('apps')
    expect(parentBrowseDir('apps')).toBe('')
  })

  describe('browsing folders', () => {
    const render = () =>
      renderHook(() =>
        useAgentResourceMentionSource({
          accessiblePaths: [root],
          files: [],
          setFiles: vi.fn(),
          enabled: true,
          labelStyle: 'name',
          browseFolders: true
        })
      ).result.current[0]

    it('lists a folder with a breadcrumb and a row to reference the folder', async () => {
      listDirectoryEntries.mockResolvedValue([
        { path: '/work/a3s/apps/studio', isDirectory: true },
        { path: '/work/a3s/apps/README.md', isDirectory: false }
      ])
      const items = await render().items({ query: 'apps/', editor: {} as Editor })

      expect(listDirectoryEntries).toHaveBeenCalledWith(
        '/work/a3s/apps',
        expect.objectContaining({ recursive: false, searchPattern: '.' })
      )
      expect(items.map((item) => item.label)).toEqual([
        'a3s / apps',
        'chat.input.resource_panel.use_folder',
        'studio',
        'README.md'
      ])
      expect(items[0].disabled).toBeUndefined()
      expect(items[0].alwaysVisible).toBe(true)
    })

    it('returns to the parent folder from the breadcrumb and to the root from the first level', async () => {
      vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
        callback(0)
        return 0
      })
      listDirectoryEntries.mockResolvedValue([])
      const run = vi.fn()
      const insertContent = vi.fn(() => ({ run }))
      const editor = { chain: () => ({ focus: () => ({ insertContent }) }) } as unknown as Editor

      const nested = await render().items({ query: 'apps/studio/', editor: {} as Editor })
      nested[0].command({ editor, range: { from: 0, to: 0 }, item: nested[0], query: 'apps/studio/' })
      expect(insertContent).toHaveBeenCalledWith('@apps/')

      const top = await render().items({ query: 'apps/', editor: {} as Editor })
      top[0].command({ editor, range: { from: 0, to: 0 }, item: top[0], query: 'apps/' })
      expect(insertContent).toHaveBeenLastCalledWith('@')
      expect(run).toHaveBeenCalledTimes(2)
    })

    it('uses Backspace to leave a folder and leaves a typed filter to the editor', async () => {
      const source = render()
      const insertText = vi.fn(function (this: { marker: string }) {
        return this
      })
      const transaction = { insertText, marker: 'tr' }
      const dispatch = vi.fn()
      const press = (text: string) =>
        source.onKeyDown?.({
          event: new KeyboardEvent('keydown', { key: 'Backspace' }),
          range: { from: 1, to: 1 + text.length },
          view: {
            dispatch,
            state: { doc: { textBetween: () => text }, tr: transaction }
          }
        } as never)

      expect(press('@apps/studio/')).toBe(true)
      expect(insertText).toHaveBeenCalledWith('@apps/', 1, '@apps/studio/'.length + 1)
      expect(dispatch).toHaveBeenCalledWith(transaction)

      insertText.mockClear()
      expect(press('@apps/st')).toBe(false)
      expect(insertText).not.toHaveBeenCalled()
      expect(press('@')).toBe(false)
    })

    it('filters a folder by the text after the last slash', async () => {
      listDirectoryEntries.mockResolvedValue([
        { path: '/work/a3s/apps/studio', isDirectory: true },
        { path: '/work/a3s/apps/web', isDirectory: true }
      ])
      const items = await render().items({ query: 'apps/st', editor: {} as Editor })

      expect(items.map((item) => item.label)).toEqual(['a3s / apps', 'studio'])
    })

    it('falls back to path search when the text is not a folder', async () => {
      listDirectoryEntries
        .mockRejectedValueOnce(new Error('404'))
        .mockResolvedValueOnce([{ path: '/work/a3s/src/composer/index.ts', isDirectory: false }])
      const items = await render().items({ query: 'composer/ind', editor: {} as Editor })

      expect(listDirectoryEntries).toHaveBeenLastCalledWith(
        root,
        expect.objectContaining({ recursive: true, searchPattern: 'composer/ind' })
      )
      expect(items.map((item) => item.label)).toEqual(['index.ts'])
    })

    it('opens a folder instead of inserting it', async () => {
      vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
        callback(0)
        return 0
      })
      listDirectoryEntries.mockResolvedValue([{ path: '/work/a3s/apps', isDirectory: true }])
      const [folder] = await render().items({ query: '', editor: {} as Editor })
      const run = vi.fn()
      const insertContent = vi.fn(() => ({ run }))
      const editor = { chain: () => ({ focus: () => ({ insertContent }) }) } as unknown as Editor

      folder.command({ editor, range: { from: 0, to: 0 }, item: folder, query: '' })

      expect(insertContent).toHaveBeenCalledWith('@apps/')
      expect(run).toHaveBeenCalled()
    })
  })
})
