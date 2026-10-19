import { describe, expect, it } from 'vitest'

import { documentFileIconType, workspaceFileIconType } from '../DocumentFileIcon'

describe('workspaceFileIconType', () => {
  it('maps document formats to the documents page icon family', () => {
    expect(workspaceFileIconType('报告.docx')).toBe('word')
    expect(workspaceFileIconType('合同.PDF')).toBe('pdf')
    expect(workspaceFileIconType('README.md')).toBe('markdown')
    expect(workspaceFileIconType('预算.xlsx')).toBe('excel')
    expect(workspaceFileIconType('发布会.pptx')).toBe('slides')
    expect(documentFileIconType('封面.png')).toBe('image')
    expect(workspaceFileIconType('语音.mp3')).toBe('audio')
    expect(workspaceFileIconType('剪辑.mp4')).toBe('video')
    expect(workspaceFileIconType('备份.zip')).toBe('archive')
  })

  it('gives source, data, and config files their own marks', () => {
    expect(workspaceFileIconType('main.rs')).toBe('code')
    expect(workspaceFileIconType('package.json')).toBe('json')
    expect(workspaceFileIconType('Dockerfile')).toBe('code')
    expect(workspaceFileIconType('001_init.sql')).toBe('sql')
    expect(workspaceFileIconType('Cargo.toml')).toBe('config')
    expect(workspaceFileIconType('Cargo.lock')).toBe('config')
    expect(workspaceFileIconType('notes.txt')).toBe('text')
    expect(workspaceFileIconType('blob.bin')).toBe('file')
  })
})
