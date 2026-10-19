import { describe, expect, it } from 'vitest'

import {
  blankDocument,
  DOCUMENT_TREE_FOLDERS,
  documentOwners,
  documentsInTreeFolder,
  formatDocumentActivity,
  formatDocumentSize,
  iconForName,
  isSameCalendarDay,
  listDocuments,
  type LibraryDocument,
  SEED_DOCUMENTS,
  storageUsage,
  STORAGE_QUOTA_BYTES,
  uploadedDocument
} from '../documentLibrary'

const recent = { section: 'recent' as const, folder: null, view: 'recent' as const }
const openQuery = { query: '', kind: 'all' as const, owner: null, sort: 'desc' as const }

const byId = Object.fromEntries(SEED_DOCUMENTS.map((document) => [document.id, document]))

describe('document library', () => {
  it('lists recent documents with the newest view first', () => {
    const names = listDocuments(SEED_DOCUMENTS, recent, openQuery).map((document) => document.id)
    expect(names.slice(0, 3)).toEqual(['privacy', 'consignment', 'business-plan'])
    expect(names.at(-1)).toBe('ops')
    expect(isSameCalendarDay(byId.privacy.viewedAt, Date.now())).toBe(true)
    expect(isSameCalendarDay(byId.checkup.viewedAt, Date.now())).toBe(false)
  })

  it('filters favorites, kind, owner, search, and a folder', () => {
    expect(
      listDocuments(SEED_DOCUMENTS, { ...recent, view: 'favorites' }, openQuery).map((document) => document.id)
    ).toEqual(['privacy', 'checkup'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, kind: 'sheet' }).map((document) => document.id)
    ).toEqual(['users', 'ops'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, owner: '林志霄' }).map((document) => document.id)
    ).toEqual(['resume', 'resume-copy', 'resume-flip'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, query: 'readme' }).map((document) => document.id)
    ).toEqual(['readme'])
    expect(
      listDocuments(SEED_DOCUMENTS, { section: 'cloud', folder: 'mine', view: 'recent' }, openQuery).map(
        (document) => document.id
      )
    ).toContain('privacy')
    expect(
      listDocuments(SEED_DOCUMENTS, { section: 'cloud', folder: 'shared', view: 'recent' }, openQuery).map(
        (document) => document.id
      )
    ).toEqual(['resume', 'resume-copy', 'resume-flip'])
    expect(listDocuments(SEED_DOCUMENTS, { section: 'cloud', folder: 'docker', view: 'recent' }, openQuery)).toEqual([])
    expect(
      listDocuments(SEED_DOCUMENTS, { section: 'local', folder: 'downloads', view: 'recent' }, openQuery).map(
        (document) => document.id
      )
    ).toEqual(['biome', 'readme'])
    expect(listDocuments(SEED_DOCUMENTS, { section: 'local', folder: 'desktop', view: 'recent' }, openQuery)).toEqual(
      []
    )
    expect(
      listDocuments(SEED_DOCUMENTS, { ...recent, origin: 'local' }, openQuery).map((document) => document.id)
    ).toEqual(['biome', 'readme'])
    expect(
      listDocuments(SEED_DOCUMENTS, { ...recent, origin: 'cloud' }, openQuery).map((document) => document.id)
    ).not.toContain('biome')
    expect(
      listDocuments(SEED_DOCUMENTS, { ...recent, origin: 'cloud' }, openQuery).map((document) => document.id)
    ).toContain('privacy')
    expect(
      listDocuments(SEED_DOCUMENTS, { ...recent, origin: 'cloud' }, openQuery).map((document) => document.id)
    ).not.toContain('chain-plan')
  })

  it('filters format, source, and device', () => {
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, formats: ['pdf'] }).map((document) => document.id)
    ).toEqual(['resume', 'checkup', 'resume-copy', 'resume-flip', 'chain-plan'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, sources: ['quark'] }).map((document) => document.id)
    ).toEqual(['resume'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, device: 'phone' }).map((document) => document.id)
    ).toEqual(['checkup'])
    expect(
      listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, formats: [], sources: [] }).map((document) => document.id)
    ).toEqual(listDocuments(SEED_DOCUMENTS, recent, openQuery).map((document) => document.id))
    expect(blankDocument('doc', '我', '未命名文档', Date.parse('2026-10-02T10:00:00'), 'new-doc')).toMatchObject({
      format: 'word',
      source: 'manual',
      device: 'computer'
    })
    expect(uploadedDocument('笔记.pdf', 2048, '我', Date.parse('2026-10-02T10:00:00'), 'upload-doc')).toMatchObject({
      format: 'pdf',
      source: 'manual',
      device: 'computer'
    })
  })

  it('keeps trashed documents out of recent and sorts the oldest view first', () => {
    const trashed: LibraryDocument = { ...SEED_DOCUMENTS[0], trashed: true }
    const documents = [trashed, ...SEED_DOCUMENTS.slice(1)]
    expect(listDocuments(documents, recent, openQuery).some((document) => document.id === 'privacy')).toBe(false)
    expect(
      listDocuments(documents, { section: 'trash', folder: null, view: 'recent' }, openQuery).map(
        (document) => document.id
      )
    ).toEqual(['privacy'])
    expect(listDocuments(SEED_DOCUMENTS, recent, { ...openQuery, sort: 'asc' })[0]?.id).toBe('ops')
    expect(documentOwners(listDocuments(SEED_DOCUMENTS, recent, openQuery))).toContain('林志霄')
  })

  it('creates blank and uploaded documents and formats size, activity, and storage', () => {
    const now = Date.parse('2026-10-02T10:00:00')
    const created = blankDocument('sheet', '我', '未命名表格', now, 'new-sheet', '我的文档')
    expect(created).toMatchObject({
      place: 'cloud',
      folder: 'mine',
      kind: 'sheet',
      icon: 'excel',
      activity: 'view',
      locationPath: '我的文档',
      owner: '我',
      viewedAt: now
    })
    expect(blankDocument('doc', '我', '未命名文档', now, 'new-doc').icon).toBe('word')
    expect(uploadedDocument('预算.xlsx', 4096, '我', now, 'upload-sheet', '下载')).toMatchObject({
      kind: 'sheet',
      icon: 'excel',
      activity: 'upload',
      place: 'local',
      folder: 'downloads',
      locationPath: '下载'
    })
    expect(uploadedDocument('笔记.pdf', 2048, '我', now, 'upload-doc')).toMatchObject({
      kind: 'doc',
      icon: 'pdf',
      activity: 'upload'
    })
    expect(iconForName('biome.json')).toBe('json')
    expect(iconForName('README.md')).toBe('markdown')
    expect(iconForName('隐私政策.docx')).toBe('word')

    expect(formatDocumentSize(byId.privacy.sizeBytes)).toBe('17.1KB')
    expect(formatDocumentSize(byId.consignment.sizeBytes)).toBe('13.8KB')
    expect(formatDocumentSize(byId.checkup.sizeBytes)).toBe('32.6MB')
    expect(formatDocumentSize(byId.biome.sizeBytes)).toBe('477B')
    expect(formatDocumentSize(byId.readme.sizeBytes)).toBe('262B')
    expect(formatDocumentSize(byId.platform.sizeBytes)).toBe('2.5KB')
    expect(formatDocumentSize(byId.users.sizeBytes)).toBe('6.4MB')
    expect(formatDocumentSize(STORAGE_QUOTA_BYTES)).toBe('10GB')
    expect(formatDocumentActivity(Date.parse('2026-07-05T14:58:00'))).toBe('07/05 14:58')
    expect(formatDocumentActivity(Date.parse('2026-03-02T21:21:00'))).toBe('03/02 21:21')

    const usage = storageUsage(SEED_DOCUMENTS)
    expect(usage.quotaBytes).toBe(STORAGE_QUOTA_BYTES)
    expect(usage.usedBytes).toBe(usage.docBytes + usage.sheetBytes)
    expect(usage.docBytes).toBeGreaterThan(usage.sheetBytes)
    expect(listDocuments(SEED_DOCUMENTS, { section: 'storage', folder: null, view: 'recent' }, openQuery)).toEqual([])
  })

  it('lists the document tree for the active place', () => {
    expect(DOCUMENT_TREE_FOLDERS.cloud).toEqual(['mine', 'shared'])
    expect(DOCUMENT_TREE_FOLDERS.local).toEqual([])
    expect(documentsInTreeFolder(SEED_DOCUMENTS, 'local', 'downloads').map((document) => document.name)).toEqual([
      'biome.json',
      'README.md'
    ])
    expect(documentsInTreeFolder(SEED_DOCUMENTS, 'local', 'desktop')).toEqual([])
    expect(documentsInTreeFolder(SEED_DOCUMENTS, 'cloud', 'docker')).toEqual([])
    expect(
      documentsInTreeFolder(SEED_DOCUMENTS, 'cloud', 'shared')
        .map((document) => document.id)
        .sort()
    ).toEqual(['resume', 'resume-copy', 'resume-flip'])
    expect(documentsInTreeFolder(SEED_DOCUMENTS, 'local', 'mine')).toEqual([])
    const trashed = { ...SEED_DOCUMENTS[0], trashed: true, place: 'local' as const, folder: 'downloads' as const }
    expect(documentsInTreeFolder([trashed], 'local', 'downloads')).toEqual([])
  })
})
