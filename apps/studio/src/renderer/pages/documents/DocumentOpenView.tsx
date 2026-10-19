import { SegmentedControl } from '@a3s-lab/ui'
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Textarea
} from '@a3s-lab/ui'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { HtmlPreviewFrame } from '@renderer/components/CodeBlockView/HtmlPreviewFrame'
import { ImagePreviewOverlay } from '@renderer/services/ImagePreviewService'
import { WorkspaceDocumentClient } from '@renderer/services/WorkspaceDocumentService'
import type {
  WorkspaceDiagnosticObservation,
  WorkspaceDocumentRecovery,
  WorkspaceDocumentRecoveryReview,
  WorkspaceDocumentSnapshot
} from '@shared/types/workspaceDocument'

import { isCloudWorkspace } from './cloudWorkspace'
import { decodeUtf8Async } from './decodeUtf8'
import { readDocumentBytes } from './documentBytes'
import { DocumentDiagnostics } from './DocumentDiagnostics'
import type { LibraryDocument } from './documentLibrary'
import { formatDocumentSize } from './documentLibrary'
import { fileExtension, isOfficePreviewKind, previewKind, previewMime } from './documentPreview'
import { FileEditorLoading } from './FileEditorLoading'
import { COMFORTABLE_TEXT_CHARS, readBudgetBytes, visualSourceAllowed } from './fileOpenBudget'
import { formatSourceText } from './formatSourceText'
import { monacoLanguage } from './monacoLanguage'
import { prefetchOfficeEditors } from './OfficeDocumentSurface'
import { OfficeDocumentSurface } from './OfficeDocumentSurface'
import { FileTooLargeError, writeLocalDocumentBytes } from './useLocalDocuments'
import { WorkspaceCodeEditor } from './WorkspaceCodeEditor'
import { WorkspaceNotice, WorkspaceToolbar, workspaceFieldButtonClass } from './WorkspaceFileChrome'

const logger = loggerService.withContext('DocumentOpenView')

function textOfficeExtension(extension: string): boolean {
  return extension === 'md' || extension === 'markdown' || extension === 'csv' || extension === 'tsv'
}

export function formatSavedClock(date: Date): string {
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

function decodeUtf8(bytes: Uint8Array): string | null {
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

function useObjectUrl(bytes: Uint8Array, mime: string): string | null {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    const blob = new Blob([bytes], { type: mime })
    const next = URL.createObjectURL(blob)
    setUrl(next)
    return () => URL.revokeObjectURL(next)
  }, [bytes, mime])
  return url
}

function PreviewNotice({ title, description, action }: { title: string; description: string; action?: ReactNode }) {
  return (
    <div role="status" className="mt-8">
      <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">{title}</h3>
      <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">{description}</p>
      {action}
    </div>
  )
}

function MediaPreview({
  bytes,
  mime,
  kind,
  name,
  frame
}: {
  bytes: Uint8Array
  mime: string
  kind: 'image' | 'audio' | 'video'
  name: string
  frame: 'page' | 'workspace'
}) {
  const { t } = useTranslation()
  const url = useObjectUrl(bytes, mime)
  const [open, setOpen] = useState(false)
  if (!url) return null
  if (kind === 'image') {
    const image = (
      <button
        type="button"
        aria-label={t('preview.label')}
        onClick={() => setOpen(true)}
        className={frame === 'workspace' ? 'max-h-full max-w-full cursor-zoom-in' : 'mt-5 max-w-full cursor-zoom-in'}>
        <img
          alt={name}
          src={url}
          className={
            frame === 'workspace' ? 'max-h-full max-w-full object-contain' : 'max-h-[70vh] max-w-full object-contain'
          }
        />
      </button>
    )
    const preview = <ImagePreviewOverlay alt={name} onOpenChange={setOpen} open={open} src={url} />
    if (frame === 'workspace') {
      return (
        <div className="grid h-full min-h-0 flex-1 place-items-center bg-white p-6">
          {image}
          {preview}
        </div>
      )
    }
    return (
      <>
        {image}
        {preview}
      </>
    )
  }
  if (frame === 'workspace') {
    return (
      <div className="grid h-full min-h-0 flex-1 place-items-center bg-white p-6">
        {kind === 'audio' ? (
          <audio aria-label={name} controls src={url} className="w-full max-w-md" />
        ) : (
          <video aria-label={name} controls src={url} className="max-h-full max-w-full" />
        )}
      </div>
    )
  }
  if (kind === 'audio') {
    return <audio aria-label={name} controls src={url} className="mt-5 w-full" />
  }
  return <video aria-label={name} controls src={url} className="mt-5 max-h-[70vh] w-full" />
}

