import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { Download } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { MESSAGE_FILE_ICON_CLASS } from '@renderer/components/chat/messages/frame/FileMessageCard'
import { DocumentFileIcon, documentFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import type { ImStore } from '@renderer/im/imStore'
import { formatFileSize } from '@renderer/im/imView'
import {
  fileExtension,
  isOfficePreviewKind,
  previewKind,
  type DocumentPreviewKind,
  type OfficePreviewKind
} from '@renderer/pages/documents/documentPreview'
import { readBudgetBytes } from '@renderer/pages/documents/fileOpenBudget'

export { MESSAGE_FILE_ICON_CLASS }

export type MessagePreviewPayload = {
  blob: Blob
  /** Store-owned object URL for media. The preview does not revoke it. */
  objectUrl?: string
}

type Ready = MessagePreviewPayload & { bytes: Uint8Array }

type Phase = { status: 'idle' } | { status: 'loading' } | { status: 'failed' } | ({ status: 'ready' } & Ready)

export function attachmentPreviewKind(name: string, mime?: string | null): DocumentPreviewKind {
  if (fileExtension(name)) return previewKind(name)
  if (mime?.startsWith('image/')) return 'image'
  if (mime?.startsWith('audio/')) return 'audio'
  if (mime?.startsWith('video/')) return 'video'
  if (mime === 'application/pdf') return 'pdf'
  return previewKind(name)
}

export async function loadMessageAttachment(
  store: ImStore,
  messageId: string,
  name: string,
  mime: string | null
): Promise<MessagePreviewPayload> {
  const blob = await store.attachmentFile(messageId, name, mime)
  const kind = attachmentPreviewKind(name, mime)
  if (kind !== 'audio' && kind !== 'image' && kind !== 'video') return { blob }
  return { blob, objectUrl: await store.attachmentUrl(messageId) }
}

function readBlobBytes(blob: Blob): Promise<Uint8Array> {
  const native = blob.arrayBuffer
  if (typeof native === 'function') {
    try {
      return Promise.resolve(native.call(blob)).then(
        (buffer) => new Uint8Array(buffer),
        () => readBlobWithFileReader(blob)
      )
    } catch {
      return readBlobWithFileReader(blob)
    }
  }
  return readBlobWithFileReader(blob)
}

function readBlobWithFileReader(blob: Blob): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      if (reader.result instanceof ArrayBuffer) resolve(new Uint8Array(reader.result))
      else reject(new Error('read'))
    }
    reader.onerror = () => reject(reader.error ?? new Error('read'))
    reader.readAsArrayBuffer(blob)
  })
}

