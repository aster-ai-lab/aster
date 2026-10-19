import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { writeOfficeDocx } from '../pdfToolsOffice'
import { encodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import type * as PdfToolsRunModule from '../pdfToolsRun'
import { WorkbenchPdfToolsWindow } from '../WorkbenchPdfToolsDialog'
import { DocumentTaskApplication } from './application'
import { DocumentWorkspace } from './DocumentWorkspace'
import { createDocumentTask } from './task'
import type { DocumentArtifact } from './toolEngine'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('../AppletPermissionDialog', () => ({
  AppletPermissionGate: ({ children }: { children: ReactNode }) => children
}))
vi.mock('@renderer/pages/documents/OfficeDocumentSurface', () => ({
  OfficeDocumentSurface: () => null
}))
vi.mock('../pdfToolsRun', async (importOriginal) => {
  const actual = await importOriginal<typeof PdfToolsRunModule>()
  return { ...actual, runPdfToolJob: vi.fn(actual.runPdfToolJob) }
})

afterEach(() => vi.clearAllMocks())

describe('reviewed document tool outputs', () => {
  it('shows a single completed PDF with a preview and direct download, without batch controls', async () => {
    const artifact: DocumentArtifact = {
      output: {
        state: 'done',
        kind: 'word-to-pdf',
        files: [{ name: 'report.pdf', mediaType: 'application/pdf', bytes: new Uint8Array([1, 2, 3]) }]
      }
    }
    const engine = vi.fn(async () => artifact)
    const application = new DocumentTaskApplication(
      createDocumentTask<DocumentArtifact>('simple', 'word-to-pdf', [
        { id: 'word', name: 'report.docx', type: '', size: 10 }
      ]),
      engine,
      true
    )
    render(
      <DocumentWorkspace
        application={application}
        onBack={() => undefined}
        onFilter={() => undefined}
        onAddSources={() => undefined}
      />
    )
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.process' }))
    await waitFor(() => expect(screen.getByRole('link')).toHaveAttribute('download', 'report.pdf'))
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.workspace.accept' })
    ).not.toBeInTheDocument()
    expect(screen.queryByText('workbench.feature.pdf.tools.workspace.options')).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.workspace.process' })
    ).not.toBeInTheDocument()
    expect(engine).toHaveBeenCalledOnce()
  })

  it('refuses a semantic substitute when the Word print-layout runtime is unavailable', async () => {
    const bytes = await writeOfficeDocx('Upload, process, download.')
    render(
      <WorkbenchPdfToolsWindow
        initialKind="word-to-pdf"
        initialFiles={[
          new globalThis.File([bytes.slice()], 'simple.docx', {
            type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
          })
        ]}
      />
    )
    await waitFor(() =>
      expect(screen.getByText('workbench.feature.pdf.tools.workspace.office.layout_error')).toBeInTheDocument()
    )
    expect(globalThis.document.querySelector('a[download="simple.pdf"]')).toBeNull()
    expect(screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.workspace.accept' })).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.retry' })).toBeEnabled()
  })

  it('keeps options and generation locked until an actual writer settles', async () => {
    const artifact: DocumentArtifact = {
      output: {
        state: 'done',
        kind: 'filter',
        files: [{ name: 'input.png', mediaType: 'image/png', bytes: new Uint8Array([1]) }]
      }
    }
    const application = new DocumentTaskApplication(
      createDocumentTask<DocumentArtifact>('save-lock', 'filter', [
        { id: 'a', name: 'a.png', type: 'image/png', size: 1 },
        { id: 'b', name: 'b.png', type: 'image/png', size: 1 }
      ]),
      async () => artifact
    )
    await application.run(['a'])
    const candidate = application.getSnapshot().items[0].candidate
    if (!candidate) throw new Error('Missing candidate')
    application.accept('a', candidate)
    let finish: (receipt: { destination: string; savedAt: string }) => void = () => undefined
    const writing = new Promise<{ destination: string; savedAt: string }>((resolve) => {
      finish = resolve
    })
    render(
      <DocumentWorkspace
        application={application}
        onBack={() => undefined}
        onFilter={() => undefined}
        onAddSources={() => undefined}
        onRemoveSources={(ids) => application.remove(ids)}
        writer={() => writing}
      />
    )
    const filter = screen.getByRole('radio', {
      name: 'workbench.feature.pdf.tools.workspace.filter.grayscale'
    })
    const retry = screen.getByRole('button', {
      name: 'workbench.feature.pdf.tools.workspace.retry_selected'
    })
    expect(filter).not.toBeDisabled()
    expect(retry).not.toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.save_as' }))
    expect(filter).toBeDisabled()
    expect(retry).toBeDisabled()
    expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.add_sources' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.remove_selected' })).toBeDisabled()
    expect(screen.getByRole('checkbox', { name: 'workbench.feature.pdf.tools.workspace.select_all' })).toBeDisabled()
    finish({ destination: '/chosen/input.png', savedAt: '2026-10-08T00:00:00Z' })
    await waitFor(() => expect(filter).not.toBeDisabled())
    expect(retry).not.toBeDisabled()
    expect(application.getSnapshot().items[0].saveState).toBe('saved')
  })

  it('uses the remaining and newly added file bytes after keyboard sorting and filter changes', async () => {
    const original = await encodePng(solid(2, 2, [10, 80, 160, 255]))
    const second = await encodePng(solid(2, 2, [160, 20, 90, 255]))
    const third = await encodePng(solid(2, 2, [30, 100, 190, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[
          new globalThis.File([original.slice()], 'first.png', { type: 'image/png' }),
          new globalThis.File([second.slice()], 'second.png', { type: 'image/png' })
        ]}
      />
    )
    await waitFor(() => expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(globalThis.document.querySelectorAll('[data-source-state="review"]')).toHaveLength(2))
    const firstRemove = globalThis.document.querySelector('[data-action="remove"]')
    if (!firstRemove) throw new Error('Missing remove control')
    fireEvent.click(firstRemove)
    const input = globalThis.document.querySelector('[data-ui="document-tools.add-input"]')
    if (!input) throw new Error('Missing add input')
    fireEvent.change(input, {
      target: { files: [new globalThis.File([third.slice()], 'third.png', { type: 'image/png' })] }
    })
    expect(globalThis.document.querySelectorAll('[data-source-state="ready"]')).toHaveLength(2)
    const lastMoveUp = [...globalThis.document.querySelectorAll('[data-action="move-up"]')].at(-1)
    if (!lastMoveUp) throw new Error('Missing move control')
    fireEvent.click(lastMoveUp)
    expect([...globalThis.document.querySelectorAll('[data-source-id]')].map((button) => button.textContent)).toEqual([
      expect.stringContaining('third.png'),
      expect.stringContaining('second.png')
    ])
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.retry_selected' }))
    await waitFor(() => expect(globalThis.document.querySelectorAll('[data-source-state="review"]')).toHaveLength(2))
    fireEvent.click(globalThis.document.querySelectorAll('[data-source-id]')[0])
    fireEvent.click(screen.getByRole('radio', { name: 'workbench.feature.pdf.tools.workspace.filter.grayscale' }))
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'workbench.feature.pdf.tools.workspace.filters.selected_files' })
    )
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' })).toBeEnabled()
    )
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' }))
    await waitFor(() => expect(globalThis.document.querySelectorAll('[data-source-state="accepted"]')).toHaveLength(2))
    const requests = vi
      .mocked(runPdfToolJob)
      .mock.calls.slice(-2)
      .map(([input]) => input)
    expect(requests.map((input) => input.files[0].name)).toEqual(['third.png', 'second.png'])
    expect(requests[0].files[0].bytes).toEqual(third)
    expect(requests[1].files[0].bytes).toEqual(second)
    expect(
      requests.every(
        (input) => (input.filterDocument?.pages[1]?.name ?? input.filterDocument?.default.name) === 'grayscale'
      )
    ).toBe(true)
  })

  it('appends focused file paste/drop and leaves text-field paste untouched', async () => {
    const png = await encodePng(solid(2, 2, [10, 80, 160, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[new globalThis.File([png.slice()], 'first.png', { type: 'image/png' })]}
      />
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' })
      ).toBeInTheDocument()
    )
    const workspace = globalThis.document.querySelector('[data-ui="document-tools.workspace"]')
    const input = globalThis.document.querySelector('[data-ui="document-tools.add-input"]')
    if (!workspace || !input) throw new Error('Missing workspace')
    const pasted = new globalThis.File([png.slice()], 'pasted.png', { type: 'image/png' })
    fireEvent.paste(workspace, { clipboardData: { files: [pasted] } })
    expect(globalThis.document.querySelectorAll('[data-source-id]')).toHaveLength(2)
    fireEvent.paste(input, { clipboardData: { files: [pasted] } })
    expect(globalThis.document.querySelectorAll('[data-source-id]')).toHaveLength(2)
    fireEvent.drop(workspace, {
      dataTransfer: {
        files: [new globalThis.File(['bad'], 'unsupported.txt', { type: 'text/plain' })]
      }
    })
    expect(globalThis.document.querySelectorAll('[data-source-id]')).toHaveLength(3)
    expect(globalThis.document.querySelectorAll('[data-source-state="rejected"]')).toHaveLength(1)
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('checkbox', { name: 'workbench.feature.pdf.tools.workspace.select_all' }))
    expect(
      screen.queryByRole('button', {
        name: 'workbench.feature.pdf.tools.workspace.retry_selected'
      })
    ).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('checkbox', { name: 'workbench.feature.pdf.tools.workspace.select_all' }))
    expect(
      screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.retry_selected' })
    ).not.toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.remove_selected' }))
    expect(globalThis.document.querySelectorAll('[data-source-id]')).toHaveLength(0)
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(globalThis.document.querySelectorAll('[data-source-state="rejected"]')).toHaveLength(1)
  })

  it('accepts the exact masked edit candidate without another model call', async () => {
    const png = await encodePng(solid(2, 2, [10, 20, 30, 255]))
    const edited = await encodePng(solid(2, 2, [80, 90, 100, 255]))
    const fetchImpl = vi.fn<typeof globalThis.fetch>(
      async () => new globalThis.Response(edited.slice(), { headers: { 'Content-Type': 'image/png' } })
    )
    const preview = await runPdfToolJob({
      kind: 'erase',
      files: [{ name: 'input.png', type: 'image/png', bytes: png }],
      autoCrop: false,
      maskedEditUrl: 'http://edit.test/v1/images/edits',
      fetchImpl
    })
    expect(preview.state).toBe('preview')
    if (preview.state !== 'preview') throw new Error('Missing preview')
    expect(preview.confirmation.state).toBe('done')
    expect(preview.confirmation.files[0].bytes).toBe(preview.files[0].bytes)
    expect(preview.confirmation.files[0].bytes).toEqual(edited)
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })

  it('changes the reviewed filter locally and confirms the displayed candidate without rerunning', async () => {
    const png = await encodePng(solid(2, 2, [10, 80, 160, 255]))
    render(
      <WorkbenchPdfToolsWindow
        initialKind="filter"
        initialFiles={[new globalThis.File([png.slice()], 'input.png', { type: 'image/png' })]}
      />
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' })
      ).toBeInTheDocument()
    )
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('radio', { name: 'workbench.feature.pdf.tools.workspace.filter.grayscale' }))
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' })
      ).toBeInTheDocument()
    )
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(1)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' })).toBeEnabled()
    )
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.workspace.filters.apply' }))
    await waitFor(() => expect(globalThis.document.querySelector('[data-source-state="accepted"]')).toBeInTheDocument())
    expect(vi.mocked(runPdfToolJob)).toHaveBeenCalledTimes(2)
    expect(vi.mocked(runPdfToolJob).mock.lastCall?.[0].filterDocument?.pages[1]?.name).toBe('grayscale')
    expect(globalThis.document.querySelector('a[download="input.png"]')).toBeInTheDocument()
  })
})
