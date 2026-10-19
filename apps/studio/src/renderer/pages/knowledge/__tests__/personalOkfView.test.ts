import { describe, expect, it } from 'vitest'

import type { VaultTreeNode } from '../knowledgeVault'
import {
  conceptDiagnostics,
  conceptDocument,
  conceptOutlinks,
  conceptPassageHits,
  conceptProse,
  isPersonalVaultDirectory,
  isReservedNoteName,
  isVaultArtifact,
  noteEditorBody,
  noteTypeLine,
  compileRunProgress,
  parseCompileReport,
  parseIndexCatalog,
  resolveBundleReference,
  treeMatchingPaths,
  visibleNoteTree,
  withNoteType
} from '../personalOkfView'

const INDEX = `---
okf_version: "0.2"
---

- [甲](/concepts/甲.md)
- [乙](/concepts/目录__乙.md)

## Skipped

- \`空.md\` — empty

## Failed

- \`坏.md\` — frontmatter
`

const CONCEPT = `---
type: Document
title: "甲"
resource: "../甲.md"
sources:
  - id: source
    resource: "../甲.md"
    title: "甲"
---

见轮渡

[乙](../乙.md)
[缺失](缺失.md)

## Sources

- [甲](../甲.md)
`

function file(name: string, path: string): VaultTreeNode {
  return { name, path, kind: 'file', children: [] }
}

function folder(name: string, path: string, children: VaultTreeNode[]): VaultTreeNode {
  return { name, path, kind: 'directory', children }
}

