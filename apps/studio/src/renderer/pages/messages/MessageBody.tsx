import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { Download, Play } from 'lucide-react'
import { useEffect, useState, type MouseEvent as ReactMouseEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { FileMessageCard, MessageFileMark } from '@renderer/components/chat/messages/frame/FileMessageCard'
import { parseChatRecord, type ChatRecordLine } from '@renderer/im/forwardRecord'
import type { ImAttachment, ImMessage } from '@renderer/im/imApi'
import type { ImStore, PendingMessage } from '@renderer/im/imStore'
import { decodeQuotedText, type MessageQuote } from '@renderer/im/quote'
import { TaskConversationProse } from '@renderer/pages/agents/taskConversation/TaskConversationMessage'
import { ImagePreviewOverlay } from '@renderer/services/ImagePreviewService'
import { splitMentions, type MentionLabel } from '@renderer/utils/messageMentions'
import { cn } from '@renderer/utils/style'

import { loadMessageAttachment, MessageFilePreview } from './MessageFilePreview'
import { openMessageFileEditor } from './openMessageFileEditor'
import { readAgentFile } from './tobotFiles'

type Loaded = { url: string | null; failed: boolean }

/** Loads an attachment through the shell once and caches the object URL in the store. */
function useAttachmentUrl(store: ImStore, messageId: string, enabled: boolean): Loaded {
  const [state, setState] = useState<Loaded & { id: string }>({ id: messageId, url: null, failed: false })
  useEffect(() => {
    if (!enabled) return
    let live = true
    store.attachmentUrl(messageId).then(
      (url) => live && setState({ id: messageId, url, failed: false }),
      () => live && setState({ id: messageId, url: null, failed: true })
    )
    return () => {
      live = false
    }
  }, [store, messageId, enabled])
  return state.id === messageId ? state : { url: null, failed: false }
}

async function saveAttachment(store: ImStore, messageId: string, name: string) {
  const url = await store.attachmentUrl(messageId)
  const anchor = globalThis.document.createElement('a')
  anchor.href = url
  anchor.download = name
  anchor.rel = 'noopener'
  globalThis.document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
}

export { MessageFileMark }

export function FileCard({
  name,
  sizeBytes,
  onOpen,
  busy = false,
  label
}: {
  name: string
  sizeBytes: number
  onOpen?: () => void
  busy?: boolean
  label?: string
}) {
  return <FileMessageCard busy={busy} label={label} name={name} onOpen={onOpen} sizeBytes={sizeBytes} />
}

function namedFile(blob: Blob, name: string, mime?: string | null): File {
  if (blob instanceof File && blob.name) return blob
  return new File([blob], name, { type: mime || blob.type || 'application/octet-stream' })
}

function AttachmentPreviewCard({
  store,
  messageId,
  name,
  sizeBytes,
  mime
}: {
  store: ImStore
  messageId: string
  name: string
  sizeBytes: number
  mime: string | null
}) {
  const [open, setOpen] = useState(false)
  const [opening, setOpening] = useState(false)
  return (
    <>
      <FileCard
        name={name}
        sizeBytes={sizeBytes}
        busy={opening}
        onOpen={() => {
          if (opening) return
          setOpening(true)
          void loadMessageAttachment(store, messageId, name, mime)
            .then((payload) => openMessageFileEditor(namedFile(payload.blob, name, mime), messageId))
            .then((opened) => {
              if (!opened) setOpen(true)
            })
            .catch(() => setOpen(true))
            .finally(() => setOpening(false))
        }}
      />
      <MessageFilePreview
        open={open}
        name={name}
        mime={mime}
        onOpenChange={setOpen}
        load={() => loadMessageAttachment(store, messageId, name, mime)}
      />
    </>
  )
}

function ImageAttachment({
  store,
  messageId,
  attachment
}: {
  store: ImStore
  messageId: string
  attachment: ImAttachment
}) {
  const { t } = useTranslation()
  const { url, failed } = useAttachmentUrl(store, messageId, true)
  const [open, setOpen] = useState(false)
  if (failed) {
    return (
      <AttachmentPreviewCard
        store={store}
        messageId={messageId}
        name={attachment.name}
        sizeBytes={attachment.sizeBytes}
        mime={attachment.mime}
      />
    )
  }
  return (
    <>
      <button
        type="button"
        data-ui="messages.image"
        aria-label={attachment.name}
        disabled={!url}
        onClick={() => setOpen(true)}
        className="block overflow-hidden rounded-[8px] bg-[#f5f5f5] outline-none focus-visible:ring-2 focus-visible:ring-black/10">
        {url ? (
          <img alt={attachment.name} src={url} className="block max-h-60 max-w-[240px] object-contain" />
        ) : (
          <span className="block h-32 w-40 animate-pulse" />
        )}
      </button>
      {url ? (
        <ImagePreviewOverlay
          alt={attachment.name}
          onOpenChange={setOpen}
          open={open}
          src={url}
          toolbarActions={[
            {
              icon: <Download className="size-4" aria-hidden />,
              id: 'download',
              label: t('messages.download'),
              onSelect: () => saveAttachment(store, messageId, attachment.name)
            }
          ]}
        />
      ) : null}
    </>
  )
}

function VoiceAttachment({
  store,
  messageId,
  durationSecs
}: {
  store: ImStore
  messageId: string
  durationSecs: number
}) {
  const { t } = useTranslation()
  const [enabled, setEnabled] = useState(false)
  const { url, failed } = useAttachmentUrl(store, messageId, enabled)
  if (url) return <audio data-ui="messages.voice" controls autoPlay src={url} className="h-9 w-[220px]" />
  return (
    <button
      type="button"
      data-ui="messages.voice"
      onClick={() => setEnabled(true)}
      className="flex h-9 items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none hover:bg-[#ececec]">
      <Play className="size-3.5" strokeWidth={1.7} aria-hidden />
      {failed ? t('messages.attachment_failed') : `${t('messages.preview.voice')} ${durationSecs}″`}
    </button>
  )
}

export function MessageBody({
  store,
  message,
  mine,
  mentions,
  withdrawnIds,
  onQuoteOpen,
  onTextContextMenu
}: {
  store: ImStore
  message: ImMessage
  mine: boolean
  mentions?: readonly MentionLabel[]
  withdrawnIds?: ReadonlySet<string>
  onQuoteOpen?: (messageId: string) => void
  onTextContextMenu?: (event: ReactMouseEvent, text: string) => void
}) {
  const body = message.body
  if (body.kind === 'file') {
    return (
      <AttachmentPreviewCard
        store={store}
        messageId={message.id}
        name={body.attachment.name}
        sizeBytes={body.attachment.sizeBytes}
        mime={body.attachment.mime}
      />
    )
  }
  if (body.kind === 'image')
    return <ImageAttachment store={store} messageId={message.id} attachment={body.attachment} />
  if (body.kind === 'voice')
    return <VoiceAttachment store={store} messageId={message.id} durationSecs={body.durationSecs} />
  if (body.kind !== 'text') return null
  const record = parseChatRecord(body.text)
  if (record) {
    const transcript = record.lines.map((line) => `${line.sender}: ${line.text}`).join('\n')
    return (
      <ChatRecordCard
        title={record.title}
        lines={record.lines}
        onContextMenu={onTextContextMenu ? (event) => onTextContextMenu(event, transcript) : undefined}
      />
    )
  }
  const visible = decodeQuotedText(body.text).text
  return (
    <TextBubble
      text={body.text}
      mine={mine}
      mentions={mentions}
      withdrawnIds={withdrawnIds}
      onQuoteOpen={onQuoteOpen}
      onContextMenu={onTextContextMenu ? (event) => onTextContextMenu(event, visible) : undefined}
    />
  )
}

export function AgentFileView({
  name,
  mediaType,
  path,
  sizeBytes,
  previewUrl
}: {
  name: string
  mediaType: string
  path: string
  sizeBytes: number
  previewUrl?: string
}) {
  const image = mediaType.startsWith('image/')
  const [url, setUrl] = useState(previewUrl ?? '')
  const [failed, setFailed] = useState(false)
  const [open, setOpen] = useState(false)
  const [fileOpen, setFileOpen] = useState(false)
  useEffect(() => {
    if (!image || previewUrl) return
    let live = true
    let created = ''
    void readAgentFile(path).then(
      (blob) => {
        if (!live) return
        created = globalThis.URL.createObjectURL(blob)
        setUrl(created)
      },
      () => {
        if (live) setFailed(true)
      }
    )
    return () => {
      live = false
      if (created) globalThis.URL.revokeObjectURL(created)
    }
  }, [image, path, previewUrl])
  if (!image || failed || (!url && !previewUrl)) {
    if (image && !failed && !previewUrl) {
      return <span data-ui="messages.image" className="block h-32 w-40 animate-pulse rounded-[8px] bg-[#f5f5f5]" />
    }
    return (
      <>
        <FileCard
          name={name}
          sizeBytes={sizeBytes}
          onOpen={() => {
            void readAgentFile(path)
              .then((blob) => openMessageFileEditor(namedFile(blob, name, mediaType), path))
              .then((opened) => {
                if (!opened) setFileOpen(true)
              })
              .catch(() => setFileOpen(true))
          }}
        />
        <MessageFilePreview
          open={fileOpen}
          name={name}
          mime={mediaType}
          onOpenChange={setFileOpen}
          load={async () => ({ blob: await readAgentFile(path) })}
        />
      </>
    )
  }
  return (
    <>
      <button
        type="button"
        data-ui="messages.image"
        aria-label={name}
        onClick={() => setOpen(true)}
        className="block overflow-hidden rounded-[8px] bg-[#f5f5f5] outline-none focus-visible:ring-2 focus-visible:ring-black/10">
        <img alt={name} src={url || previewUrl} className="block max-h-60 max-w-[240px] object-contain" />
      </button>
      <ImagePreviewOverlay alt={name} onOpenChange={setOpen} open={open} src={url || previewUrl || ''} />
    </>
  )
}

function QuoteBlock({
  quote,
  mine,
  withdrawn,
  onOpen
}: {
  quote: MessageQuote
  mine: boolean
  withdrawn: boolean
  onOpen?: (messageId: string) => void
}) {
  const { t } = useTranslation()
  return (
    <button
      type="button"
      data-ui="messages.quote"
      data-quote-id={quote.messageId}
      onClick={(event) => {
        event.stopPropagation()
        onOpen?.(quote.messageId)
      }}
      className={cn(
        'mb-1 block w-full truncate rounded-[4px] border-l-2 px-2 py-1 text-left text-[12px] leading-4',
        mine ? 'border-white/40 bg-white/10 text-white/80' : 'border-[#d6d6d6] bg-white text-[#8a8a8a]'
      )}>
      <span className={cn('block truncate', mine ? 'text-white' : 'text-[#3f3f3f]')}>{quote.senderName}</span>
      <span className="block truncate">{withdrawn ? t('messages.quote_missing') : quote.excerpt}</span>
    </button>
  )
}

/** Assistant turn before any text arrives. Dots show motion; the live label does not tick. */
export function ThinkingIndicator() {
  const { t } = useTranslation()
  const [seconds, setSeconds] = useState(0)
  useEffect(() => {
    const started = Date.now()
    const timer = globalThis.window.setInterval(() => {
      setSeconds(Math.max(0, Math.floor((Date.now() - started) / 1000)))
    }, 1000)
    return () => globalThis.window.clearInterval(timer)
  }, [])
  const quiet = t('messages.status.thinking')
  const shown = seconds >= 3 ? t('messages.status.thinking_elapsed', { seconds }) : quiet
  return (
    <div
      data-ui="messages.thinking"
      aria-label={quiet}
      className="relative flex h-9 items-center gap-2.5 rounded-[8px] bg-[#f5f5f5] px-3">
      <span
        data-ui="messages.bubble-tail"
        data-tail="start"
        aria-hidden
        className="pointer-events-none absolute top-1/2 -left-[6px] h-0 w-0 -translate-y-1/2 border-y-[5px] border-r-[6px] border-y-transparent border-r-[#f5f5f5]"
      />
      <span className="flex items-center gap-1" aria-hidden>
        {[0, 160, 320].map((delay) => (
          <span
            key={delay}
            data-ui="messages.thinking-dot"
            className="messages-thinking-dot"
            style={{ animationDelay: `${delay}ms` }}
          />
        ))}
      </span>
      <span className="sr-only" role="status">
        {quiet}
      </span>
      <span data-ui="messages.thinking-label" aria-hidden className="text-[13px] leading-5 text-[#6b6b6b] tabular-nums">
        {shown}
      </span>
    </div>
  )
}

function ChatRecordCard({
  title,
  lines,
  onContextMenu
}: {
  title: string
  lines: readonly ChatRecordLine[]
  onContextMenu?: (event: ReactMouseEvent) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  return (
    <>
      <button
        type="button"
        data-ui="messages.chat-record"
        onClick={() => setOpen(true)}
        onContextMenu={onContextMenu}
        className="w-[240px] max-w-full rounded-[8px] border border-[#ebebeb] bg-white px-3 py-2 text-left outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
        <span className="block truncate text-[13px] font-medium text-[#171717]">{title}</span>
        {lines.slice(0, 4).map((line, index) => (
          <span key={index} className="mt-0.5 block truncate text-[12px] leading-5 text-[#8a8a8a]">
            {line.sender}: {line.text}
          </span>
        ))}
        <span className="mt-2 block border-t border-[#f0f0f0] pt-1.5 text-[11px] text-[#8a8a8a]">
          {t('messages.forward_record')}
        </span>
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent
          closeLabel={t('common.close')}
          aria-describedby={undefined}
          data-ui="messages.chat-record-view"
          className="flex max-h-[480px] flex-col gap-0 bg-white p-0 sm:max-w-[420px]">
          <div className="border-b border-[#f0f0f0] px-4 py-3 pr-12">
            <DialogTitle className="m-0 text-[15px] font-medium text-[#171717]">{title}</DialogTitle>
          </div>
          <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto px-4 py-3">
            {lines.map((line, index) => (
              <li key={index} className="py-1 text-[13px] leading-5 break-words text-[#171717]">
                <span className="text-[#8a8a8a]">{line.sender}: </span>
                {line.text}
              </li>
            ))}
          </ul>
        </DialogContent>
      </Dialog>
    </>
  )
}

export function TextBubble({
  text,
  mine,
  mentions,
  withdrawnIds,
  onQuoteOpen,
  onContextMenu
}: {
  text: string
  mine: boolean
  mentions?: readonly MentionLabel[]
  withdrawnIds?: ReadonlySet<string>
  onQuoteOpen?: (messageId: string) => void
  onContextMenu?: (event: ReactMouseEvent) => void
}) {
  const decoded = decodeQuotedText(text)
  const pieces = mentions && mentions.length > 0 ? splitMentions(decoded.text, mentions) : null
  const marked = pieces?.some((piece) => piece.kind === 'mention') ? pieces : null
  return (
    <div
      data-ui="messages.bubble"
      onContextMenu={onContextMenu}
      className={cn(
        'relative rounded-[8px] px-3 py-2 text-[13px] leading-5 break-words whitespace-pre-wrap',
        mine ? 'bg-[#171717] text-white' : 'bg-[#f5f5f5] text-[#171717]'
      )}>
      <span
        data-ui="messages.bubble-tail"
        data-tail={mine ? 'end' : 'start'}
        aria-hidden
        className={cn(
          'pointer-events-none absolute top-[13px] h-0 w-0 border-y-[5px] border-y-transparent',
          mine ? '-right-[6px] border-l-[6px] border-l-[#171717]' : '-left-[6px] border-r-[6px] border-r-[#f5f5f5]'
        )}
      />
      {decoded.quote ? (
        <QuoteBlock
          quote={decoded.quote}
          mine={mine}
          withdrawn={Boolean(withdrawnIds?.has(decoded.quote.messageId))}
          onOpen={onQuoteOpen}
        />
      ) : null}
      {marked
        ? marked.map((piece, index) =>
            piece.kind === 'text' ? (
              <span key={index}>{piece.text}</span>
            ) : (
              <span
                key={index}
                data-ui="messages.mention"
                data-mention-me={piece.me ? 'true' : 'false'}
                data-mention-target={piece.target}
                className={cn(
                  'rounded-[4px] px-0.5 font-medium',
                  mine ? 'bg-white/20 text-white' : piece.me ? 'bg-[#e8eeff] text-[#4c6fff]' : 'text-[#4c6fff]'
                )}>
                {piece.text}
              </span>
            )
          )
        : decoded.text}
    </div>
  )
}

/** ToBot assistant text, with the task conversation markdown renderer. */
export function AgentMarkdown({
  id,
  text,
  streaming,
  onContextMenu
}: {
  id: string
  text: string
  streaming: boolean
  onContextMenu?: (event: ReactMouseEvent) => void
}) {
  return (
    <div
      data-ui="messages.bubble"
      data-agent-streaming={streaming ? 'true' : 'false'}
      onContextMenu={onContextMenu}
      className="relative w-full min-w-0 flex-1 rounded-[8px] bg-[#f5f5f5] px-3 py-2 text-[13px] leading-5 break-words text-[#171717]">
      <span
        data-ui="messages.bubble-tail"
        data-tail="start"
        aria-hidden
        className="pointer-events-none absolute top-[13px] -left-[6px] h-0 w-0 border-y-[5px] border-r-[6px] border-y-transparent border-r-[#f5f5f5]"
      />
      <div className="task-conversation">
        <TaskConversationProse id={id} text={text} streaming={streaming} />
      </div>
    </div>
  )
}

export function PendingBody({
  store,
  pending,
  mentions,
  withdrawnIds,
  onQuoteOpen,
  onTextContextMenu,
  mine = true
}: {
  store: ImStore
  pending: PendingMessage
  mentions?: readonly MentionLabel[]
  withdrawnIds?: ReadonlySet<string>
  onQuoteOpen?: (messageId: string) => void
  onTextContextMenu?: (event: ReactMouseEvent, text: string) => void
  mine?: boolean
}) {
  const [open, setOpen] = useState(false)
  if (pending.kind === 'text') {
    return (
      <TextBubble
        text={pending.text}
        mine={mine}
        mentions={mentions}
        withdrawnIds={withdrawnIds}
        onQuoteOpen={onQuoteOpen}
        onContextMenu={
          onTextContextMenu ? (event) => onTextContextMenu(event, decodeQuotedText(pending.text).text) : undefined
        }
      />
    )
  }
  if (pending.kind === 'image' && pending.localUrl) {
    return (
      <div data-ui="messages.image" className="overflow-hidden rounded-[8px] opacity-80">
        <img alt={pending.name} src={pending.localUrl} className="block max-h-60 max-w-[240px] object-contain" />
      </div>
    )
  }
  const staged = store.stagedFile(pending.clientMsgId)
  return (
    <>
      <FileCard
        name={pending.name}
        sizeBytes={pending.sizeBytes}
        busy={pending.state === 'sending'}
        onOpen={
          staged
            ? () => {
                void openMessageFileEditor(staged, pending.clientMsgId).then((opened) => {
                  if (!opened) setOpen(true)
                })
              }
            : undefined
        }
      />
      {staged ? (
        <MessageFilePreview
          open={open}
          name={staged.name || pending.name}
          mime={staged.type}
          onOpenChange={setOpen}
          load={async () => ({ blob: staged })}
        />
      ) : null}
    </>
  )
}
