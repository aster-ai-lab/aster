import { createElement } from 'react'
import { createRoot } from 'react-dom/client'

import { loggerService } from '@logger'

const logger = loggerService.withContext('WordLayoutExport')

type HTMLElement = InstanceType<typeof globalThis.HTMLElement>

import { OFFICE_OPERATION_MS, OFFICE_OUTPUT_LIMIT } from './officeProcess'
import { PDF_EXPORT_PIXEL_LIMIT, PDF_PAGE_PIXEL_LIMIT, selectedPdfPages } from './pdfExport'

/** Export the real DOCX print layout; the native semantic SVG is an inspection view. */
export async function wordLayoutPdf(
  bytes: Uint8Array,
  name: string,
  pageRange: string,
  signal?: InstanceType<typeof globalThis.AbortController>['signal']
): Promise<Uint8Array> {
  if (typeof globalThis.Worker !== 'function') throw new Error('office-layout')
  if (signal?.aborted) throw new Error('abort')
  if (bytes.byteLength > OFFICE_OUTPUT_LIMIT) throw new Error('office-output-limit')
  const controller = new globalThis.AbortController()
  const abort = () => controller.abort(new Error('abort'))
  signal?.addEventListener('abort', abort, { once: true })
  const timer = globalThis.setTimeout(() => controller.abort(new Error('office-timeout')), OFFICE_OPERATION_MS)
  let release: (() => void) | undefined
  try {
    const [core, editors] = await Promise.all([
      import('@a3s-lab/office/core'),
      import('@a3s-lab/office/react'),
      import('@a3s-lab/office/styles.css')
    ])
    controller.signal.throwIfAborted()
    const artifact = await core.importOfficeFile(new globalThis.File([bytes.slice()], name), {
      signal: controller.signal
    })
    if (artifact.content.type !== 'document') throw new Error('office-layout')
    controller.signal.throwIfAborted()
    const host = globalThis.document.createElement('div')
    host.dataset.ui = 'document-tools.office-layout'
    host.setAttribute('aria-hidden', 'true')
    host.inert = true
    Object.assign(host.style, {
      position: 'fixed',
      left: '-20000px',
      top: '0',
      width: '1400px',
      height: '1200px',
      pointerEvents: 'none'
    })
    globalThis.document.body.append(host)
    const root = createRoot(host)
    release = () => {
      root.unmount()
      host.remove()
    }
    let fail: (error: unknown) => void = () => undefined
    root.render(
      createElement(editors.DocumentEditor, {
        artifactId: artifact.id,
        content: artifact.content,
        preview: true,
        autoFocus: false,
        onChange: () => undefined,
        onError: (error) => {
          logger.error('Document layout failed', error)
          fail(new Error('office-layout'))
        }
      })
    )
    const pageCount = await new Promise<number>((resolve, reject) => {
      const stop = () => {
        observer.disconnect()
        controller.signal.removeEventListener('abort', cancelled)
      }
      fail = (error) => {
        stop()
        reject(error)
      }
      const cancelled = () => {
        stop()
        reject(controller.signal.reason)
      }
      const ready = () => {
        const page = host.querySelector<HTMLElement>('[data-work-pdf-live-document]')
        const count = Number(page?.dataset.pdfPageCount)
        if (
          !host.querySelector('[data-pagination-state="ready"]') ||
          !page ||
          !Number.isSafeInteger(count) ||
          count <= 0
        )
          return
        const width = (Number(page.dataset.pdfPageWidthPoints) * 2 * 96) / 72
        const height = (Number(page.dataset.pdfPageHeightPoints) * 2 * 96) / 72
        if (
          count > 100 ||
          !Number.isFinite(width * height) ||
          width * height > PDF_PAGE_PIXEL_LIMIT ||
          width * height * count > PDF_EXPORT_PIXEL_LIMIT
        ) {
          stop()
          reject(new Error('office-output-limit'))
          return
        }
        stop()
        resolve(count)
      }
      const observer = new globalThis.MutationObserver(ready)
      observer.observe(host, { subtree: true, childList: true, attributes: true })
      controller.signal.addEventListener('abort', cancelled, { once: true })
      if (controller.signal.aborted) cancelled()
      else ready()
    })
    controller.signal.throwIfAborted()
    const pageIndexes = selectedPdfPages(pageRange, pageCount)
    const pdf = await core.createArtifactPdf(artifact, {
      pageIndexes: [...pageIndexes],
      signal: controller.signal,
      rasterOnly: true
    })
    controller.signal.throwIfAborted()
    if (pdf.byteLength > OFFICE_OUTPUT_LIMIT) throw new Error('office-output-limit')
    return pdf
  } catch (error) {
    logger.error('Word PDF export failed', error instanceof Error ? error : new Error(String(error)))
    const message = error instanceof Error ? error.message : ''
    if (['abort', 'office-timeout', 'office-output-limit', 'office-layout'].includes(message)) throw error
    if (message === 'pdf-page-range') throw new Error('office-page-range')
    throw new Error('office-layout')
  } finally {
    release?.()
    globalThis.clearTimeout(timer)
    signal?.removeEventListener('abort', abort)
  }
}
