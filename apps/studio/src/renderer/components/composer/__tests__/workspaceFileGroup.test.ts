import { describe, expect, it } from 'vitest'

import {
  buildWorkspaceFileTree,
  createWorkspaceFileGroupToken,
  readWorkspaceFileGroup,
  workspaceFileGroupLabel,
  workspaceGroupIcon
} from '../workspaceFileGroup'

describe('workspace file group', () => {
  const choices = [
    { path: '/work/docs', isDirectory: true },
    { path: '/work/src/a.ts', isDirectory: false },
    { path: '/work/src/a.ts', isDirectory: false }
  ]

  it('stores one chip whose prompt lists every selected path', () => {
    const token = createWorkspaceFileGroupToken(choices, '2 个文件')
    expect(token.kind).toBe('folder')
    expect(token.label).toBe('2 个文件')
    expect(token.promptText).toBe('/work/docs/\n/work/src/a.ts')
    expect(readWorkspaceFileGroup(token)).toEqual([
      { path: '/work/docs', isDirectory: true },
      { path: '/work/src/a.ts', isDirectory: false }
    ])
  })

  it('rebuilds the selected tree from the prompt when the payload is gone', () => {
    const token = createWorkspaceFileGroupToken(choices, '2 个文件')
    const entries = readWorkspaceFileGroup({ promptText: token.promptText })
    expect(buildWorkspaceFileTree(entries ?? [])).toEqual([
      {
        name: 'docs',
        path: '/work/docs',
        isDirectory: true,
        children: []
      },
      {
        name: 'src',
        path: '/work/src',
        isDirectory: true,
        children: [
          {
            name: 'a.ts',
            path: '/work/src/a.ts',
            isDirectory: false,
            children: []
          }
        ]
      }
    ])
  })

  it('uses the workspace file icon for a selected file and the folder icon for a directory', () => {
    expect(workspaceGroupIcon([{ path: '/work/报告.docx', isDirectory: false }])).toBe('word')
    expect(workspaceGroupIcon([{ path: '/work/docs', isDirectory: true }])).toBe('directory')
    expect(
      workspaceGroupIcon([
        { path: '/work/a.md', isDirectory: false },
        { path: '/work/b.md', isDirectory: false }
      ])
    ).toBe('markdown')
    expect(
      workspaceGroupIcon([
        { path: '/work/a.md', isDirectory: false },
        { path: '/work/b.docx', isDirectory: false }
      ])
    ).toBe('file')
  })

  it('names a single file after itself and a batch after the count', () => {
    expect(workspaceFileGroupLabel([{ path: '/work/notes.md', isDirectory: false }], (count) => `${count} files`)).toBe(
      'notes.md'
    )
    expect(workspaceFileGroupLabel(choices, (count) => `${count} files`)).toBe('2 files')
  })
})
