import type * as OfficeCoreModule from '@a3s-lab/office/core'
import type { OfficeArtifact } from '@a3s-lab/office/core'
import type * as OfficeReactModule from '@a3s-lab/office/react'

import './fileEditorOffice.css'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { fileExtension, type OfficePreviewKind } from './documentPreview'
import { FileEditorLoading } from './FileEditorLoading'
import { WorkspaceNotice, WorkspaceToolbar } from './WorkspaceFileChrome'

type OfficeReact = typeof OfficeReactModule

let officeReactPromise: Promise<OfficeReact> | undefined
let officeCorePromise: Promise<typeof OfficeCoreModule> | undefined

/** Start the editor chunks while the file bytes are still downloading. */
export function prefetchOfficeEditors(): void {
  officeReactPromise ??= import('@a3s-lab/office/react')
  officeCorePromise ??= import('@a3s-lab/office/core')
  void import('@a3s-lab/office/styles.css').catch(() => undefined)
}

function officeReact(): Promise<OfficeReact> {
  officeReactPromise ??= import('@a3s-lab/office/react')
  return officeReactPromise
}

function officeCore(): Promise<typeof OfficeCoreModule> {
  officeCorePromise ??= import('@a3s-lab/office/core')
  return officeCorePromise
}

type LoadState =
  | { status: 'loading' }
  | { status: 'failed' }
  | { status: 'pdf'; editors: OfficeReact }
  | { status: 'editor'; editors: OfficeReact; artifact: OfficeArtifact }