export function DocumentOpenView({
  document,
  workspaceRoot,
  cloudReady,
  frame = 'page',
  readOnly = false,
  onOpenOutside,
  onDirtyChange,
  onSavedAtChange
}: {
  document: LibraryDocument
  workspaceRoot: string
  cloudReady: boolean
  /** `page` is the documents library. `workspace` fills a task-workspace tab. */
  frame?: 'page' | 'workspace'
  /** Chat attachments use the same editor and cannot write the copy. */
  readOnly?: boolean
  onOpenOutside?: () => void
  onDirtyChange?: (dirty: boolean) => void
  /** When set, the host draws the saved-time hint and this view does not. */
  onSavedAtChange?: (at: Date) => void
}) {
  const { t } = useTranslation()
  const [phase, setPhase] = useState<'loading' | 'ready' | 'unavailable' | 'too-large'>('loading')
  const [oversized, setOversized] = useState(0)
  const [bytes, setBytes] = useState<Uint8Array | null>(null)
  const [text, setText] = useState('')
  const [savedText, setSavedText] = useState('')
  const [textBroken, setTextBroken] = useState(false)
  const [surfaceMode, setSurfaceMode] = useState<'preview' | 'source'>('source')
  const [previewBytes, setPreviewBytes] = useState<Uint8Array | null>(null)
  const [officeDirty, setOfficeDirty] = useState(false)
  const [textDirty, setTextDirty] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [recoveries, setRecoveries] = useState<readonly WorkspaceDocumentRecovery[]>([])
  const [recoveryReview, setRecoveryReview] = useState<WorkspaceDocumentRecoveryReview | null>(null)
  const [resolvingRecovery, setResolvingRecovery] = useState(false)
  const [recoveryError, setRecoveryError] = useState(false)
  const [diagnostics, setDiagnostics] = useState<WorkspaceDiagnosticObservation | null>(null)
  const [diagnosticsUnavailable, setDiagnosticsUnavailable] = useState(false)
  const [savedAt, setSavedAt] = useState<Date | null>(null)
  const [loadProgress, setLoadProgress] = useState<number | null>(null)
  const kind = previewKind(document.name)
  const extension = fileExtension(document.name)
  const htmlFile = extension === 'html' || extension === 'htm' || extension === 'xhtml' || extension === 'shtml'
  const svgFile = extension === 'svg'
  const textOffice = textOfficeExtension(extension)
  const canSave = !readOnly && document.place === 'local' && Boolean(workspaceRoot) && Boolean(document.locationPath)
  const { id, name, place, locationPath } = document
  const markdownFile = extension === 'md' || extension === 'markdown'
  const knownBytes = document.sizeBytes ?? 0
  const heavyText = text.length > COMFORTABLE_TEXT_CHARS
  const visualOk = bytes != null && visualSourceAllowed(extension, bytes.byteLength)
  const visualRef = useRef('')
  const visualFresh = useRef(false)
  const savedTextRef = useRef('')
  const latestTextRef = useRef('')
  const sourceReader = useRef<(() => string) | null>(null)
  const fileGeneration = useRef(0)
  const diskRevision = useRef<string | null>(null)
  const nativeDocument = useRef<WorkspaceDocumentClient | null>(null)
  const reviewGeneration = useRef(0)
  const saveTail = useRef<Promise<unknown>>(Promise.resolve())

  useEffect(() => {
    const ticket = ++fileGeneration.current
    diskRevision.current = null
    nativeDocument.current = null
    let ownedDocument: WorkspaceDocumentClient | null = null
    let diagnosticsDisabled = false
    let observationTimer: ReturnType<typeof globalThis.setTimeout> | undefined
    const retire = (client: WorkspaceDocumentClient) => {
      void client
        .close()
        .catch((error: unknown) =>
          logger.error('Native document close incomplete', error instanceof Error ? error : new Error(String(error)))
        )
    }
    let cancelled = false
    setPhase('loading')
    setBytes(null)
    setTextBroken(false)
    setSurfaceMode(fileExtension(name) === 'svg' || textOfficeExtension(fileExtension(name)) ? 'preview' : 'source')
    setPreviewBytes(null)
    setOfficeDirty(false)
    setTextDirty(false)
    visualFresh.current = false
    setNotice(null)
    setRecoveries([])
    setRecoveryReview(null)
    setResolvingRecovery(false)
    setRecoveryError(false)
    setDiagnostics(null)
    setDiagnosticsUnavailable(false)
    reviewGeneration.current++
    setSavedAt(null)
    setLoadProgress(null)
    setOversized(0)
    if (isOfficePreviewKind(previewKind(name)) && knownBytes <= readBudgetBytes(name)) prefetchOfficeEditors()
    void (async () => {
      try {
        const next = await readDocumentBytes(
          { id, place, locationPath },
          workspaceRoot,
          cloudReady,
          (ratio) => {
            if (!cancelled && ticket === fileGeneration.current) setLoadProgress(ratio)
          },
          readBudgetBytes(name),
          (revision) => {
            if (!cancelled && ticket === fileGeneration.current) diskRevision.current = revision
          }
        )
        if (cancelled || ticket !== fileGeneration.current) return
        const opened = fileExtension(name)
        if (!visualSourceAllowed(opened, next.byteLength)) setSurfaceMode('source')
        setBytes(next)
        if (previewKind(name) === 'text' || opened === 'svg' || textOfficeExtension(opened)) {
          const decoded = await decodeUtf8Async(next)
          if (cancelled || ticket !== fileGeneration.current) return
          if (decoded === null) {
            setTextBroken(true)
            visualRef.current = ''
            savedTextRef.current = ''
            latestTextRef.current = ''
          } else {
            let draft = decoded
            if (!readOnly && place === 'local' && !isCloudWorkspace(workspaceRoot)) {
              const client = await WorkspaceDocumentClient.open(workspaceRoot, locationPath)
              if (cancelled || ticket !== fileGeneration.current) {
                retire(client)
                return
              }
              ownedDocument = client
              nativeDocument.current = client
              draft = client.observation.text
              setTextDirty(client.observation.dirty)
              setBytes(new globalThis.TextEncoder().encode(draft))
              const pending = await client.recoveries()
              if (cancelled || ticket !== fileGeneration.current) return
              setRecoveries(pending)
              const observe = async () => {
                if (cancelled || ticket !== fileGeneration.current) return
                try {
                  const changed = await client.observe()
                  if (cancelled || ticket !== fileGeneration.current) return
                  const pending = await client.recoveries()
                  if (cancelled || ticket !== fileGeneration.current) return
                  setRecoveries(pending)
                  if (changed && client.observation === changed && client.text === changed.text) {
                    setDiagnostics(null)
                    latestTextRef.current = changed.text
                    visualRef.current = changed.text
                    visualFresh.current = false
                    setText(changed.text)
                    setBytes(new globalThis.TextEncoder().encode(changed.text))
                    setTextDirty(changed.dirty)
                    if (changed.diskConflict) setNotice(t('agent.preview_pane.edit.save_failed'))
                    if (!changed.dirty) {
                      savedTextRef.current = changed.text
                      setSavedText(changed.text)
                    }
                  }
                  if (!diagnosticsDisabled) {
                    try {
                      const observed = await client.diagnostics()
                      if (!cancelled && ticket === fileGeneration.current) setDiagnostics(observed ?? null)
                    } catch (error) {
                      if (!cancelled && ticket === fileGeneration.current) {
                        diagnosticsDisabled = true
                        setDiagnostics(null)
                        setDiagnosticsUnavailable(true)
                        logger.error(
                          'Native document diagnostics disabled',
                          error instanceof Error ? error : new Error(String(error))
                        )
                      }
                    }
                  }
                } catch {
                  if (!cancelled && ticket === fileGeneration.current) {
                    setNotice(t('agent.preview_pane.edit.save_failed'))
                    try {
                      const pending = await client.recoveries()
                      if (!cancelled && ticket === fileGeneration.current) setRecoveries(pending)
                    } catch {
                      /* Keep the existing failure visible until explicit recovery. */
                    }
                  }
                } finally {
                  if (!cancelled && ticket === fileGeneration.current)
                    observationTimer = globalThis.setTimeout(() => {
                      void observe()
                    }, 500)
                }
              }
              observationTimer = globalThis.setTimeout(() => {
                void observe()
              }, 500)
            }
            visualRef.current = draft
            savedTextRef.current = decoded
            latestTextRef.current = draft
            setText(draft)
            setSavedText(decoded)
          }
        }
        setPhase('ready')
      } catch (error) {
        if (cancelled || ticket !== fileGeneration.current) return
        if (error instanceof FileTooLargeError) {
          setOversized(error.bytes)
          setPhase('too-large')
          return
        }
        setPhase('unavailable')
      }
    })()
    return () => {
      cancelled = true
      if (ticket === fileGeneration.current) fileGeneration.current += 1
      if (nativeDocument.current === ownedDocument) nativeDocument.current = null
      if (observationTimer !== undefined) globalThis.clearTimeout(observationTimer)
      if (ownedDocument) retire(ownedDocument)
    }
  }, [cloudReady, id, knownBytes, locationPath, name, place, readOnly, t, workspaceRoot])

  const noteMarkdown = (markdown: string) => {
    publishDraft(markdown)
    visualRef.current = markdown
    latestTextRef.current = markdown
    visualFresh.current = true
    setTextDirty(markdown !== savedTextRef.current)
  }

  const editText = (value: string) => {
    publishDraft(value)
    latestTextRef.current = value
    visualFresh.current = false
    setText(value)
  }

  const publishDraft = (value: string) => {
    setDiagnostics(null)
    const client = nativeDocument.current
    if (!client || client.text === value) return
    const ticket = fileGeneration.current
    void client.replace(value).catch(async () => {
      if (ticket !== fileGeneration.current) return
      setNotice(t('agent.preview_pane.edit.save_failed'))
      try {
        const pending = await client.recoveries()
        if (ticket === fileGeneration.current) setRecoveries(pending)
      } catch {
        /* The failed admission remains visible. */
      }
    })
  }

  const saveText = (next = sourceReader.current?.() ?? latestTextRef.current) => {
    if (!canSave) return
    const root = workspaceRoot
    const path = document.locationPath
    const ticket = fileGeneration.current
    const client = nativeDocument.current
    if (next === savedTextRef.current && !client?.observation.dirty) return
    if (client && client.text !== next) publishDraft(next)
    const nativeSave = client?.save().then(
      (value) => ({ value }),
      (error: unknown) => ({ error })
    )
    const run = saveTail.current.then(async () => {
      if (ticket !== fileGeneration.current) return
      if (!client && next === savedTextRef.current) return
      const encoded = new TextEncoder().encode(next)
      try {
        if (nativeSave) {
          const result = await nativeSave
          if ('error' in result) throw result.error
          if (result.value.text !== next) throw new Error('Native saved content changed')
        } else {
          if (!diskRevision.current) throw new Error('document snapshot unavailable')
          const revision = await writeLocalDocumentBytes(root, path, encoded, diskRevision.current)
          if (ticket === fileGeneration.current) diskRevision.current = revision
        }
      } catch {
        if (ticket === fileGeneration.current) setNotice(t('agent.preview_pane.edit.save_failed'))
        return
      }
      if (ticket !== fileGeneration.current) return
      setBytes(encoded)
      setPreviewBytes(null)
      savedTextRef.current = next
      const heavy = next.length > COMFORTABLE_TEXT_CHARS
      if (!heavy) setSavedText(next)
      setNotice(null)
      const at = new Date()
      setSavedAt(at)
      onSavedAtChange?.(at)
      const current = sourceReader.current?.() ?? latestTextRef.current
      if (current !== next) {
        setTextDirty(true)
        return
      }
      latestTextRef.current = next
      visualRef.current = next
      visualFresh.current = false
      setTextDirty(false)
      setOfficeDirty(false)
    })
    saveTail.current = run.then(
      () => undefined,
      () => undefined
    )
  }

  const saveTextRef = useRef(saveText)
  saveTextRef.current = saveText
  const textSurface =
    kind === 'text' ||
    (svgFile && surfaceMode === 'source' && !textBroken) ||
    (textOffice && surfaceMode === 'source' && !textBroken)
  useEffect(() => {
    if (!canSave || frame !== 'workspace' || !textSurface) return undefined
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
      if (event.key.toLowerCase() !== 's') return
      event.preventDefault()
      void saveTextRef.current()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [canSave, frame, textSurface])

  useEffect(() => {
    const edited = text.length > COMFORTABLE_TEXT_CHARS ? false : text !== savedText
    onDirtyChange?.(edited || textDirty || officeDirty)
  }, [officeDirty, onDirtyChange, savedText, text, textDirty])

  const showSource = (mode: 'preview' | 'source') => {
    if (mode === 'source' && markdownFile && visualFresh.current) {
      latestTextRef.current = visualRef.current
      setText(visualRef.current)
      visualFresh.current = false
    }
    if (mode === 'preview' && textOffice && !textBroken && text !== savedTextRef.current) {
      setPreviewBytes(new TextEncoder().encode(text))
    } else if (mode === 'preview') {
      setPreviewBytes(null)
    }
    setSurfaceMode(mode)
  }

  const modeSwitch =
    markdownFile && visualOk && !textBroken ? (
      <SegmentedControl<'preview' | 'source'>
        aria-label={t('file_preview.markdown.mode.label')}
        size="sm"
        value={surfaceMode}
        options={[
          { value: 'preview', label: t('file_preview.markdown.mode.preview') },
          { value: 'source', label: t('file_preview.markdown.mode.source') }
        ]}
        onValueChange={showSource}
      />
    ) : null

  const savedHint =
    savedAt && !onSavedAtChange ? (
      <time
        dateTime={savedAt.toISOString()}
        className="shrink-0 pr-1 text-[12px] leading-none text-[#8a8a8a] tabular-nums">
        {t('knowledge.notes.saved_at', { time: formatSavedClock(savedAt) })}
      </time>
    ) : null

  const dirty = !readOnly && (officeDirty || textDirty || (!heavyText && text !== savedText))
  const outsideLabel = t('file_preview.unsupported.action')
  const showOutside = Boolean(onOpenOutside) && (kind === 'image' || kind === 'audio' || kind === 'video')
  const fileToolbar =
    frame === 'workspace' && (modeSwitch || dirty || savedHint || showOutside) ? (
      <WorkspaceToolbar>
        {modeSwitch}
        <div className="ml-auto flex min-w-0 items-center gap-2">
          {dirty ? (
            <span data-workspace-dirty className="shrink-0 text-[13px] leading-none text-[#171717]">
              {t('agent.preview_pane.edit.unsaved')}
            </span>
          ) : null}
          {savedHint}
          {showOutside ? (
            <button type="button" onClick={onOpenOutside} className={workspaceFieldButtonClass}>
              {outsideLabel}
            </button>
          ) : null}
        </div>
      </WorkspaceToolbar>
    ) : null

  const outsideAction = (show: boolean) => {
    if (!show || !onOpenOutside) return undefined
    if (frame === 'workspace') {
      return (
        <button type="button" onClick={onOpenOutside} className={`${workspaceFieldButtonClass} mt-4`}>
          {outsideLabel}
        </button>
      )
    }
    return (
      <button
        type="button"
        onClick={onOpenOutside}
        className="mt-4 h-[30px] rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717]">
        {outsideLabel}
      </button>
    )
  }

  const saveOffice = (next: Uint8Array) => {
    if (!canSave) return Promise.reject(new Error('write failed'))
    const root = workspaceRoot
    const path = document.locationPath
    const officeText = textOffice
    const ticket = fileGeneration.current
    const client = nativeDocument.current
    let nativeSave: Promise<{ value: unknown } | { error: unknown }> | undefined
    if (client && officeText) {
      const decoded = decodeUtf8(next)
      if (decoded === null) return Promise.reject(new Error('Invalid text document'))
      publishDraft(decoded)
      nativeSave = client.save().then(
        (value) => ({ value }),
        (error: unknown) => ({ error })
      )
    }
    const run = saveTail.current.then(async () => {
      if (ticket !== fileGeneration.current) throw new Error('document view changed')
      try {
        if (nativeSave) {
          const result = await nativeSave
          if ('error' in result) throw result.error
        } else {
          if (!diskRevision.current) throw new Error('document snapshot unavailable')
          const revision = await writeLocalDocumentBytes(root, path, next, diskRevision.current)
          if (ticket === fileGeneration.current) diskRevision.current = revision
        }
      } catch {
        if (ticket === fileGeneration.current) setNotice(t('agent.preview_pane.edit.save_failed'))
        throw new Error('write failed')
      }
      if (ticket !== fileGeneration.current) return
      setBytes(next)
      setPreviewBytes(null)
      setNotice(null)
      const at = new Date()
      setSavedAt(at)
      onSavedAtChange?.(at)
      if (!officeText) return
      const decoded = decodeUtf8(next)
      if (decoded === null) return
      savedTextRef.current = decoded
      setSavedText(decoded)
      if (latestTextRef.current !== decoded) {
        setTextDirty(true)
        return
      }
      visualRef.current = decoded
      visualFresh.current = false
      setText(decoded)
      setTextDirty(false)
    })
    saveTail.current = run.then(
      () => undefined,
      () => undefined
    )
    return run
  }

  const formatDraft = () => {
    try {
      editText(formatSourceText(text, monacoLanguage(document.name)))
      setNotice(null)
    } catch {
      setNotice(t('code.cli_config.format_failed'))
    }
  }

  const displayObservation = (client: WorkspaceDocumentClient, ticket: number, next: WorkspaceDocumentSnapshot) => {
    if (
      ticket !== fileGeneration.current ||
      nativeDocument.current !== client ||
      client.observation !== next ||
      client.text !== next.text
    )
      return
    latestTextRef.current = next.text
    visualRef.current = next.text
    visualFresh.current = false
    savedTextRef.current = next.dirty ? savedTextRef.current : next.text
    setText(next.text)
    if (!next.dirty) setSavedText(next.text)
    setBytes(new globalThis.TextEncoder().encode(next.text))
    setPreviewBytes(null)
    setTextDirty(next.dirty)
    setOfficeDirty(false)
    setNotice(null)
  }

  const discardDraft = async () => {
    const client = nativeDocument.current
    const ticket = fileGeneration.current
    if (!client) return
    try {
      displayObservation(client, ticket, await client.discard())
      const pending = await client.recoveries()
      if (ticket === fileGeneration.current) setRecoveries(pending)
    } catch {
      if (ticket === fileGeneration.current) setNotice(t('agent.preview_pane.edit.refresh_failed'))
    }
  }

  const reviewDraft = async (id: string) => {
    const client = nativeDocument.current
    const ticket = fileGeneration.current
    if (!client) return
    const reviewTicket = ++reviewGeneration.current
    setRecoveryError(false)
    try {
      const review = await client.reviewRecovery(id)
      if (
        ticket === fileGeneration.current &&
        nativeDocument.current === client &&
        reviewTicket === reviewGeneration.current
      )
        setRecoveryReview(review)
    } catch {
      if (ticket === fileGeneration.current) setNotice(t('agent.preview_pane.edit.refresh_failed'))
    }
  }

  const resolveDraft = async (restore: boolean) => {
    const client = nativeDocument.current
    const review = recoveryReview
    const ticket = fileGeneration.current
    if (!client || !review || resolvingRecovery) return
    setResolvingRecovery(true)
    setRecoveryError(false)
    try {
      const next = await client.resolveRecovery(review, restore)
      if (ticket !== fileGeneration.current || nativeDocument.current !== client) return
      displayObservation(client, ticket, next)
      setRecoveryReview(null)
      const pending = await client.recoveries()
      if (ticket === fileGeneration.current) setRecoveries(pending)
    } catch {
      if (ticket === fileGeneration.current) {
        setNotice(t('agent.preview_pane.edit.refresh_failed'))
        setRecoveryError(true)
      }
    } finally {
      if (ticket === fileGeneration.current) setResolvingRecovery(false)
    }
  }

  const noticeLine = (
    <>
      {notice ? (
        <div
          role="status"
          className="flex items-center gap-2 border-b border-[#ececec] px-3 py-1.5 text-[12px] text-[#8a8a8a]">
          <span>{notice}</span>
          {nativeDocument.current ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => {
                void discardDraft()
              }}>
              {t('agent.preview_pane.edit.discard')}
            </Button>
          ) : null}
        </div>
      ) : null}
      {recoveries.length ? (
        <div
          role="status"
          className="flex flex-wrap items-center gap-2 border-b border-[#ececec] px-3 py-1.5 text-[12px] text-[#8a8a8a]">
          <span>{t('agent.preview_pane.edit.recovery.available')}</span>
          {recoveries.map((draft, index) => (
            <Button
              key={draft.id}
              type="button"
              size="sm"
              variant="outline"
              onClick={() => {
                void reviewDraft(draft.id)
              }}>
              {t('agent.preview_pane.edit.recovery.review', { number: index + 1 })}
            </Button>
          ))}
        </div>
      ) : null}
      <Dialog
        open={recoveryReview !== null}
        onOpenChange={(open) => {
          if (!open && !resolvingRecovery) {
            reviewGeneration.current++
            setRecoveryReview(null)
          }
        }}>
        <DialogContent size="xl" closeLabel={t('common.cancel')}>
          <DialogHeader>
            <DialogTitle>{t('agent.preview_pane.edit.recovery.title')}</DialogTitle>
            <DialogDescription>{t('agent.preview_pane.edit.recovery.description')}</DialogDescription>
          </DialogHeader>
          {recoveryReview ? (
            <div className="grid gap-3">
              <label>
                {t('agent.preview_pane.edit.recovery.current')}
                <Textarea.Input
                  readOnly
                  value={recoveryReview.snapshot.text}
                  className="h-32 overflow-auto font-mono"
                />
              </label>
              <label>
                {t('agent.preview_pane.edit.recovery.draft')}
                <Textarea.Input readOnly value={recoveryReview.text} className="h-32 overflow-auto font-mono" />
              </label>
            </div>
          ) : null}
          {recoveryError || (recoveryReview && !recoveryReview.recovery.canResolve) ? (
            <div role="status" className="flex items-center gap-2 text-[13px] text-[#8a8a8a]">
              <span>
                {t(
                  recoveryError ? 'agent.preview_pane.edit.recovery.changed' : 'agent.preview_pane.edit.recovery.owned'
                )}
              </span>
              <Button
                type="button"
                variant="outline"
                disabled={resolvingRecovery}
                onClick={() => {
                  if (recoveryReview) void reviewDraft(recoveryReview.recovery.id)
                }}>
                {t('agent.preview_pane.edit.conflict.reload')}
              </Button>
            </div>
          ) : null}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={resolvingRecovery || !recoveryReview?.recovery.canResolve}
              onClick={() => {
                void resolveDraft(false)
              }}>
              {t('common.delete')}
            </Button>
            <Button
              type="button"
              disabled={resolvingRecovery || !recoveryReview?.recovery.canResolve}
              onClick={() => {
                void resolveDraft(true)
              }}>
              {t('documents.restore')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )

  const diagnosticPanel = (
    <DocumentDiagnostics
      observation={
        diagnostics &&
        diagnostics.snapshot.documentId === nativeDocument.current?.observation.documentId &&
        diagnostics.snapshot.version === nativeDocument.current?.observation.version &&
        diagnostics.snapshot.text === text
          ? diagnostics
          : null
      }
      unavailable={diagnosticsUnavailable}
    />
  )

  const sourceEditor = (
    <>
      <WorkspaceCodeEditor
        fileName={document.name}
        value={text}
        placeholder={t('documents.preview.placeholder')}
        readOnly={readOnly}
        onChange={readOnly ? () => undefined : editText}
        onDirty={
          readOnly
            ? undefined
            : () => {
                setTextDirty(true)
                const value = sourceReader.current?.()
                if (value !== undefined) publishDraft(value)
              }
        }
        onSave={(next) => {
          if (!readOnly) void saveText(next)
        }}
        onFormat={readOnly || heavyText ? undefined : formatDraft}
        statusExtra={frame === 'workspace' ? undefined : savedHint}
        valueReader={sourceReader}
      />
      {diagnosticPanel}
    </>
  )

  const blocked = (title: string, description: string, action?: ReactNode) => {
    if (frame === 'workspace') return <WorkspaceNotice title={title} description={description} action={action} />
    return <PreviewNotice title={title} description={description} action={action} />
  }
  if (phase === 'loading') {
    if (frame === 'workspace') {
      return (
        <WorkspaceNotice
          title={t('file_preview.loading')}
          description={t('file_preview.loading.description')}
          progress={loadProgress}
        />
      )
    }
    return <FileEditorLoading label={t('file_preview.loading')} progress={loadProgress} />
  }
  if (phase === 'too-large') {
    return blocked(
      t('documents.open.too_large.title'),
      t('documents.open.too_large.description', { size: formatDocumentSize(oversized) }),
      outsideAction(true)
    )
  }
  if (phase === 'unavailable' || bytes === null || (kind === 'text' && textBroken)) {
    return blocked(
      t('file_preview.unavailable.title'),
      t('file_preview.unavailable.description'),
      outsideAction(frame === 'workspace')
    )
  }
  if (kind === 'unsupported') {
    return blocked(t('file_preview.unsupported.title'), t('file_preview.unsupported.description'), outsideAction(true))
  }
  if (kind === 'text') {
    if (frame === 'workspace') {
      return (
        <div
          className="flex h-full min-h-0 flex-1 flex-col bg-white"
          data-workspace-text-editor={document.locationPath}>
          {fileToolbar}
          {noticeLine}
          {htmlFile && visualOk && surfaceMode === 'preview' ? (
            <HtmlPreviewFrame html={text} title={document.name} />
          ) : (
            sourceEditor
          )}
        </div>
      )
    }
    return (
      <div className="mt-5">
        {savedHint ? <div className="mb-3 flex justify-end">{savedHint}</div> : null}
        <textarea
          aria-label={document.name}
          value={text}
          readOnly={readOnly}
          onChange={(event) => {
            if (!readOnly) editText(event.target.value)
          }}
          onKeyDown={(event) => {
            if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
            if (event.key.toLowerCase() !== 's') return
            event.preventDefault()
            void saveText()
          }}
          placeholder={t('documents.preview.placeholder')}
          className="min-h-[420px] w-full resize-none rounded-[10px] border border-[#ebebeb] bg-white p-4 text-[15px] text-[#171717] outline-none placeholder:text-[#b0b0b0] focus:border-[#171717]"
        />
        {diagnosticPanel}
      </div>
    )
  }
  if (kind === 'image' || kind === 'audio' || kind === 'video') {
    const svgSource = svgFile && surfaceMode === 'source' && !textBroken
    return (
      <div className={frame === 'workspace' ? 'flex h-full min-h-0 flex-1 flex-col bg-white' : undefined}>
        {fileToolbar}
        {svgSource ? noticeLine : null}
        {svgSource ? (
          sourceEditor
        ) : (
          <MediaPreview
            bytes={bytes}
            mime={previewMime(document.name) ?? 'application/octet-stream'}
            kind={kind}
            name={document.name}
            frame={frame}
          />
        )}
      </div>
    )
  }
  if (isOfficePreviewKind(kind)) {
    const sourceReady = textOffice && !textBroken && frame === 'workspace'
    if (sourceReady && surfaceMode === 'source') {
      return (
        <div
          className="flex h-full min-h-0 flex-1 flex-col bg-white"
          data-workspace-text-editor={document.locationPath}>
          {fileToolbar}
          {noticeLine}
          {sourceEditor}
        </div>
      )
    }
    return (
      <div className={frame === 'workspace' ? 'relative flex h-full min-h-0 flex-1 flex-col bg-white' : 'relative'}>
        {fileToolbar}
        {noticeLine}
        {frame !== 'workspace' && savedHint ? (
          <div className="pointer-events-none absolute top-2.5 right-12 z-10">{savedHint}</div>
        ) : null}
        <div
          className={frame === 'workspace' ? 'flex h-full min-h-0 flex-1 flex-col' : 'relative'}
          data-workspace-preview={frame === 'workspace' ? '' : undefined}>
          <OfficeDocumentSurface
            surface={kind}
            bytes={previewBytes ?? bytes}
            fileName={document.name}
            frame={frame}
            preview={readOnly}
            onSave={canSave ? saveOffice : undefined}
            onFail={sourceReady ? () => setSurfaceMode('source') : undefined}
            onMarkdownChange={readOnly || !markdownFile ? undefined : noteMarkdown}
            onOfficeDirty={readOnly ? undefined : setOfficeDirty}
          />
        </div>
      </div>
    )
  }
  return blocked(t('file_preview.unsupported.title'), t('file_preview.unsupported.description'))
}
