import { describe, expect, it } from 'vitest'

import type { DriveFile } from '../driveTransport'
import { cloudDocumentFromFile } from '../useCloudDocuments'

function file(overrides: Partial<DriveFile> = {}): DriveFile {
  return {
    id: 'f-1',
    name: '设计规范.pdf',
    contentType: 'application/pdf',
    sizeBytes: 2048,
    nodeId: null,
    deptId: null,
    createdAt: '2026-10-02T12:00:00Z',
    agentRetrievals: 0,
    parseStatus: 'parsed',
    compileStatus: 'done',
    ...overrides
  }
}

describe('云端文档映射（documents 页 cloud 分区）', () => {
  it('DriveFile → LibraryDocument：格式/图标/种类由文件名推导，归属 cloud/mine', () => {
    const doc = cloudDocumentFromFile(file())
    expect(doc).toMatchObject({
      id: 'f-1',
      name: '设计规范.pdf',
      place: 'cloud',
      folder: 'mine',
      format: 'pdf',
      source: 'cloud_save',
      sizeBytes: 2048
    })
    expect(['doc', 'sheet']).toContain(doc.kind)
  })

  it('不同扩展名推导不同图标与格式', () => {
    const sheet = cloudDocumentFromFile(file({ name: '预算.xlsx', id: 'f-2' }))
    expect(sheet.format).toBe('excel')
    expect(sheet.icon).toBe('excel')
    const code = cloudDocumentFromFile(file({ name: 'config.json', id: 'f-3' }))
    expect(code.icon).toBe('code')
  })

  it('非法时间回退当前时刻（不产生 NaN）', () => {
    const doc = cloudDocumentFromFile(file({ createdAt: 'not-a-date' }))
    expect(Number.isFinite(doc.viewedAt)).toBe(true)
  })
})
