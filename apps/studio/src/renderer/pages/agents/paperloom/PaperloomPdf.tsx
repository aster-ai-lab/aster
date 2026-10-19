import '@renderer/assets/styles/vendor/pdf-viewer.css'
import { getDocument, GlobalWorkerOptions, TextLayer, type PDFDocumentProxy } from 'pdfjs-dist'
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.mjs?url'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { PdfjsBundledCMapReaderFactory, PdfjsBundledStandardFontDataFactory } from '@renderer/components/FilePreview'

import { usePaperloomClient } from './PaperloomClient'

GlobalWorkerOptions.workerSrc = pdfWorkerUrl

let framePatches = 0
let nativeFrame: typeof window.requestAnimationFrame | null = null
let nativeCancel: typeof window.cancelAnimationFrame | null = null

// shortcut: one render holds this patch, a second simultaneous page restores the native frame early.
function holdAnimationFrames(): () => void {
  if (framePatches === 0) {
    nativeFrame = window.requestAnimationFrame.bind(window)
    nativeCancel = window.cancelAnimationFrame.bind(window)
    window.requestAnimationFrame = (callback: FrameRequestCallback) => {
      queueMicrotask(() => callback(performance.now()))
      return 0
    }
    window.cancelAnimationFrame = () => undefined
  }
  framePatches += 1
  let released = false
  return () => {
    if (released) return
    released = true
    framePatches -= 1
    if (framePatches === 0 && nativeFrame && nativeCancel) {
      window.requestAnimationFrame = nativeFrame
      window.cancelAnimationFrame = nativeCancel
      nativeFrame = null
      nativeCancel = null
    }
  }
}

/** Same bundled PDF engine and fonts as Studio's file preview; no browser PDF frame. */
export function PaperloomPdf({
  path,
  source,
  page,
  zoom,
  label,
  caption = false,
  onPages,
  onReady
}: {
  path: string
  source?: Uint8Array
  page: number
  zoom: number
  label: string
  caption?: boolean
  onPages?: (pages: number) => void
  onReady?: () => void
}) {
  const { t } = useTranslation()
  const { paperloomResponse } = usePaperloomClient()
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null)
  const [error, setError] = useState(false)
  const [width, setWidth] = useState(0)
  const [busy, setBusy] = useState(true)
  const [pageText, setPageText] = useState('')
  const host = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const text = useRef<HTMLDivElement>(null)
  const callbacks = useRef({ onPages, onReady })
  callbacks.current = { onPages, onReady }
  useEffect(() => {
    const abort = new AbortController()
    let task: ReturnType<typeof getDocument> | undefined
    setPdf(null)
    setError(false)
    setBusy(true)
    const read = source
      ? Promise.resolve(source.slice())
      : paperloomResponse(path, { signal: abort.signal }).then(
          async (response) => new Uint8Array(await response.arrayBuffer())
        )
    void read
      .then(async (bytes) => {
        if (abort.signal.aborted) return
        task = getDocument({
          data: bytes,
          isEvalSupported: false,
          CMapReaderFactory: PdfjsBundledCMapReaderFactory,
          StandardFontDataFactory: PdfjsBundledStandardFontDataFactory
        })
        const pdf = await task.promise
        if (abort.signal.aborted) return
        setPdf(pdf)
        callbacks.current.onPages?.(pdf.numPages)
      })
      .catch(() => {
        if (!abort.signal.aborted) {
          setError(true)
          setBusy(false)
        }
      })
    return () => {
      abort.abort()
      void task?.destroy().catch(() => undefined)
    }
  }, [path, source, paperloomResponse])
  useEffect(() => {
    const element = host.current
    if (!element) return
    const observer = new ResizeObserver(() => setWidth(element.clientWidth))
    observer.observe(element)
    setWidth(element.clientWidth)
    return () => observer.disconnect()
  }, [])
  useEffect(() => {
    if (!pdf || !canvas.current || !text.current || width < 1) return
    let live = true
    let render: ReturnType<Awaited<ReturnType<PDFDocumentProxy['getPage']>>['render']> | undefined
    let layer: TextLayer | undefined
    const canvasNode = canvas.current
    const textNode = text.current
    const releaseFrames = holdAnimationFrames()
    setBusy(true)
    setError(false)
    void (async () => {
      try {
        if (page > pdf.numPages) {
          setBusy(false)
          return
        }
        const pdfPage = await pdf.getPage(page)
        if (!live) return
        const textContent = await pdfPage.getTextContent()
        if (!live) return
        const items = (textContent as { items?: { str?: string }[] }).items ?? []
        setPageText(
          items
            .map((item) => item.str ?? '')
            .filter(Boolean)
            .join('\n')
        )
        const base = pdfPage.getViewport({ scale: 1 })
        const viewport = pdfPage.getViewport({
          scale: Math.max(0.1, (width - 32) / base.width) * zoom
        })
        const ratio = window.devicePixelRatio || 1
        canvasNode.width = Math.ceil(viewport.width * ratio)
        canvasNode.height = Math.ceil(viewport.height * ratio)
        canvasNode.style.width = `${viewport.width}px`
        canvasNode.style.height = `${viewport.height}px`
        textNode.replaceChildren()
        textNode.style.setProperty('--scale-factor', String(viewport.scale))
        textNode.style.width = `${viewport.width}px`
        textNode.style.height = `${viewport.height}px`
        render = pdfPage.render({
          canvas: canvasNode,
          viewport,
          intent: 'display',
          transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0]
        })
        // A background webview may never deliver animation frames or timers.
        render.onContinue = (resume) => {
          queueMicrotask(() => {
            if (live) resume()
          })
        }
        layer = new TextLayer({
          textContentSource: textContent,
          container: textNode,
          viewport
        })
        await Promise.all([render.promise, layer.render()])
        if (live) {
          setBusy(false)
          callbacks.current.onReady?.()
        }
      } catch {
        if (live) {
          setError(true)
          setBusy(false)
        }
      } finally {
        releaseFrames()
      }
    })()
    return () => {
      live = false
      render?.cancel()
      layer?.cancel()
      releaseFrames()
    }
  }, [pdf, width, page, zoom])
  const missingPage = !!pdf && page > pdf.numPages
  return (
    <section
      ref={host}
      aria-label={label}
      className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-auto bg-[#f5f5f5]"
      data-paperloom-pdf="">
      {caption && (
        <p className="sticky top-0 z-10 shrink-0 bg-white/95 px-3 py-2 text-[12px] text-[#8a8a8a]">{label}</p>
      )}
      {error ? (
        <p role="alert" className="p-4 text-[#b42318]">
          {t('paperloom.pdf_failed')}
        </p>
      ) : missingPage ? (
        <p className="p-4 text-[#8a8a8a]">{t('paperloom.no_corresponding_page')}</p>
      ) : (
        <>
          {busy && (
            <p role="status" className={`absolute left-4 text-[#8a8a8a] ${caption ? 'top-12' : 'top-4'}`}>
              {t('paperloom.loading')}
            </p>
          )}
          <div
            className="relative m-4 self-start bg-white shadow-sm"
            style={{ visibility: busy ? 'hidden' : 'visible' }}>
            <canvas ref={canvas} aria-label={pageText || `${label} ${page}`} />
            <div ref={text} className="textLayer absolute inset-0" />
          </div>
        </>
      )}
    </section>
  )
}