describe('personal OKF view', () => {
  it('parses a compile report and rejects a partial line', () => {
    expect(parseCompileReport('compiled=1 unchanged=2 skipped=3 failed=4')).toEqual({
      compiled: 1,
      unchanged: 2,
      skipped: 3,
      failed: 4
    })
    expect(parseCompileReport('compiled=1')).toBeNull()
  })

  it('counts concept files written in this run and ignores a newer index', () => {
    const startedAt = 1_000
    expect(
      compileRunProgress({
        noteTotal: 4,
        startedAt,
        conceptModifiedMs: [1, startedAt]
      })
    ).toEqual({ done: 1, total: 4 })
    expect(
      compileRunProgress({
        noteTotal: 4,
        startedAt,
        conceptModifiedMs: [1, 1]
      })
    ).toEqual({ done: 0, total: 4 })
    expect(
      compileRunProgress({
        noteTotal: 2,
        startedAt,
        conceptModifiedMs: [startedAt, startedAt, startedAt]
      })
    ).toEqual({ done: 2, total: 2 })
    expect(
      compileRunProgress({
        noteTotal: 100,
        startedAt,
        conceptModifiedMs: [startedAt],
        pendingTotal: 2,
        pendingModifiedMs: [1, startedAt]
      })
    ).toEqual({ done: 1, total: 2 })
  })

  it('reads concept links separately from skipped and failed sources', () => {
    expect(parseIndexCatalog(INDEX)).toEqual({
      concepts: [
        { title: '甲', file: 'concepts/甲.md' },
        { title: '乙', file: 'concepts/目录__乙.md' }
      ],
      skipped: [{ path: '空.md', reason: 'empty' }],
      failed: [{ path: '坏.md', reason: 'frontmatter' }]
    })
  })

  it('resolves a concept resource onto the note and drops schemes and escapes', () => {
    expect(resolveBundleReference('concepts/甲.md', '../甲.md')).toBe('甲.md')
    expect(resolveBundleReference('concepts/目录__乙.md', '../目录/乙.md')).toBe('目录/乙.md')
    expect(resolveBundleReference('concepts/甲.md', '/甲.md')).toBe('甲.md')
    expect(resolveBundleReference('concepts/甲.md', '../甲.md#passage-0')).toBe('甲.md')
    expect(resolveBundleReference('concepts/甲.md', '../../甲.md')).toBeNull()
    expect(resolveBundleReference('concepts/甲.md', 'javascript:alert(1)')).toBeNull()
    expect(resolveBundleReference('concepts/甲.md', 'https://example.test/甲.md')).toBeNull()
  })

  it('hides the concepts directory and reserved names, then filters skipped sources', () => {
    const tree = [
      file('甲.md', '/wiki/local/甲.md'),
      file('index.md', '/wiki/local/index.md'),
      file('log.md', '/wiki/local/log.md'),
      folder('concepts', '/wiki/local/concepts', [file('甲.md', '/wiki/local/concepts/甲.md')]),
      folder('目录', '/wiki/local/目录', [
        file('乙.md', '/wiki/local/目录/乙.md'),
        file('index.md', '/wiki/local/目录/index.md')
      ])
    ]
    const visible = visibleNoteTree(tree)
    expect(visible.map((node) => node.name)).toEqual(['甲.md', '目录'])
    expect(visible[1]?.children.map((node) => node.name)).toEqual(['乙.md'])
    expect(treeMatchingPaths(visible, '/wiki/local', new Set(['目录/乙.md'])).map((node) => node.name)).toEqual([
      '目录'
    ])
    expect(isReservedNoteName('index.md')).toBe(true)
    expect(isReservedNoteName('Index.md')).toBe(false)
    expect(isVaultArtifact('/wiki/local', '/wiki/local/concepts/甲.md')).toBe(true)
    expect(isVaultArtifact('/wiki/local', '/wiki/local/甲.md')).toBe(false)
    expect(isPersonalVaultDirectory('/workspace/wiki/local')).toBe(true)
    expect(isPersonalVaultDirectory('/workspace/wiki/local/nested')).toBe(false)
    expect(isPersonalVaultDirectory('/workspace/Wiki/local')).toBe(false)
  })

  it('keeps a type line out of the editor and puts it back on save', () => {
    const stamped = '---\ntype: Note\ntitle: "甲"\n---\n见乙\n'
    expect(noteEditorBody(stamped)).toBe('见乙\n')
    expect(noteTypeLine(stamped)).toBe('type: Note')
    expect(noteEditorBody('---\ntitle: "甲"\n---\n见乙\n')).toBe('见乙\n')
    expect(noteEditorBody('---\ntitle: "甲"\nstatus: draft\n---\n见乙\n')).toBe(
      '---\ntitle: "甲"\nstatus: draft\n---\n见乙\n'
    )
    expect(withNoteType('见乙\n', 'type: Note')).toBe('---\ntype: Note\n---\n见乙\n')
    expect(withNoteType('---\ntitle: "家常食谱"\n---\n见乙\n', 'type: Note')).toBe(
      '---\ntype: Note\ntitle: "家常食谱"\n---\n见乙\n'
    )
    expect(withNoteType('见乙\n', null)).toBe('见乙\n')
  })

  it('lists sources, file outlinks, and a dangling link without treating a web URL as one', () => {
    const files = new Set(['甲.md', '乙.md', 'concepts/甲.md'])
    expect(conceptDocument(CONCEPT)).toEqual({
      type: 'Document',
      title: '甲',
      sources: [{ title: '甲', resource: '../甲.md' }]
    })
    expect(conceptProse(CONCEPT)).toContain('见轮渡')
    expect(conceptProse(CONCEPT)).not.toContain('## Sources')
    expect(conceptOutlinks(CONCEPT, 'concepts/甲.md', files)).toEqual([{ label: '乙', href: '乙.md' }])
    expect(conceptDiagnostics(CONCEPT, 'concepts/甲.md', files)).toEqual(['缺失'])
    expect(conceptDiagnostics('见 [站](https://example.test) 和 [甲](../甲.md)\n', 'concepts/甲.md', files)).toEqual([])
    expect(conceptPassageHits([{ name: '甲.md', title: '甲', markdown: CONCEPT }], '轮渡')).toHaveLength(1)
    expect(conceptPassageHits([{ name: '甲.md', title: '甲', markdown: CONCEPT }], '甲')).toEqual([])
  })
})
