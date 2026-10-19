import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { encodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob, type PdfToolRunResult } from '../pdfToolsRun'
import type * as PdfToolsRunModule from '../pdfToolsRun'
import { WorkbenchPdfToolsWindow } from '../WorkbenchPdfToolsDialog'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { name?: string }) => key + (options?.name ? ` ${options.name}` : '')
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
const PREFIX = 'workbench.feature.pdf.tools.workspace.'
afterEach(() => vi.clearAllMocks())
async function files() {
  const red = await encodePng(solid(4, 2, [255, 0, 0, 255]))
  const blue = await encodePng(solid(2, 2, [0, 0, 255, 255]))
  return [
    new globalThis.File([red.slice()], 'red.png', { type: 'image/png' }),
    new globalThis.File([blue.slice()], 'blue.png', { type: 'image/png' })
  ]
}

describe('assembly interaction', () => {
  it('withdraws outdated downloads after keyboard sorting and changing segment settings, then exports the new revision', async () => {
    const view = render(<WorkbenchPdfToolsWindow initialKind="merge" initialFiles={await files()} />)
    await waitFor(() =>
      expect(screen.getByRole('link', { name: PREFIX + 'download_file long.png' })).toHaveAttribute(
        'download',
        'long.png'
      )
    )
    expect(view.container.querySelector('[data-ui="document-tools.assembly-options"]')).not.toHaveAttribute('open')
    fireEvent.click(screen.getByRole('button', { name: PREFIX + 'move_up blue.png' }))
    expect(screen.queryByRole('link')).toBeNull()
    expect(
      [...view.container.querySelectorAll('[data-ui="document-tools.assembly-sources"] li')].map(
        (row) => row.textContent
      )
    ).toEqual(['blue.png', 'red.png'])
    fireEvent.click(screen.getByText(PREFIX + 'options'))
    fireEvent.change(screen.getByLabelText(PREFIX + 'assembly.segment'), { target: { value: '2' } })
    fireEvent.click(screen.getByRole('button', { name: PREFIX + 'process' }))
    await waitFor(() =>
      expect(screen.getByRole('link', { name: PREFIX + 'download_file long-2.png' })).toBeInTheDocument()
    )
    expect(screen.queryByRole('link', { name: PREFIX + 'download_file long.png' })).toBeNull()
    expect(screen.getByRole('img', { name: PREFIX + 'preview_of long-1.png' })).toBeInTheDocument()
    expect(screen.getByRole('img', { name: PREFIX + 'preview_of long-2.png' })).toBeInTheDocument()
  })

  it('hides batch sorting for a single image and removes a PDF download until changed settings are applied', async () => {
    const view = render(
      <WorkbenchPdfToolsWindow initialKind="image-to-pdf" initialFiles={(await files()).slice(0, 1)} />
    )
    await waitFor(() =>
      expect(screen.getByRole('link', { name: PREFIX + 'download_file images.pdf' })).toBeInTheDocument()
    )
    expect(view.container.querySelector('[data-ui="document-tools.assembly-sources"]')).toBeNull()
    fireEvent.click(screen.getByText(PREFIX + 'options'))
    expect(screen.getByLabelText(PREFIX + 'assembly.orientation')).toBeDisabled()
    fireEvent.click(screen.getByRole('combobox', { name: PREFIX + 'assembly.paper' }))
    fireEvent.click(screen.getByRole('option', { name: PREFIX + 'assembly.letter' }))
    expect(screen.queryByRole('link', { name: PREFIX + 'download_file images.pdf' })).toBeNull()
    expect(screen.getByLabelText(PREFIX + 'assembly.orientation')).toBeEnabled()
    fireEvent.change(screen.getByLabelText(PREFIX + 'assembly.margin'), { target: { value: '36' } })
    fireEvent.click(screen.getByRole('button', { name: PREFIX + 'process' }))
    await waitFor(() =>
      expect(screen.getByRole('link', { name: PREFIX + 'download_file images.pdf' })).toBeInTheDocument()
    )
  })

  it('stays busy after cancelling until the owned converter settles and never exposes its late result', async () => {
    let finish: (result: PdfToolRunResult) => void = () => undefined
    vi.mocked(runPdfToolJob).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve
        })
    )
    render(<WorkbenchPdfToolsWindow initialKind="merge" initialFiles={await files()} />)
    const cancel = await screen.findByRole('button', { name: 'workbench.feature.pdf.tools.job.cancel' })
    fireEvent.click(cancel)
    expect(cancel).toBeDisabled()
    expect(screen.getByText(PREFIX + 'cancelling')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: PREFIX + 'move_up blue.png' })).toBeDisabled()
    await act(async () =>
      finish({
        state: 'done',
        kind: 'merge',
        files: [{ name: 'late.png', mediaType: 'image/png', bytes: new Uint8Array([1]) }]
      })
    )
    await waitFor(() => expect(screen.getByText(PREFIX + 'state.cancelled')).toBeInTheDocument())
    expect(screen.queryByRole('link')).toBeNull()
    expect(screen.getByRole('button', { name: PREFIX + 'process' })).toBeEnabled()
  })
})
