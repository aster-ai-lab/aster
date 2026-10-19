import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { PDFDocument, rgb } from 'pdf-lib'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { decodePng, encodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import type * as PdfToolsRunModule from '../pdfToolsRun'
import { WorkbenchPdfToolsWindow } from '../WorkbenchPdfToolsDialog'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, number>) => key + (values?.page ? String(values.page) : '')
  })
}))
vi.mock('../AppletPermissionDialog', () => ({
  AppletPermissionGate: ({ children }: { children: ReactNode }) => children
}))
vi.mock('@renderer/pages/documents/OfficeDocumentSurface', () => ({ OfficeDocumentSurface: () => null }))
vi.mock('../pdfToolsRun', async (importOriginal) => {
  const actual = await importOriginal<typeof PdfToolsRunModule>()
  return { ...actual, runPdfToolJob: vi.fn(actual.runPdfToolJob) }
})
afterEach(() => vi.clearAllMocks())
const P = 'workbench.feature.pdf.tools.workspace.filters.'
const L = 'workbench.feature.pdf.tools.workspace.filter.'
async function ready() {
  await waitFor(() => expect(screen.getByRole('button', { name: P + 'apply' })).toBeEnabled())
}
async function pixels(call: number, page = 0) {
  const result = await vi.mocked(runPdfToolJob).mock.results[call].value
  if (result.state !== 'preview') throw new Error('Missing actual pixels')
  return [...(await decodePng(result.files[page].bytes)).rgba.slice(0, 4)]
}
describe('production filter editor', () => {
  it('keeps original by default, previews locally, undoes, compares and applies exact intensity once', async () => {
    const png = await encodePng(solid(4, 4, [10, 80, 160, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[new globalThis.File([png.slice()], 'source.png', { type: 'image/png' })]}
      />
    )
    await ready()
    expect(await pixels(0)).toEqual([10, 80, 160, 255])
    expect(screen.getByRole('radio', { name: L + 'original' })).toBeChecked()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="source.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('radio', { name: L + 'grayscale' }))
    await ready()
    fireEvent.change(screen.getByRole('slider', { name: P + 'intensity' }), { target: { value: '50' } })
    await ready()
    expect(globalThis.document.querySelector('a[download="source.png"]')).not.toBeInTheDocument()
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: P + 'undo' }))
    expect(screen.getByRole('slider', { name: P + 'intensity' })).toHaveValue('100')
    fireEvent.change(screen.getByRole('slider', { name: P + 'intensity' }), { target: { value: '50' } })
    await ready()
    fireEvent.click(screen.getByRole('combobox', { name: P + 'comparison' }))
    fireEvent.click(screen.getByRole('option', { name: P + 'split' }))
    expect(screen.getByRole('slider', { name: P + 'split_position' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: P + 'show_original' }))
    expect(screen.queryByAltText(P + 'result_image')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: P + 'show_original' }))
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="source.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    expect(await pixels(1)).toEqual([41, 76, 116, 255])
    fireEvent.click(screen.getByRole('button', { name: P + 'restore' }))
    await ready()
    expect(globalThis.document.querySelector('a[download="source.png"]')).not.toBeInTheDocument()
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
  })
  it('keeps untouched real PDF pages original and explicitly applies to selected pages', async () => {
    const pdf = await PDFDocument.create()
    for (const color of [rgb(1, 0, 0), rgb(0, 1, 0)])
      pdf.addPage([20, 12]).drawRectangle({ x: 0, y: 0, width: 20, height: 12, color })
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[
          new globalThis.File([new Uint8Array(await pdf.save())], 'pages.pdf', { type: 'application/pdf' })
        ]}
      />
    )
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'page2' }))
    fireEvent.click(screen.getByRole('radio', { name: L + 'grayscale' }))
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="pages-page-2.png"]')).toBeInTheDocument())
    expect(await pixels(1, 0)).toEqual([255, 0, 0, 255])
    expect(await pixels(1, 1)).toEqual([182, 182, 182, 255])
    fireEvent.click(screen.getByRole('checkbox', { name: P + 'select_page2' }))
    fireEvent.click(screen.getByRole('checkbox', { name: P + 'selected_pages' }))
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(3))
    expect(await pixels(2, 0)).toEqual([54, 54, 54, 255])
    expect(await pixels(2, 1)).toEqual([182, 182, 182, 255])
    await waitFor(() => expect(globalThis.document.querySelector('a[download="pages-page-1.png"]')).toBeInTheDocument())
    expect(screen.queryByText('workbench.feature.pdf.tools.workspace.geometry.unapplied')).not.toBeInTheDocument()
  })
  it('applies explicit selected-file scope to a newly appended file using its own original bytes', async () => {
    const first = await encodePng(solid(4, 4, [10, 80, 160, 255]))
    const second = await encodePng(solid(4, 4, [30, 100, 190, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[new globalThis.File([first.slice()], 'first.png', { type: 'image/png' })]}
      />
    )
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="first.png"]')).toBeInTheDocument())
    fireEvent.change(globalThis.document.querySelector('[data-ui="document-tools.add-input"]')!, {
      target: { files: [new globalThis.File([second.slice()], 'second.png', { type: 'image/png' })] }
    })
    fireEvent.click(screen.getByRole('radio', { name: L + 'grayscale' }))
    fireEvent.click(screen.getByRole('checkbox', { name: P + 'selected_files' }))
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(3))
    expect(await pixels(1)).toEqual([71, 71, 71, 255])
    expect(await pixels(2)).toEqual([92, 92, 92, 255])
  })
  it('retains the active page and explicit scope when its first selected-page Apply remounts the editor', async () => {
    const pdf = await PDFDocument.create()
    for (const color of [rgb(1, 0, 0), rgb(0, 1, 0)])
      pdf.addPage([20, 12]).drawRectangle({ x: 0, y: 0, width: 20, height: 12, color })
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[
          new globalThis.File([new Uint8Array(await pdf.save())], 'first-scope.pdf', { type: 'application/pdf' })
        ]}
      />
    )
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'page2' }))
    fireEvent.click(screen.getByRole('radio', { name: L + 'grayscale' }))
    fireEvent.click(screen.getByRole('checkbox', { name: P + 'select_page2' }))
    fireEvent.click(screen.getByRole('checkbox', { name: P + 'selected_pages' }))
    await ready()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() =>
      expect(globalThis.document.querySelector('a[download="first-scope-page-2.png"]')).toBeInTheDocument()
    )
    expect(screen.getByRole('button', { name: P + 'page2' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('checkbox', { name: P + 'selected_pages' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: P + 'select_page2' })).toBeChecked()
    expect(await pixels(1, 0)).toEqual([54, 54, 54, 255])
  })
})
