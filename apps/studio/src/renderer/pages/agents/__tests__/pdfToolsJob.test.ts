import { describe, expect, it } from 'vitest'

import {
  PDF_TOOL_BYTE_LIMIT,
  PDF_TOOL_FILE_LIMIT,
  engineForKind,
  failureKey,
  fileKindOf,
  gapKey,
  intake,
  longImageLayout,
  type ToolFile
} from '../pdfToolsJob'

function file(name: string, size = 10, type = ''): ToolFile {
  return { name, size, type }
}

describe('pdf tool jobs', () => {
  it('classifies a file by its extension before its mime type', () => {
    expect(fileKindOf(file('scan.PDF', 1, 'application/octet-stream'))).toBe('pdf')
    expect(fileKindOf(file('notes.docx'))).toBe('word')
    expect(fileKindOf(file('sheet.xlsx'))).toBe('excel')
    expect(fileKindOf(file('page.png'))).toBe('image')
    expect(fileKindOf(file('memo.m4a'))).toBe('audio')
    expect(fileKindOf(file('blob', 1, 'application/pdf'))).toBe('pdf')
    expect(fileKindOf(file('blob.bin'))).toBe('unknown')
  })

  it('asks which image job to run when files are dropped on the window', () => {
    const result = intake(null, [file('a.png'), file('b.jpg')])
    expect(result).toMatchObject({ state: 'choose' })
    if (result.state !== 'choose') return
    expect(result.kinds[0]).toBe('merge')
    expect(result.kinds).toContain('image-to-pdf')
    expect(result.kinds).not.toContain('word-to-pdf')
  })

  it('refuses a mixed drop and a file the chosen tool cannot read', () => {
    expect(intake(null, [file('a.pdf'), file('b.png')])).toEqual({ state: 'stop', failure: 'mixed' })
    expect(intake('pdf-to-word', [file('notes.docx')])).toEqual({ state: 'stop', failure: 'type' })
    expect(intake('merge', [])).toEqual({ state: 'stop', failure: 'empty' })
    expect(
      intake(
        'merge',
        Array.from({ length: PDF_TOOL_FILE_LIMIT + 1 }, (_, index) => file(`${index}.png`))
      )
    ).toEqual({
      state: 'stop',
      failure: 'too-many'
    })
    expect(intake('merge', [file('a.png', PDF_TOOL_BYTE_LIMIT + 1)])).toEqual({ state: 'stop', failure: 'too-big' })
    expect(failureKey('mixed')).toBe('workbench.feature.pdf.tools.job.mixed')
  })

  it('accepts every tool and records the engine that tool still needs', () => {
    expect(intake('merge', [file('a.png'), file('b.png')])).toEqual({ state: 'run', kind: 'merge' })
    expect(intake('word-to-pdf', [file('notes.docx')])).toEqual({ state: 'run', kind: 'word-to-pdf' })
    expect(intake('pdf-to-image', [file('scan.pdf')])).toEqual({ state: 'run', kind: 'pdf-to-image' })
    expect(intake('image-to-pdf', [file('page.png')])).toEqual({ state: 'run', kind: 'image-to-pdf' })
    expect(intake('rotate', [file('page.png')])).toEqual({ state: 'run', kind: 'rotate' })
    expect(intake('filter', [file('page.png')])).toEqual({ state: 'run', kind: 'filter' })
    expect(engineForKind('merge', 'image')).toBe('local')
    expect(engineForKind('word-to-pdf', 'word')).toBe('office')
    expect(engineForKind('pdf-to-image', 'pdf')).toBe('pdfium')
    expect(engineForKind('extract', 'image')).toBe('ocr')
    expect(engineForKind('audio', 'audio')).toBe('asr')
    expect(engineForKind('erase', 'image')).toBe('inpaint')
    expect(gapKey('inpaint')).toBe('workbench.feature.pdf.tools.job.gap.inpaint')
  })

  it('stacks images on the widest width and refuses a strip that is too tall', () => {
    expect(
      longImageLayout([
        { width: 100, height: 40 },
        { width: 50, height: 50 }
      ])
    ).toEqual({
      width: 100,
      height: 140,
      rows: [{ height: 40 }, { height: 100 }]
    })
    expect(longImageLayout([{ width: 100, height: 40000 }])).toEqual({ error: 'too-long' })
    expect(longImageLayout([])).toEqual({ error: 'empty' })
  })
})