export function OfficeDocumentSurface({
  surface,
  bytes,
  fileName,
  frame = 'page',
  preview = false,
  onSave,
  onFail,
  onMarkdownChange,
  onOfficeDirty
}: {
  surface: OfficePreviewKind
  bytes: Uint8Array
  fileName: string
  frame?: 'page' | 'workspace'
  /** Read-only result preview; existing document hosts remain editable by default. */
  preview?: boolean
  onSave?: (bytes: Uint8Array) => Promise<void>
  /** Called when the visual editor cannot open the bytes. */
  onFail?: () => void
  /** Markdown drafts stay in the text buffer so source mode and the dirty dot match. */
  onMarkdownChange?: (markdown: string) => void
  /** Structured office edits that are not a text draft. */
  onOfficeDirty?: (dirty: boolean) => void
}) {
  const { t } = useTranslation()
  const [loaded, setLoaded] = useState<LoadState>({ status: 'loading' })
  const [pdfRevision, setPdfRevision] = useState(0)
  const [progress, setProgress] = useState<number | null>(null)
  const [exportFailed, setExportFailed] = useState(false)
  const artifactRef = useRef<OfficeArtifact | null>(null)
  const acceptedRef = useRef<{ surface: OfficePreviewKind; fileName: string; bytes: Uint8Array } | null>(null)
  const bytesRef = useRef(bytes)
  const surfaceRef = useRef(surface)
  const fileNameRef = useRef(fileName)
  const onSaveRef = useRef(onSave)
  const onFailRef = useRef(onFail)
  const onMarkdownChangeRef = useRef(onMarkdownChange)
  const onOfficeDirtyRef = useRef(onOfficeDirty)
  bytesRef.current = bytes
  surfaceRef.current = surface
  fileNameRef.current = fileName
  onSaveRef.current = onSave
  onFailRef.current = onFail
  onMarkdownChangeRef.current = onMarkdownChange
  onOfficeDirtyRef.current = onOfficeDirty
  const loadPdf = useCallback(
    () => Promise.resolve(new globalThis.Blob([bytesRef.current.slice()], { type: 'application/pdf' })),
    []
  )

  const acceptSavedBytes = (next: Uint8Array) => {
    const previous = acceptedRef.current
    if (!previous) return null
    acceptedRef.current = { surface: surfaceRef.current, fileName: fileNameRef.current, bytes: next }
    return previous
  }

  useEffect(() => {
    // Saving writes the bytes this editor already shows. Importing them again
    // replaces the editor and drops the caret, scroll position, and undo stack.
    const accepted = acceptedRef.current
    if (accepted?.surface === surface && accepted.fileName === fileName && accepted.bytes === bytes) return
    setPdfRevision((value) => value + 1)
    let cancelled = false
    const abort = new globalThis.AbortController()
    artifactRef.current = null
    setLoaded({ status: 'loading' })
    setProgress(null)
    setExportFailed(false)
    void (async () => {
      try {
        const editors = await officeReact()
        try {
          await import('@a3s-lab/office/styles.css')
        } catch {
          // The editor still mounts when the stylesheet cannot be fetched.
        }
        if (surface === 'pdf') {
          if (!cancelled) {
            acceptedRef.current = { surface, fileName, bytes }
            setLoaded({ status: 'pdf', editors })
          }
          return
        }
        const { importOfficeFile } = await officeCore()
        if (cancelled) return
        let reported = -1
        const imported = await importOfficeFile(new globalThis.File([bytes.slice()], fileName), {
          signal: abort.signal,
          preserveDelimitedText: surface === 'spreadsheet' && ['csv', 'tsv'].includes(fileExtension(fileName)),
          onProgress: (event) => {
            if (cancelled) return
            const step = event.progress >= 1 ? 1 : Math.floor(event.progress * 20) / 20
            if (step === reported) return
            reported = step
            setProgress(step)
          }
        })
        if (imported.kind !== surface) throw new Error('kind')
        if (cancelled) return
        artifactRef.current = imported
        acceptedRef.current = { surface, fileName, bytes }
        setLoaded({ status: 'editor', editors, artifact: imported })
      } catch (error) {
        if (cancelled || (error instanceof Error && error.name === 'AbortError')) return
        setLoaded({ status: 'failed' })
      }
    })()
    return () => {
      cancelled = true
      abort.abort()
    }
  }, [surface, fileName, bytes])

  useEffect(() => {
    if (loaded.status === 'failed') onFailRef.current?.()
  }, [loaded.status])

  const persist = async () => {
    const current = artifactRef.current
    const save = onSaveRef.current
    if (!current || !save) return
    let next: Uint8Array
    try {
      const { createArtifactBlob } = await import('@a3s-lab/office/core')
      const extension = fileExtension(fileNameRef.current)
      const options: OfficeCoreModule.ArtifactExportOptions | undefined =
        current.kind === 'spreadsheet' && (extension === 'csv' || extension === 'tsv')
          ? { spreadsheetFormat: extension }
          : undefined
      const blob = await createArtifactBlob(current, options)
      next = new Uint8Array(await blob.arrayBuffer())
    } catch (error) {
      setExportFailed(true)
      throw error
    }
    const previous = acceptSavedBytes(next)
    try {
      await save(next)
    } catch {
      acceptedRef.current = previous
      return
    }
    setExportFailed(false)
    if (artifactRef.current === current) onOfficeDirtyRef.current?.(false)
  }
  const persistRef = useRef(persist)
  const saveTail = useRef<Promise<unknown>>(Promise.resolve())
  persistRef.current = persist
  const queuedPersist = () => {
    const run = saveTail.current.then(() => persistRef.current())
    saveTail.current = run.then(
      () => undefined,
      () => undefined
    )
    return run
  }

  useEffect(() => {
    if (preview || !onSave || surface === 'pdf') return undefined
    const onKey = (event: InstanceType<typeof globalThis.KeyboardEvent>) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
      if (event.key.toLowerCase() !== 's') return
      event.preventDefault()
      void queuedPersist()
    }
    globalThis.window.addEventListener('keydown', onKey)
    return () => globalThis.window.removeEventListener('keydown', onKey)
  }, [onSave, surface, preview])

  if (loaded.status === 'loading') {
    if (frame === 'workspace') {
      return (
        <WorkspaceNotice
          title={t('file_preview.loading')}
          description={t('file_preview.loading.description')}
          progress={progress}
        />
      )
    }
    return <FileEditorLoading label={t('file_preview.loading')} progress={progress} />
  }
  if (loaded.status === 'failed') {
    if (frame === 'workspace') {
      return (
        <WorkspaceNotice
          title={t('file_preview.unavailable.title')}
          description={t('file_preview.unavailable.description')}
        />
      )
    }
    return (
      <div role="status" className="mt-8">
        <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">{t('file_preview.unavailable.title')}</h3>
        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">{t('file_preview.unavailable.description')}</p>
      </div>
    )
  }

  const remember = (content: OfficeArtifact['content']) => {
    const current = artifactRef.current
    if (!current) return
    artifactRef.current = { ...current, content }
    if (content.type === 'markdown' && typeof content.markdown === 'string') {
      onMarkdownChangeRef.current?.(content.markdown)
      return
    }
    onOfficeDirtyRef.current?.(true)
  }

  const Editors = loaded.editors
  const artifact = loaded.status === 'editor' ? loaded.artifact : null
  const content = artifact?.content
  const embedded = frame === 'workspace'
  const surfaceClass = embedded ? 'h-full min-h-0' : undefined
  const saveMarkdown = async (markdown: string) => {
    const current = artifactRef.current
    if (!current || current.content.type !== 'markdown') return
    artifactRef.current = { ...current, content: { ...current.content, markdown } }
    await queuedPersist()
  }
  return (
    <div
      className={
        embedded ? 'relative flex h-full min-h-0 flex-1 flex-col overflow-hidden' : 'relative mt-5 min-h-[480px]'
      }>
      {exportFailed ? (
        <WorkspaceToolbar>
          <span role="alert" className="text-[#b42318]">
            {t('agent.preview_pane.edit.save_failed')}
          </span>
        </WorkspaceToolbar>
      ) : null}
      {loaded.status === 'pdf' ? (
        <Editors.PdfViewer
          className={surfaceClass}
          fileName={fileName}
          sourceKey={`${fileName}:${pdfRevision}`}
          themePreference="light"
          loadSource={loadPdf}
          onSave={
            onSave && !preview
              ? async (pdf) => {
                  const next = new Uint8Array(await pdf.arrayBuffer())
                  const previous = acceptSavedBytes(next)
                  try {
                    await onSave(next)
                  } catch (error) {
                    acceptedRef.current = previous
                    throw error
                  }
                  return true
                }
              : undefined
          }
        />
      ) : content?.type === 'document' ? (
        <Editors.DocumentEditor
          className={surfaceClass}
          content={content}
          defaultRibbonCollapsed={embedded}
          preview={preview}
          autoFocus={preview ? false : undefined}
          onChange={remember}
        />
      ) : content?.type === 'markdown' ? (
        <Editors.MarkdownEditor
          className={surfaceClass}
          content={content}
          onChange={remember}
          preview={preview}
          presentation={embedded ? 'note' : 'document'}
          onSave={onSave && !preview ? saveMarkdown : undefined}
        />
      ) : content?.type === 'spreadsheet' ? (
        <Editors.SpreadsheetEditor preview={preview} className={surfaceClass} content={content} onChange={remember} />
      ) : content?.type === 'presentation' ? (
        <Editors.PresentationEditor preview={preview} className={surfaceClass} content={content} onChange={remember} />
      ) : null}
    </div>
  )
}
