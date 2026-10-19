import { describe, expect, it } from 'vitest'

import { officeCreateFileName, officeCreateIssue, uniqueOfficeStem } from '../officeCreateName'

describe('officeCreateName', () => {
  it('keeps a typed stem and drops one pasted extension', () => {
    expect(officeCreateFileName('  季度计划.docx  ', 'docx')).toBe('季度计划.docx')
    expect(officeCreateFileName('季度计划.DOCX', 'docx')).toBe('季度计划.docx')
    expect(officeCreateIssue('季度计划', 'docx', [])).toBeNull()
  })

  it('rejects an empty name, a path character, a trailing dot, and a reserved device name', () => {
    expect(officeCreateIssue('   ', 'docx', [])).toBe('empty')
    expect(officeCreateIssue('季度/计划', 'docx', [])).toBe('invalid')
    expect(officeCreateIssue('计划.', 'docx', [])).toBe('invalid')
    expect(officeCreateIssue('CON', 'docx', [])).toBe('reserved')
    expect(officeCreateIssue('com1', 'xlsx', [])).toBe('reserved')
  })

  it('rejects a name longer than 255 characters and a name the folder already has', () => {
    expect(officeCreateIssue(`${'文'.repeat(251)}`, 'docx', [])).toBe('too_long')
    expect(officeCreateIssue(`${'文'.repeat(250)}`, 'docx', [])).toBeNull()
    expect(officeCreateIssue('计划', 'docx', ['计划.DOCX'])).toBe('taken')
    expect(officeCreateIssue('计划', 'md', ['计划.docx'])).toBeNull()
  })

  it('suggests the next free default without renaming a name the user typed', () => {
    expect(uniqueOfficeStem(['未命名文档.docx'], '未命名文档', 'docx')).toBe('未命名文档 2')
    expect(uniqueOfficeStem(['未命名文档.docx', '未命名文档 2.docx'], '未命名文档', 'docx')).toBe('未命名文档 3')
    expect(uniqueOfficeStem([], '未命名文档', 'docx')).toBe('未命名文档')
  })
})
