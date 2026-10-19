import { describe, expect, it } from 'vitest'

import { createComposerFolderToken } from '@renderer/components/composer/folderToken'
import type { ComposerDraftToken } from '@renderer/components/composer/tokens'
import { agentFileToComposerToken } from '@renderer/components/composer/variants/agentComposerTokens'
import { FILE_TYPE } from '@renderer/types/file'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { AbsoluteFilePathSchema } from '@shared/types/file'

import { applyWorkspaceFileSelection, selectedWorkspacePaths } from '../WorkbenchWorkspaceFiles'

const historical: ComposerAttachment = {
  fileTokenSourceId: 'historical-attachment',
  name: 'saved.txt',
  origin_name: 'saved.txt',
  ext: '.txt',
  size: 10,
  type: FILE_TYPE.TEXT
}

const local: ComposerAttachment = {
  ...historical,
  fileTokenSourceId: 'local-attachment',
  path: AbsoluteFilePathSchema.parse('/work/old.txt')
}

describe('workspace file selection with historical attachments', () => {
  it('lists filesystem paths without treating a pathless attachment as a local file', () => {
    expect(
      selectedWorkspacePaths([historical, local], [{ kind: 'folder', promptText: '/work/old.txt\n/work/notes/' }])
    ).toEqual(['/work/old.txt', '/work/notes/'])
  })

  it('preserves historical attachments when replacing and clearing a local selection', () => {
    let files = [historical, local]
    let tokens: ComposerDraftToken[] = [
      agentFileToComposerToken(historical),
      agentFileToComposerToken(local),
      createComposerFolderToken('/work/old-folder')
    ]
    const update = (next: Parameters<typeof applyWorkspaceFileSelection>[0]['next']) => {
      applyWorkspaceFileSelection({
        next,
        files,
        tokens,
        insertToken: (token) => {
          tokens = [...tokens, token]
        },
        removeToken: (id) => {
          tokens = tokens.filter((token) => token.id !== id)
        },
        setFiles: (value) => {
          files = typeof value === 'function' ? value(files) : value
        }
      })
    }

    update([
      { path: '/work/new.txt', isDirectory: false },
      { path: '/work/new.txt', isDirectory: false },
      { path: '/work/new-folder', isDirectory: true }
    ])
    expect(files).toHaveLength(2)
    expect(files[0]).toBe(historical)
    expect(selectedWorkspacePaths(files, tokens)).toEqual(['/work/new.txt', '/work/new-folder'])
    expect(tokens).toHaveLength(3)
    expect(tokens).toContainEqual(agentFileToComposerToken(historical))

    update([])
    expect(files).toEqual([historical])
    expect(tokens).toEqual([agentFileToComposerToken(historical)])
  })
})
