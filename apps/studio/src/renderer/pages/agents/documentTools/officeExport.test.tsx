import { Buffer } from 'node:buffer'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import process from 'node:process'

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import ExcelJS from 'exceljs'
import { PDFDocument } from 'pdf-lib'
import sharp from 'sharp'
import { describe, expect, it, vi } from 'vitest'

import { captureOfficePages, writeOfficeDocx } from '../pdfToolsOffice'
import { pdfiumPages } from '../pdfToolsPdfium'
import { decodePng } from '../pdfToolsPng'
import { runPdfToolJob, type PdfToolRunResult } from '../pdfToolsRun'
import { DocumentTaskApplication } from './application'
import { DocumentWorkspace } from './DocumentWorkspace'
import { DEFAULT_OFFICE_EXPORT, type OfficeExportOptions } from './officeSvg'
import { createDocumentTask } from './task'
import { documentToolEngine, type DocumentArtifact } from './toolEngine'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
const PREFIX = 'workbench.feature.pdf.tools.workspace.'

async function retain(name: string, bytes: Uint8Array) {
  const root = process.env.A3S_CLOUD_DOCUMENT_TOOLS_EVIDENCE_DIR
  if (!root) return
  const dir = join(root, 'office-artifacts')
  await mkdir(dir, { recursive: true })
  await writeFile(join(dir, name), bytes)
}

function done(result: PdfToolRunResult) {
  if (result.state !== 'done') throw new Error(JSON.stringify(result))
  return result
}

async function workbook() {
  const book = new ExcelJS.Workbook()
  book.addWorksheet('First').getCell('A1').value = 'first-only'
  book.addWorksheet('Second').getCell('A1').value = { formula: '1+1', result: 99 }
  return {
    name: 'book.xlsx',
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    bytes: new Uint8Array(await book.xlsx.writeBuffer())
  }
}

describe('real Office range and result identity', () => {
  it('exports selected workbook sheets in explicit order with accurate PNG metadata', async () => {
    const file = await workbook()
    const result = done(
      await runPdfToolJob({
        kind: 'excel-to-image',
        files: [file],
        officeExport: { ...DEFAULT_OFFICE_EXPORT, pageRange: '2,1,2' }
      })
    )
    const capture = await captureOfficePages(file.bytes, file.name)
    await retain('ranges-source.xlsx', file.bytes)
    expect(capture.text).toContain('99')
    expect(result.files.map((file) => file.name)).toEqual(['book-2.png', 'book-1.png'])
    for (const [index, file] of result.files.entries()) {
      const original = capture.pages[index === 0 ? 1 : 0]
      expect(file.bytes).toEqual(original)
      const raster = await decodePng(file.bytes)
      expect([file.width, file.height]).toEqual([raster.width, raster.height])
      expect(file.sourceSheet).toBe(index === 0 ? 2 : 1)
      await retain(file.name, file.bytes)
    }
  })

  it('reopens a selected-sheet PDF without repeating the whole workbook text on that page', async () => {
    const file = await workbook()
    const result = done(
      await runPdfToolJob({
        kind: 'excel-to-pdf',
        files: [file],
        officeExport: { ...DEFAULT_OFFICE_EXPORT, pageRange: '2' }
      })
    )
    const pdf = await PDFDocument.load(result.files[0].bytes)
    expect(pdf.getPageCount()).toBe(1)
    expect((await pdfiumPages(result.files[0].bytes, 96)).texts.join('')).toBe('')
    const capture = await captureOfficePages(file.bytes, file.name, undefined, '2')
    const raster = await decodePng(capture.pages[0])
    expect(pdf.getPage(0).getSize()).toEqual({ width: (raster.width * 72) / 96, height: (raster.height * 72) / 96 })
    expect(result.note).toBe('workbench.feature.pdf.tools.job.office_semantic_pdf')
    await retain('selected-source.xlsx', file.bytes)
    await retain('selected-sheet-2.pdf', result.files[0].bytes)
  })

  it('exports actual JPEG bytes and refuses nonexistent sheet numbers', async () => {
    const file = await workbook()
    const result = done(
      await runPdfToolJob({
        kind: 'excel-to-image',
        files: [file],
        officeExport: { pageRange: '2', imageFormat: 'jpeg' }
      })
    )
    const out = result.files[0]
    expect(out.name).toBe('book-2.jpg')
    expect(out.mediaType).toBe('image/jpeg')
    const metadata = await sharp(Buffer.from(out.bytes)).metadata()
    expect([metadata.width, metadata.height, metadata.format]).toEqual([out.width, out.height, 'jpeg'])
    await retain('jpeg-source.xlsx', file.bytes)
    await retain(out.name, out.bytes)
    expect(
      await runPdfToolJob({
        kind: 'excel-to-pdf',
        files: [file],
        officeExport: { ...DEFAULT_OFFICE_EXPORT, pageRange: '3' }
      })
    ).toMatchObject({ state: 'failed', reason: PREFIX + 'office.range_error' })
  })

  it('keeps optional settings collapsed and removes outdated downloads until real reprocessing', async () => {
    const file = {
      name: 'notes.docx',
      type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      bytes: await writeOfficeDocx('Hello office')
    }
    const load = async () => file
    const application = new DocumentTaskApplication(
      createDocumentTask<DocumentArtifact>('office-options', 'word-to-image', [
        { id: 'source', name: file.name, type: file.type, size: file.bytes.length }
      ]),
      documentToolEngine('word-to-image', load, { officeExport: DEFAULT_OFFICE_EXPORT }),
      true
    )
    const onChange = (officeExport: OfficeExportOptions) =>
      application.revise(documentToolEngine('word-to-image', load, { officeExport }))
    const view = render(
      <DocumentWorkspace
        application={application}
        onBack={() => undefined}
        onFilter={() => undefined}
        onOfficeExport={onChange}
      />
    )
    expect(view.container.querySelector('details')).not.toHaveAttribute('open')
    fireEvent.click(screen.getByRole('button', { name: PREFIX + 'process' }))
    await waitFor(() => expect(screen.getByRole('link')).toHaveAttribute('download', 'notes-1.png'))
    fireEvent.click(screen.getByText(PREFIX + 'options'))
    const revision = application.getSnapshot().revision
    fireEvent.click(screen.getByRole('combobox'))
    fireEvent.click(screen.getByRole('option', { name: 'JPEG' }))
    expect(screen.queryByRole('link')).toBeNull()
    expect(application.getSnapshot().revision).toBeGreaterThan(revision)
    fireEvent.click(screen.getByRole('button', { name: PREFIX + 'process' }))
    await waitFor(() => expect(screen.getByRole('link')).toHaveAttribute('download', 'notes-1.jpg'))
    expect(application.getSnapshot().items[0].accepted?.value.output.files[0].mediaType).toBe('image/jpeg')
    await retain('options-source.docx', file.bytes)
    await retain('notes-1.jpg', application.getSnapshot().items[0].accepted!.value.output.files[0].bytes)
    view.unmount()
    application.dispose()
  })
})