function decodeUtf8(bytes: Uint8Array): string | null {
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

function saveBlob(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = name
  anchor.rel = 'noopener'
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}

function PreviewStatus({ title, description }: { title: string; description: string }) {
  return (
    <div role="status" className="grid h-full place-items-center px-8 py-10 text-center">
      <div>
        <h3 className="m-0 text-[16px] font-semibold text-[#171717]">{title}</h3>
        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">{description}</p>
      </div>
    </div>
  )
}

function OfficePreview({ name, bytes, surface }: { name: string; bytes: Uint8Array; surface: OfficePreviewKind }) {
  const { t } = useTranslation()
  const [view, setView] = useState<'loading' | 'failed' | 'ready'>('loading')
  const [Surface, setSurface] = useState<
    typeof import('@renderer/pages/documents/OfficeDocumentSurface').OfficeDocumentSurface | null
  >(null)
  useEffect(() => {
    let live = true
    void import('@renderer/pages/documents/OfficeDocumentSurface').then(
      (mod) => {
        if (!live) return
        setSurface(() => mod.OfficeDocumentSurface)
        setView('ready')
      },
      () => {
        if (live) setView('failed')
      }
    )
    return () => {
      live = false
    }
  }, [])
  if (view === 'failed') {
    return (
      <PreviewStatus
        title={t('file_preview.unavailable.title')}
        description={t('file_preview.unavailable.description')}
      />
    )
  }
  if (!Surface || view === 'loading') {
    return <p className="m-0 px-6 py-8 text-[13px] text-[#8a8a8a]">{t('file_preview.loading')}</p>
  }
  return (
    <div className="h-full min-h-0">
      <Surface surface={surface} bytes={bytes} fileName={name} frame="workspace" />
    </div>
  )
}

function PreviewBody({ name, mime, ready }: { name: string; mime?: string | null; ready: Ready }) {
  const { t } = useTranslation()
  const kind = attachmentPreviewKind(name, mime ?? ready.blob.type)
  const [localUrl, setLocalUrl] = useState<string | null>(ready.objectUrl ?? null)
  const media = kind === 'image' || kind === 'audio' || kind === 'video'
  useEffect(() => {
    if (!media || ready.objectUrl) return
    const url = URL.createObjectURL(ready.blob)
    setLocalUrl(url)
    return () => URL.revokeObjectURL(url)
  }, [media, ready.blob, ready.objectUrl])
  const url = ready.objectUrl ?? localUrl
  if (kind === 'image' && url) {
    return (
      <div className="grid h-full place-items-center p-6">
        <img alt={name} src={url} className="max-h-full max-w-full object-contain" />
      </div>
    )
  }
  if (kind === 'audio' && url) {
    return (
      <div className="grid h-full place-items-center p-6">
        <audio aria-label={name} controls src={url} className="w-full max-w-md" />
      </div>
    )
  }
  if (kind === 'video' && url) {
    return (
      <div className="grid h-full place-items-center bg-[#171717] p-4">
        <video data-ui="messages.video" aria-label={name} controls src={url} className="max-h-full max-w-full" />
      </div>
    )
  }
  if (media) return <p className="m-0 px-6 py-8 text-[13px] text-[#8a8a8a]">{t('file_preview.loading')}</p>
  if (ready.blob.size > readBudgetBytes(name)) {
    return (
      <PreviewStatus
        title={t('documents.open.too_large.title')}
        description={t('documents.open.too_large.description', { size: formatFileSize(ready.blob.size) })}
      />
    )
  }
  if (isOfficePreviewKind(kind)) {
    return <OfficePreview name={name} bytes={ready.bytes} surface={kind} />
  }
  if (kind === 'text') {
    const text = decodeUtf8(ready.bytes)
    if (text != null) {
      return (
        <pre
          data-ui="messages.file-preview-text"
          className="m-0 h-full overflow-auto p-4 text-[13px] leading-5 whitespace-pre-wrap text-[#171717]">
          {text}
        </pre>
      )
    }
  }
  return (
    <PreviewStatus
      title={t('file_preview.unsupported.title')}
      description={t('file_preview.unsupported.description')}
    />
  )
}

/** Read-only preview of a message attachment or a file still sitting in the composer. */
export function MessageFilePreview({
  open,
  name,
  mime,
  load,
  onOpenChange
}: {
  open: boolean
  name: string
  mime?: string | null
  load: () => Promise<MessagePreviewPayload>
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useTranslation()
  const loadRef = useRef(load)
  loadRef.current = load
  const [phase, setPhase] = useState<Phase>({ status: 'idle' })
  useEffect(() => {
    if (!open) return
    let live = true
    setPhase({ status: 'loading' })
    void loadRef
      .current()
      .then(async (payload) => {
        const bytes = await readBlobBytes(payload.blob)
        if (!live) return
        setPhase({ status: 'ready', blob: payload.blob, bytes, objectUrl: payload.objectUrl })
      })
      .catch(() => {
        if (live) setPhase({ status: 'failed' })
      })
    return () => {
      live = false
    }
  }, [open, name])
  const ready = phase.status === 'ready' ? phase : null
  const icon = documentFileIconType(name)
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        data-ui="messages.file-preview"
        className="flex h-[min(80vh,720px)] max-h-[calc(100vh-2rem)] w-full max-w-[min(920px,calc(100%-2rem))] flex-col gap-0 overflow-hidden bg-white p-0 sm:max-w-[920px]">
        <div className="flex items-center gap-3 border-b border-[#f0f0f0] py-3 pr-14 pl-4">
          <DocumentFileIcon type={icon} className={MESSAGE_FILE_ICON_CLASS} />
          <DialogTitle className="m-0 min-w-0 flex-1 truncate text-[15px] font-medium text-[#171717]">
            {name}
          </DialogTitle>
          <button
            type="button"
            aria-label={t('messages.download')}
            disabled={!ready}
            onClick={() => ready && saveBlob(ready.blob, name)}
            className="grid size-8 shrink-0 place-items-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] disabled:text-[#c4c4c4]">
            <Download className="size-4" strokeWidth={1.7} aria-hidden />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-hidden bg-[#f7f7f7]">
          {phase.status === 'loading' || phase.status === 'idle' ? (
            <p className="m-0 px-6 py-8 text-[13px] text-[#8a8a8a]">{t('file_preview.loading')}</p>
          ) : null}
          {phase.status === 'failed' ? (
            <PreviewStatus
              title={t('file_preview.load_error.title')}
              description={t('file_preview.load_error.description')}
            />
          ) : null}
          {ready ? <PreviewBody name={name} mime={mime} ready={ready} /> : null}
        </div>
      </DialogContent>
    </Dialog>
  )
}
