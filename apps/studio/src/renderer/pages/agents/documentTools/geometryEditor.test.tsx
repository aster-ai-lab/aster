import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { PDFDocument } from 'pdf-lib'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { decodePng, encodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import type * as PdfToolsRunModule from '../pdfToolsRun'
import { WorkbenchPdfToolsWindow } from '../WorkbenchPdfToolsDialog'

const locale = vi.hoisted(() => ({ suffix: '' }))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'workbench.feature.pdf.tools.title' ? key + locale.suffix : key)
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
afterEach(() => {
  locale.suffix = ''
  vi.clearAllMocks()
})
const P = 'workbench.feature.pdf.tools.workspace.geometry.'
function number(label: string, value: number) {
  fireEvent.change(screen.getByRole('spinbutton', { name: P + label }), { target: { value: String(value) } })
}
async function ready(dimensions: string) {
  await waitFor(() =>
    expect(globalThis.document.querySelector('[data-ui="document-tools.geometry-dimensions"]')).toHaveTextContent(
      dimensions
    )
  )
}
function numeric() {
  fireEvent.click(screen.getByText(P + 'numeric', { selector: 'summary' }))
}

describe('production geometry editor', () => {
  it('preserves initial dimensions, previews/undoes edits locally, and applies the original crop once', async () => {
    const raster = solid(4, 3, [10, 20, 30, 255])
    raster.rgba.set([70, 80, 90, 255], (1 * 4 + 1) * 4)
    const png = await encodePng(raster)
    render(
      <WorkbenchPdfToolsWindow
        initialKind="rotate"
        initialFiles={[new globalThis.File([png.slice()], 'original.png', { type: 'image/png' })]}
      />
    )
    await ready('4 × 3 px')
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledOnce()
    expect(
      screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.workspace.accept' })
    ).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: P + 'right' }))
    await ready('3 × 4 px')
    fireEvent.click(screen.getByRole('button', { name: P + 'undo' }))
    await ready('4 × 3 px')
    numeric()
    number('rect.width', 2)
    number('rect.height', 2)
    number('rect.x', 1)
    await ready('2 × 2 px')
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="original.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    const result = await vi.mocked(runPdfToolJob).mock.results[1].value
    if (result.state !== 'preview') throw new Error('Expected real crop')
    const output = await decodePng(result.confirmation.files[0].bytes)
    expect([output.width, output.height]).toEqual([2, 2])
    expect([...output.rgba.subarray(8, 12)]).toEqual([70, 80, 90, 255])
    expect(screen.getByRole('button', { name: P + 'apply' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: P + 'restore' }))
    await ready('4 × 3 px')
    expect(globalThis.document.querySelector('a[download="original.png"]')).not.toBeInTheDocument()
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
  })
  it('preserves all PDF pages and applies whole-document rotation without an extra confirmation', async () => {
    const pdf = await PDFDocument.create()
    pdf.addPage([100, 60])
    pdf.addPage([120, 80])
    const bytes = await pdf.save()
    render(
      <WorkbenchPdfToolsWindow
        initialKind="rotate"
        initialFiles={[new globalThis.File([bytes.slice()], 'pages.pdf', { type: 'application/pdf' })]}
      />
    )
    await waitFor(() => expect(screen.getByRole('button', { name: P + 'right' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: P + 'left' }))
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="pages.pdf"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    const result = await vi.mocked(runPdfToolJob).mock.results[1].value
    if (result.state !== 'preview') throw new Error('Missing PDF result')
    const reopened = await PDFDocument.load(result.files[0].bytes)
    expect(reopened.getPages().map((page) => page.getRotation().angle)).toEqual([270, 270])
    expect(
      screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.workspace.accept' })
    ).not.toBeInTheDocument()
  })

  it('keeps the active task usable when a locale change updates only the window title', async () => {
    const png = await encodePng(solid(4, 3, [10, 20, 30, 255]))
    const files = [new globalThis.File([png.slice()], 'localized.png', { type: 'image/png' })]
    const view = render(<WorkbenchPdfToolsWindow initialKind="rotate" initialFiles={files} />)
    await ready('4 × 3 px')
    locale.suffix = ' en'
    view.rerender(<WorkbenchPdfToolsWindow initialKind="rotate" initialFiles={files} />)
    fireEvent.click(screen.getByRole('button', { name: P + 'right' }))
    await ready('3 × 4 px')
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="localized.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
  })

  it('keeps another accepted source downloadable while committing only the current geometry', async () => {
    const png = await encodePng(solid(4, 3, [10, 20, 30, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="rotate"
        initialFiles={['first.png', 'second.png'].map(
          (name) => new globalThis.File([png.slice()], name, { type: 'image/png' })
        )}
      />
    )
    await ready('4 × 3 px')
    await waitFor(() => expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2))
    fireEvent.click(globalThis.document.querySelectorAll('[data-source-id]')[1])
    await ready('4 × 3 px')
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="second.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    fireEvent.click(globalThis.document.querySelectorAll('[data-source-id]')[0])
    await ready('4 × 3 px')
    numeric()
    number('rect.width', 2)
    await ready('2 × 3 px')
    fireEvent.click(screen.getByRole('button', { name: P + 'apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('a[download="first.png"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(3)
    expect(vi.mocked(runPdfToolJob).mock.lastCall?.[0].files[0].name).toBe('first.png')
    fireEvent.click(globalThis.document.querySelectorAll('[data-source-id]')[1])
    await ready('4 × 3 px')
    expect(globalThis.document.querySelector('a[download="second.png"]')).toBeInTheDocument()
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(3)
  })
})
