import { Button, ConfirmDialog, EmptyState, SearchInput } from '@a3s-lab/ui'
import {
  Bell,
  ChevronRight,
  BellOff,
  Building2,
  CloudOff,
  Copy,
  Ellipsis,
  Folder,
  Forward,
  ImageIcon,
  Layers,
  LogIn,
  Mail,
  MessagesSquare,
  Network,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Quote,
  RotateCcw,
  Search,
  SearchX,
  Smile,
  Trash2,
  Undo2,
  Users,
  X
} from 'lucide-react'
import {
  type ClipboardEvent as ReactClipboardEvent,
  type DragEvent as ReactDragEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  memo,
  useMemo,
  useRef,
  useState
} from 'react'
import { useTranslation } from 'react-i18next'

import { dataApiService } from '@data/DataApiService'
import { publishMemberProfile, useMemberProfile } from '@renderer/components/account/memberProfile'
import {
  consumePendingMessage,
  OPEN_MESSAGE_EVENT,
  setMessageFocus
} from '@renderer/components/app/messageAttentionHost'
import { AgentFormSubmitContext, type AgentFormSubmit } from '@renderer/components/chat/form/agentFormSubmit'
import { ComposerFileDropOverlay, dragEventCarriesFiles } from '@renderer/components/composer/ComposerFileDropOverlay'
import { ComposerScreenshotControl } from '@renderer/components/composer/screenshot/ComposerScreenshotControl'
import Scrollbar from '@renderer/components/Scrollbar'
import { logoutMemberAccount, readMemberAccount, returnToLogin } from '@renderer/data/memberOrgs'
import { submitAgentForm } from '@renderer/data/studioAgentPlane'
import useAvatar from '@renderer/hooks/useAvatar'
import { encodeChatRecord, parseChatRecord } from '@renderer/im/forwardRecord'
import { IM_ATTACHMENT_MAX_BYTES, type ImInboxEntry, type ImMessage } from '@renderer/im/imApi'
import {
  BUILTIN_TOBOSS_ID,
  BUILTIN_TOBOT_ID,
  ImActionError,
  type ImSnapshot,
  type PendingMessage
} from '@renderer/im/imStore'
import {
  canRecall,
  conversationTitle,
  formatConversationTime,
  personName,
  previewText,
  readState,
  recallNotice,
  showTimeDivider,
  systemNotice,
  type Translate
} from '@renderer/im/imView'
import {
  decodeBossWire,
  encodeBossWire,
  encodeQuotedText,
  makeQuote,
  visibleText,
  type MessageQuote
} from '@renderer/im/quote'
import {
  TOBOSS_PROFILE_KEY,
  TOBOT_PROFILE_KEY,
  tobotDisplayName,
  tobotHostContext,
  useBotProfile,
  useToBotProfile,
  type ToBotHostContext
} from '@renderer/im/tobotProfile'
import { useIm } from '@renderer/im/useIm'
import { ipcApi } from '@renderer/ipc'
import { PAGE_SIDEBAR_DEFAULT_WIDTH, pageSidebarEdgeClassName } from '@renderer/layout/pageSidebar'
import { mailTurnNote } from '@renderer/mail/mailApi'
import { TaskConversationTurn } from '@renderer/pages/agents/taskConversation/TaskConversationMessage'
import { useConversationComposer } from '@renderer/pages/messages/composerDrafts'
import { listOrgMembers } from '@renderer/pages/org/orgApi'
import {
  deliverToBotReply,
  prepareToBotDataTurn,
  type DataChannelId
} from '@renderer/pages/settings/DataSettings/dataLink'
import {
  acceptBossCatchUp,
  BOSS_ROOM_DIRECTIVE,
  BOSS_WARM_CAP,
  bossRoomNeedsWarm,
  bossRoomPrompt,
  bossTranscript,
  finishSharedBossJob,
  isBossRoomTurnAnchor,
  noteBossSession,
  offerSharedBossJob,
  omitBossRoomTurns,
  readBossGroups,
  bossRoomSessionId,
  registerBossJobRunner,
  teamBotMentionLabels,
  writeBossGroups,
  type BossJob,
  type BossRoomMessage
} from '@renderer/services/bossRoom'
import { withHarnessComponents } from '@renderer/services/harnessAssemble'
import { ImagePreviewOverlay } from '@renderer/services/ImagePreviewService'
import {
  applyTobotChunk,
  approvalFromChunk,
  approvalFromLines,
  bindTobotSession,
  currentTobotSession,
  livePendingId,
  noteTobotTool,
  overlayAgentLines,
  overlayPendingReply,
  sessionIdFromEnsure,
  subscribeTobotStream,
  tobotTopic,
  type TobotApproval,
  type TobotLive,
  type TobotPart
} from '@renderer/services/tobotStream'
import { studioProductLogo, studioProductMark } from '@renderer/tenantBrand'
import {
  applyMention,
  entryMentionsYou,
  filterMentions,
  highlightLabels,
  mentionBeforeCaret,
  mentionQuery,
  textMentionsBot,
  textMentionsNamedBot
} from '@renderer/utils/messageMentions'
import { cn } from '@renderer/utils/style'
import { workTurnNote } from '@renderer/workInbox/workToBot'
import type { CherryMessagePart } from '@shared/data/types/message'

import { ForwardDialog, type ForwardChoice, type ForwardChat } from './ForwardDialog'
import { toHistoryRecord, type HistoryRecord } from './history'
import { HistoryDialog } from './HistoryDialog'
import { MentionPicker, type MentionOption } from './MentionPicker'
import { AccountAvatar, GroupAvatar, PersonAvatar, type GroupFace } from './messageAvatars'
import {
  AgentFileView,
  AgentMarkdown,
  MessageBody,
  MessageFileMark,
  PendingBody,
  TextBubble,
  ThinkingIndicator
} from './MessageBody'
import { MessageEmojiPanel } from './MessageEmojiPanel'
import { MESSAGE_FILE_ICON_CLASS, MessageFilePreview } from './MessageFilePreview'
import {
  mergeCachedPortraits,
  readCachedPortraits,
  readCachedSelfPortrait,
  writeCachedPortraits
} from './messagePortraitCache'
import { ConversationListSkeleton, ThreadStart } from './MessagesEmpty'
import { openMessageFileEditor } from './openMessageFileEditor'
import { OrgDialog, PeoplePickerDialog } from './PeopleDialogs'
import { ToBossMark } from './ToBossMark'
import { tobotAccessNote } from './tobotAccess'
import {
  pathFromFileUrl,
  previewFor,
  readAgentFile,
  rememberPreview,
  uploadTobotFile,
  type TobotFilePart
} from './tobotFiles'
import { ToBotMark } from './ToBotMark'
import { ToBotProfileDialog } from './ToBotProfileDialog'

type Row =
  | { key: string; at: number; kind: 'message'; message: ImMessage }
  | { key: string; at: number; kind: 'pending'; pending: PendingMessage }

type AgentFile = {
  name: string
  mediaType: string
  path: string
  sizeBytes: number
  previewUrl?: string
}
type AgentLine = {
  id: string
  role: 'user' | 'assistant'
  text: string
  status: string
  files: AgentFile[]
  parts?: TobotPart[]
  at: number
}

type ListDigest = { text: string; at: number }

/** One line for the inbox. Headings and emphasis stay as words. */
function listSummary(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[#*_`~]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

function messageAt(value: unknown): number {
  if (typeof value !== 'string') return 0
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function latestAgentDigest(lines: readonly Pick<AgentLine, 'text' | 'at' | 'files'>[]): ListDigest | null {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]
    const text =
      listSummary(line.text) ||
      line.files
        .map((file) => file.name)
        .filter(Boolean)
        .join(' ')
    if (!text) continue
    return { text, at: line.at }
  }
  return null
}
type ResidentAgent = 'tobot' | 'toboss'
type GroupReply = {
  id: string
  prompt: string
  modelText?: string
  text: string
  status: string
  parts?: TobotPart[]
  agent?: ResidentAgent
}

function lineKey(line: Pick<AgentLine, 'text' | 'files'>): string {
  if (line.files.length === 0) return line.text
  return `${line.text}\u0000${line.files.map((file) => file.name).join('\u0000')}`
}

function userLine(text: string, files: AgentFile[] = []): AgentLine {
  return { id: `local-${Date.now()}`, role: 'user', text, status: 'success', files, at: Date.now() }
}

function fileSizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`
  const mb = bytes / (1024 * 1024)
  return `${mb >= 10 ? Math.round(mb) : mb.toFixed(1)} MB`
}

function previewUrlFor(file: globalThis.File): string | undefined {
  if (!file.type.startsWith('image/') || typeof globalThis.URL.createObjectURL !== 'function') return undefined
  try {
    return globalThis.URL.createObjectURL(file)
  } catch {
    return undefined
  }
}

/** WKWebView hides a cross-origin error body and throws this instead. A busy ToBot turn is one of those errors. */
function planeRejected(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error)
  return message === 'Load failed' || message === 'Failed to fetch' || message.includes('already running')
}

function wait(ms: number) {
  return new Promise((resolve) => globalThis.window.setTimeout(resolve, ms))
}

function readTobotParts(value: unknown): TobotPart[] {
  if (!Array.isArray(value)) return []
  return value.filter(
    (part): part is TobotPart =>
      Boolean(part) && typeof part === 'object' && typeof (part as { type?: unknown }).type === 'string'
  )
}

function textOfParts(value: unknown): string {
  if (!Array.isArray(value)) return ''
  return value
    .map((part) => {
      if (!part || typeof part !== 'object') return ''
      const typed = part as { type?: unknown; text?: unknown }
      return typed.type === 'text' && typeof typed.text === 'string' ? typed.text : ''
    })
    .filter(Boolean)
    .join('\n')
}

function filesOfParts(value: unknown): AgentFile[] {
  if (!Array.isArray(value)) return []
  const files: AgentFile[] = []
  for (const part of value) {
    if (!part || typeof part !== 'object') continue
    const typed = part as {
      type?: unknown
      url?: unknown
      filename?: unknown
      mediaType?: unknown
      sizeBytes?: unknown
    }
    if (typed.type !== 'file' || typeof typed.url !== 'string') continue
    const path = pathFromFileUrl(typed.url)
    if (!path.startsWith('/')) continue
    const name = typeof typed.filename === 'string' && typed.filename ? typed.filename : path.split('/').pop() || 'file'
    files.push({
      name,
      mediaType: typeof typed.mediaType === 'string' ? typed.mediaType : 'application/octet-stream',
      path,
      sizeBytes: typeof typed.sizeBytes === 'number' ? typed.sizeBytes : 0,
      previewUrl: previewFor(path)
    })
  }
  return files
}

/** Newest-first session page, oldest-first on screen. Only user and assistant lines. */
function agentLinesFrom(page: unknown): AgentLine[] {
  const items =
    page && typeof page === 'object' && Array.isArray((page as { items?: unknown }).items)
      ? (page as { items: unknown[] }).items
      : []
  const lines: AgentLine[] = []
  for (const item of items) {
    if (!item || typeof item !== 'object') continue
    const record = item as {
      id?: unknown
      role?: unknown
      status?: unknown
      searchableText?: unknown
      createdAt?: unknown
      data?: { parts?: unknown }
    }
    const role = record.role === 'assistant' ? 'assistant' : record.role === 'user' ? 'user' : null
    if (!role || typeof record.id !== 'string') continue
    const fromParts = textOfParts(record.data?.parts)
    const text = fromParts || (typeof record.searchableText === 'string' ? record.searchableText : '')
    lines.push({
      id: record.id,
      role,
      text,
      files: filesOfParts(record.data?.parts),
      parts: role === 'assistant' ? readTobotParts(record.data?.parts) : undefined,
      status: typeof record.status === 'string' ? record.status : 'success',
      at: messageAt(record.createdAt)
    })
  }
  return lines.reverse()
}

function withPendingUser(lines: AgentLine[], pending: AgentLine | null): AgentLine[] {
  if (!pending || lines.some((line) => line.role === 'user' && lineKey(line) === lineKey(pending))) return lines
  return [...lines, pending]
}

function assistantAfter(lines: AgentLine[], pending: AgentLine): AgentLine | undefined {
  const key = lineKey(pending)
  let userAt = -1
  lines.forEach((line, index) => {
    if (line.role === 'user' && lineKey(line) === key) userAt = index
  })
  if (userAt < 0) return undefined
  return lines.slice(userAt + 1).find((line) => line.role === 'assistant' && line.status !== 'pending')
}

function assistantForPrompt(lines: AgentLine[], prompt: string): AgentLine | undefined {
  let userAt = -1
  lines.forEach((line, index) => {
    if (line.role === 'user' && line.text === prompt) userAt = index
  })
  if (userAt < 0) return undefined
  return lines.slice(userAt + 1).find((line) => line.role === 'assistant')
}

function settleGroupReplies(
  current: Record<string, GroupReply[]>,
  lines: AgentLine[],
  agent: ResidentAgent = 'tobot'
): Record<string, GroupReply[]> {
  let next = current
  let changed = false
  for (const [conversationId, replies] of Object.entries(current)) {
    const waiting = replies.filter((line) => line.status === 'pending' && (line.agent ?? 'tobot') === agent)
    if (waiting.length === 0) continue
    let kept = replies
    for (const item of waiting) {
      const reply = assistantAfter(lines, {
        id: '',
        role: 'user',
        text: item.modelText || item.prompt,
        status: 'success',
        files: [],
        at: 0
      })
      if (!reply || reply.status === 'pending') continue
      changed = true
      kept = kept
        .filter((line) => line.id !== item.id)
        .concat({
          id: reply.id,
          prompt: item.prompt,
          modelText: item.modelText,
          text: reply.text,
          status: reply.status,
          parts: reply.parts,
          agent: item.agent
        })
    }
    if (kept !== replies) next = { ...next, [conversationId]: kept }
  }
  return changed ? next : current
}

function withoutAskedPending(
  lines: readonly GroupReply[],
  text: string,
  agents: ReadonlySet<ResidentAgent>
): GroupReply[] {
  return lines.filter((line) => {
    if (line.prompt !== text || line.status !== 'pending') return true
    return !agents.has(line.agent ?? 'tobot')
  })
}

const ConversationAvatar = memo(function ConversationAvatar({
  entry,
  title,
  productLogo,
  faces,
  phone = '',
  avatar,
  colorKey,
  online = false,
  size = 36,
  animated = false
}: {
  entry: ImInboxEntry
  title: string
  productLogo: string
  faces?: readonly GroupFace[]
  phone?: string
  avatar?: string
  colorKey?: string
  online?: boolean
  size?: number
  animated?: boolean
}) {
  if (entry.kind === 'product' && productLogo) {
    return <img alt="" src={productLogo} className="size-9 shrink-0 rounded-[8px] object-cover" />
  }
  if (entry.kind === 'product') return <AccountAvatar kind="article" />
  if (entry.kind === 'system') return <AccountAvatar kind="bell" />
  if (entry.kind === 'transfer') {
    return <ToBotMark size={size} label={title} />
  }
  if (entry.kind === 'boss') {
    return <ToBossMark size={size} label={title} animated={animated} />
  }
  if (entry.kind === 'group') {
    return <GroupAvatar faces={faces?.length ? faces : [{ kind: 'person', name: title }]} size={size} />
  }
  return <PersonAvatar name={title} phone={phone} avatar={avatar} colorKey={colorKey} online={online} size={size} />
})

const menuItemClass =
  'flex h-8 w-full items-center gap-2 rounded-[6px] px-2 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f5f5f5]'
const menuDangerClass =
  'flex h-8 w-full items-center gap-2 rounded-[6px] px-2 text-left text-[13px] text-[#b42318] outline-none hover:bg-[#fef3f2]'
const menuIconClass = 'size-4 shrink-0'

function PickBox({ checked, label, onToggle }: { checked: boolean; label: string; onToggle: () => void }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      data-ui="messages.pick"
      onClick={(event) => {
        event.stopPropagation()
        onToggle()
      }}
      className="mt-2 grid size-[22px] shrink-0 place-items-center rounded-full border outline-none"
      style={
        checked
          ? { backgroundColor: '#171717', borderColor: '#171717' }
          : { backgroundColor: '#ffffff', borderColor: '#6b6b6b' }
      }>
      {checked ? (
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
          <path
            d="M3.2 8.2 6.4 11.4 12.8 4.6"
            fill="none"
            stroke="#ffffff"
            strokeWidth="2.4"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ stroke: '#ffffff' }}
          />
        </svg>
      ) : null}
    </button>
  )
}

const submitPendingForm: AgentFormSubmit = async (submission, signal) => {
  if (submission.binding.owner.sessionId !== currentTobotSession()) return 'failed'
  return (await submitAgentForm(submission, signal)) ? 'sent' : 'failed'
}

function TobotTurnView({
  parts,
  text,
  streaming,
  onContextMenu
}: {
  parts: readonly TobotPart[]
  text: string
  streaming: boolean
  onContextMenu?: (event: ReactMouseEvent) => void
}) {
  const source =
    parts.length > 0 ? parts : text ? [{ type: 'text', text, state: streaming ? 'streaming' : 'done' }] : []
  if (source.length === 0) return null
  return (
    <div
      data-ui="messages.tobot-turn"
      data-agent-streaming={streaming ? 'true' : 'false'}
      className="w-full min-w-0"
      onContextMenu={onContextMenu}>
      <AgentFormSubmitContext value={submitPendingForm}>
        <TaskConversationTurn parts={source as CherryMessagePart[]} streaming={streaming} />
      </AgentFormSubmitContext>
    </div>
  )
}

function TobotApprovalCard({
  toolName,
  onAllow,
  onDeny
}: {
  toolName: string
  onAllow: () => void
  onDeny: () => void
}) {
  const { t } = useTranslation()
  return (
    <div
      data-ui="messages.tobot-approval"
      className="w-full max-w-[360px] rounded-[10px] border border-[#ebebeb] bg-white p-3 shadow-[0_8px_22px_rgba(20,40,80,0.08)]">
      <p className="m-0 text-[13px] text-[#171717]">{t('messages.tobot.approval')}</p>
      {toolName ? <p className="m-0 mt-1 truncate font-mono text-[12px] text-[#5f5f5f]">{toolName}</p> : null}
      <div className="mt-3 flex gap-2">
        <Button size="sm" onClick={onAllow}>
          {t('messages.tobot.approval_allow')}
        </Button>
        <Button size="sm" variant="outline" onClick={onDeny}>
          {t('messages.tobot.approval_deny')}
        </Button>
      </div>
    </div>
  )
}

function ConversationActions({
  entry,
  onPinned,
  onMuted,
  onUnread,
  onEdit,
  onReset
}: {
  entry: ImInboxEntry
  onPinned: () => void
  onMuted: () => void
  onUnread: () => void
  onEdit?: () => void
  onReset?: () => void
}) {
  const { t } = useTranslation()
  return (
    <>
      <button type="button" role="menuitem" onClick={onPinned} className={menuItemClass}>
        {entry.pinned ? (
          <PinOff className={menuIconClass} strokeWidth={1.7} aria-hidden />
        ) : (
          <Pin className={menuIconClass} strokeWidth={1.7} aria-hidden />
        )}
        {t(entry.pinned ? 'messages.unpin' : 'messages.pin')}
      </button>
      <button type="button" role="menuitem" onClick={onMuted} className={menuItemClass}>
        {entry.muted ? (
          <Bell className={menuIconClass} strokeWidth={1.7} aria-hidden />
        ) : (
          <BellOff className={menuIconClass} strokeWidth={1.7} aria-hidden />
        )}
        {t(entry.muted ? 'messages.unmute' : 'messages.mute')}
      </button>
      {entry.lastSequence > 0 || entry.id === BUILTIN_TOBOT_ID || entry.id === BUILTIN_TOBOSS_ID ? (
        <button type="button" role="menuitem" onClick={onUnread} className={menuItemClass}>
          <Mail className={menuIconClass} strokeWidth={1.7} aria-hidden />
          {t('messages.unread')}
        </button>
      ) : null}
      {onEdit ? (
        <button type="button" role="menuitem" onClick={onEdit} className={menuItemClass}>
          <Pencil className={menuIconClass} strokeWidth={1.7} aria-hidden />
          {t('messages.tobot.edit')}
        </button>
      ) : null}
      {onReset ? (
        <>
          <div className="my-1 h-px bg-[#f0f0f0]" />
          <button type="button" role="menuitem" onClick={onReset} className={menuDangerClass}>
            <RotateCcw className={menuIconClass} strokeWidth={1.7} aria-hidden />
            {t(entry.kind === 'boss' ? 'messages.toboss.reset' : 'messages.tobot.reset')}
          </button>
        </>
      ) : null}
    </>
  )
}

function InfoTextRow({ label, value, dataUi }: { label: string; value: string; dataUi?: string }) {
  return (
    <div data-ui={dataUi} className="flex h-9 items-center gap-3 border-b border-[#efefef] px-4">
      <span className="shrink-0 text-[13px] leading-5 text-[#171717]">{label}</span>
      <span className="min-w-0 flex-1 truncate text-right text-[13px] leading-5 text-[#8a8a8a]">{value}</span>
    </div>
  )
}

/** One settings row. Click to edit, Enter or blur commits, Escape restores the saved value. */
function InfoEditRow({
  label,
  value,
  placeholder,
  dataUi,
  resetKey,
  onCommit
}: {
  label: string
  value: string
  placeholder: string
  dataUi: string
  resetKey: string
  onCommit: (next: string) => boolean | Promise<void>
}) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(value)
  const [shown, setShown] = useState(value)
  const inputRef = useRef<globalThis.HTMLInputElement>(null)
  const committed = useRef(false)
  const valueRef = useRef(value)
  valueRef.current = value

  useEffect(() => {
    committed.current = true
    setEditing(false)
    setDraft(valueRef.current)
    setShown(valueRef.current)
  }, [resetKey])

  useEffect(() => {
    if (!editing) setShown(value)
  }, [editing, value])

  useEffect(() => {
    if (!editing) return
    committed.current = false
    const input = inputRef.current
    input?.focus()
    input?.select()
  }, [editing])

  const exit = (nextShown: string) => {
    committed.current = true
    setDraft(nextShown)
    setShown(nextShown)
    setEditing(false)
  }

  const commit = (from: 'enter' | 'blur') => {
    if (committed.current) return
    const next = draft.trim()
    if (next === value.trim()) {
      exit(value)
      return
    }
    const result = onCommit(next)
    if (result === false) {
      if (from === 'blur') exit(value)
      return
    }
    exit(next)
    void Promise.resolve(result).catch(() => {
      setDraft(value)
      setShown(value)
    })
  }

  if (!editing) {
    const text = shown.trim()
    return (
      <button
        type="button"
        data-ui={dataUi}
        aria-label={label}
        onClick={() => {
          setDraft(shown)
          setEditing(true)
        }}
        className="flex h-9 w-full items-center gap-3 border-b border-[#efefef] px-4 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
        <span className="shrink-0 text-[13px] leading-5 text-[#171717]">{label}</span>
        <span
          className={cn(
            'min-w-0 flex-1 truncate text-right text-[13px] leading-5',
            text ? 'text-[#8a8a8a]' : 'text-[#c8c8c8]'
          )}>
          {text || placeholder}
        </span>
      </button>
    )
  }

  return (
    <div data-ui={dataUi} className="flex h-9 items-center gap-3 border-b border-[#efefef] px-4">
      <span className="shrink-0 text-[13px] leading-5 text-[#171717]">{label}</span>
      <input
        ref={inputRef}
        aria-label={label}
        value={draft}
        placeholder={placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault()
            commit('enter')
          } else if (event.key === 'Escape') {
            event.preventDefault()
            exit(value)
          }
        }}
        onBlur={() => commit('blur')}
        className="h-7 min-w-0 flex-1 rounded-[6px] border-0 bg-[#f5f5f5] px-2 text-right text-[13px] text-[#171717] outline-none placeholder:text-[#c8c8c8] focus-visible:ring-2 focus-visible:ring-black/10"
      />
    </div>
  )
}

const infoRowClass =
  'flex h-9 w-full items-center border-b border-[#efefef] px-4 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10'

function InfoSwitchRow({
  label,
  pressed,
  dataUi,
  onClick
}: {
  label: string
  pressed: boolean
  dataUi?: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      data-ui={dataUi}
      aria-pressed={pressed}
      onClick={onClick}
      className="flex h-9 w-full items-center justify-between gap-3 border-b border-[#efefef] px-4 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
      <span className="min-w-0 truncate text-[13px] leading-5 text-[#171717]">{label}</span>
      <span
        aria-hidden
        className={cn('relative h-[22px] w-10 shrink-0 rounded-full', pressed ? 'bg-[#171717]' : 'bg-[#e6e6e6]')}>
        <span
          className={cn(
            'absolute top-[2px] size-[18px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.18)]',
            pressed ? 'left-5' : 'left-0.5'
          )}
        />
      </span>
    </button>
  )
}

function InfoLinkRow({ label, dataUi, onClick }: { label: string; dataUi?: string; onClick: () => void }) {
  return (
    <button
      type="button"
      data-ui={dataUi}
      onClick={onClick}
      className="flex h-9 w-full items-center justify-between border-b border-[#efefef] px-4 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
      <span>{label}</span>
      <ChevronRight className="size-4 shrink-0 text-[#c8c8c8]" strokeWidth={1.7} aria-hidden />
    </button>
  )
}

function prefersReducedMotion(): boolean {
  return (
    typeof globalThis.window.matchMedia === 'function' &&
    globalThis.window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

/** The conversation sheet stays mounted through the fade, then leaves the tree. */
function InfoDrawer({
  open,
  panelRef,
  children
}: {
  open: boolean
  panelRef: { current: globalThis.HTMLElement | null }
  children: ReactNode
}) {
  const [mounted, setMounted] = useState(open)
  const [entered, setEntered] = useState(false)
  if (open && !mounted) {
    setMounted(true)
    setEntered(false)
  }

  useEffect(() => {
    if (!mounted || !open) return
    if (prefersReducedMotion()) {
      setEntered(true)
      return
    }
    const frame = globalThis.requestAnimationFrame(() => setEntered(true))
    return () => globalThis.cancelAnimationFrame(frame)
  }, [open, mounted])

  useEffect(() => {
    if (open || !mounted) return
    setEntered(false)
    if (prefersReducedMotion()) {
      setMounted(false)
      return
    }
    const timer = globalThis.window.setTimeout(() => setMounted(false), 200)
    return () => globalThis.window.clearTimeout(timer)
  }, [open, mounted])

  if (!mounted) return null
  const shown = prefersReducedMotion() ? open : entered && open
  return (
    <aside
      ref={panelRef}
      data-ui="messages.info"
      data-state={shown ? 'open' : 'closed'}
      className={cn(
        'absolute top-12 right-0 bottom-0 z-30 flex w-[300px] flex-col overflow-y-auto border-l border-[#efefef] bg-white shadow-[0_16px_48px_rgba(23,23,23,0.16)] motion-reduce:transition-none',
        shown
          ? 'scale-100 opacity-100 transition-[opacity,scale] duration-[160ms] ease-[cubic-bezier(0.16,1,0.3,1)]'
          : 'scale-[0.98] opacity-0 transition-[opacity,scale] duration-200 ease-[cubic-bezier(0.4,0,1,1)]'
      )}>
      {children}
    </aside>
  )
}

function tobotGroupStorageKey(orgId: string, sessionId: string): string {
  return sessionId === 'tobot' ? `messages.tobot.groups.${orgId}` : `messages.tobot.groups.${sessionId}.${orgId}`
}

function readToBotGroups(orgId: string | null, sessionId: string): string[] {
  return readGroupIds(tobotGroupStorageKey(orgId ?? '', sessionId), orgId)
}

function readGroupIds(key: string, orgId: string | null): string[] {
  if (!orgId || typeof localStorage === 'undefined') return []
  try {
    const raw = globalThis.localStorage.getItem(key)
    const parsed = raw ? (JSON.parse(raw) as unknown) : []
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : []
  } catch {
    return []
  }
}

const TOBOT_CONTEXT_CAP = 6
const TOBOT_HISTORY_CAP = 4

type TobotMediaSource = { id: string; name: string; mime: string | null }

/** Recent image and file messages already in the thread. Voice and recalled rows stay out. */
function recentThreadMedia(messages: readonly ImMessage[], sameSendCount: number): TobotMediaSource[] {
  const room = Math.min(TOBOT_HISTORY_CAP, Math.max(0, TOBOT_CONTEXT_CAP - sameSendCount))
  if (room === 0) return []
  const sources: TobotMediaSource[] = []
  for (const message of messages) {
    if (message.recalled) continue
    if (message.body.kind !== 'image' && message.body.kind !== 'file') continue
    sources.push({
      id: message.id,
      name: message.body.attachment.name,
      mime: message.body.attachment.mime
    })
  }
  return sources.slice(-room)
}

function ThreadState({ snapshot, onRetry, t }: { snapshot: ImSnapshot; onRetry: () => void; t: Translate }) {
  const loading = snapshot.status === 'loading' || snapshot.status === 'idle'
  return (
    <div
      data-ui="messages.state"
      data-status={snapshot.status}
      className="flex min-h-0 flex-1 flex-col"
      aria-busy={loading}>
      {loading ? (
        <p className="m-auto text-[13px] text-[#8a8a8a]">{t('messages.state.loading')}</p>
      ) : snapshot.status === 'error' ? (
        <EmptyState
          icon={CloudOff}
          title={t('messages.state.error_title')}
          description={snapshot.error || undefined}
          actionLabel={t('messages.retry')}
          onAction={onRetry}
        />
      ) : snapshot.status === 'signed-out' ? (
        <EmptyState
          icon={LogIn}
          title={t('messages.state.signed_out_title')}
          description={t('messages.state.signed_out')}
          actionLabel={t('messages.state.sign_in_again')}
          onAction={() => void logoutMemberAccount().finally(returnToLogin)}
        />
      ) : snapshot.status === 'no-org' ? (
        <EmptyState
          icon={Building2}
          title={t('messages.state.no_org_title')}
          description={t('messages.state.no_org')}
        />
      ) : (
        <EmptyState
          icon={LogIn}
          title={t('messages.state.unavailable_title')}
          description={t('messages.state.unavailable')}
        />
      )}
    </div>
  )
}

type MessagePortrait = { name: string; phone: string; avatar: string }

const EMPTY_PORTRAIT: MessagePortrait = { name: '', phone: '', avatar: '' }

export default function MessagesPage() {
  const { t } = useTranslation()
  const { snapshot, store } = useIm()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const composer = useConversationComposer()
  const draft = composer.text
  const [fileDragOver, setFileDragOver] = useState(false)
  const [stagedFile, setStagedFile] = useState<globalThis.File | null>(null)
  const [stagedImage, setStagedImage] = useState<{ name: string; src: string } | null>(null)
  const fileDragDepth = useRef(0)
  const [composerCaret, setComposerCaret] = useState(0)
  const [mentionIndex, setMentionIndex] = useState(0)
  const [mentionDismissed, setMentionDismissed] = useState<string | null>(null)
  const [mentionFocused, setMentionFocused] = useState(false)
  const shots = composer.files
  const [notice, setNotice] = useState('')
  const [menu, setMenu] = useState<'plus' | 'emoji' | 'info' | 'more' | null>(null)
  const [dialog, setDialog] = useState<'org' | 'group' | 'add' | 'leave' | 'dissolve' | 'profile' | 'reset' | null>(
    null
  )
  const tobotProfile = useToBotProfile()
  const bossProfile = useBotProfile(TOBOSS_PROFILE_KEY)
  const botLabel = tobotDisplayName(tobotProfile, t('messages.file_transfer'))
  const bossLabel = tobotDisplayName(bossProfile, t('messages.toboss'))
  const [listWidth, setListWidth] = useState(PAGE_SIDEBAR_DEFAULT_WIDTH)
  const [composerHeight, setComposerHeight] = useState(72)
  const [loadingOlder, setLoadingOlder] = useState(false)
  const [memberQuery, setMemberQuery] = useState('')
  const [memberMenu, setMemberMenu] = useState<{ userId: string; x: number; y: number } | null>(null)
  const [memberConfirm, setMemberConfirm] = useState<{
    kind: 'remove' | 'transfer'
    userId: string
    name: string
  } | null>(null)
  const memberMenuRef = useRef<globalThis.HTMLDivElement>(null)
  const memberConfirmRef = useRef(memberConfirm)
  memberConfirmRef.current = memberConfirm
  const [tobotSessionId, setTobotSessionId] = useState(BUILTIN_TOBOT_ID)
  const tobotSessionRef = useRef(BUILTIN_TOBOT_ID)
  const [tobossSessionId, setTobossSessionId] = useState(BUILTIN_TOBOSS_ID)
  const tobossSessionRef = useRef(BUILTIN_TOBOSS_ID)
  const selectedKindRef = useRef<ImInboxEntry['kind'] | undefined>(undefined)
  const [memberEpoch, setMemberEpoch] = useState(0)
  const [agentLines, setAgentLines] = useState<AgentLine[]>([])
  const [agentWaiting, setAgentWaiting] = useState(false)
  const [tobotLive, setTobotLive] = useState<TobotLive | null>(null)
  const [tobotApproval, setTobotApproval] = useState<TobotApproval | null>(null)
  const tobotToolNames = useRef(new Map<string, string>())
  const [savedDigest, setSavedDigest] = useState<ListDigest | null>(null)
  const [savedBossDigest, setSavedBossDigest] = useState<ListDigest | null>(null)
  const [profileBot, setProfileBot] = useState<'tobot' | 'toboss'>('tobot')
  const [rowMenu, setRowMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const [bubbleMenu, setBubbleMenu] = useState<{
    x: number
    y: number
    text: string
    recall: { conversationId: string; messageId: string } | null
    quote: MessageQuote | null
    selectId: string | null
    deleteId: string | null
  } | null>(null)
  const [picked, setPicked] = useState<string[] | null>(null)
  const [forwardMode, setForwardMode] = useState<'each' | 'merge' | null>(null)
  const quoteDraft = composer.quote
  const [historyOpen, setHistoryOpen] = useState(false)
  const [historyLoading, setHistoryLoading] = useState(false)
  const [historyAgentLines, setHistoryAgentLines] = useState<AgentLine[] | null>(null)
  const [focusId, setFocusId] = useState<string | null>(null)
  const [tobotGroups, setToBotGroups] = useState<string[]>([])
  const [tobossGroups, setToBossGroups] = useState<string[]>([])
  const [groupReplies, setGroupReplies] = useState<Record<string, GroupReply[]>>({})
  const [portraits, setPortraits] = useState<Record<string, MessagePortrait>>(readCachedPortraits)
  const [selfPortrait, setSelfPortrait] = useState<MessagePortrait>(readCachedSelfPortrait)
  const pendingAgentLine = useRef<AgentLine | null>(null)
  const hiddenAgentIds = useRef(new Set<string>())
  const pendingDataWrite = useRef<{ prompt: string; channel: DataChannelId } | null>(null)
  const transcriptEpoch = useRef(0)
  const pageRef = useRef<globalThis.HTMLDivElement>(null)
  const threadRef = useRef<globalThis.HTMLDivElement>(null)
  const threadContentRef = useRef<globalThis.HTMLDivElement>(null)
  const stickToBottom = useRef(true)
  const shownThread = useRef<string | null>(null)
  const plusRef = useRef<globalThis.HTMLDivElement>(null)
  const emojiRef = useRef<globalThis.HTMLDivElement>(null)
  const infoPanelRef = useRef<globalThis.HTMLElement>(null)
  const moreRef = useRef<globalThis.HTMLDivElement>(null)
  const contextRef = useRef<globalThis.HTMLDivElement>(null)
  const bubbleMenuRef = useRef<globalThis.HTMLDivElement>(null)
  const fileRef = useRef<globalThis.HTMLInputElement>(null)
  const imageRef = useRef<globalThis.HTMLInputElement>(null)
  const composerRef = useRef<globalThis.HTMLTextAreaElement>(null)
  const pendingCaret = useRef<number | null>(null)
  const typingSentAt = useRef(0)
  const focusTimer = useRef<number | null>(null)
  const runBossJobRef = useRef<(job: BossJob) => void>(() => undefined)

  const profile = useMemberProfile()
  const accountAvatar = useAvatar()
  const ready = snapshot.status === 'ready'
  const personalToBot = snapshot.status === 'no-org' && selectedId === BUILTIN_TOBOT_ID
  const meId = snapshot.me?.userId
  const now = Date.now()
  const productLogo = studioProductLogo() || studioProductMark()
  const titleOf = (entry: ImInboxEntry) => {
    if (entry.kind === 'boss' && !entry.title) return entry.remark?.trim() || bossLabel
    return entry.kind === 'transfer' && !entry.title ? botLabel : conversationTitle(entry, snapshot, t)
  }
  const tobotHost = (): ToBotHostContext | null => tobotHostContext(tobotProfile, t('messages.file_transfer'))
  const bossHost = (): ToBotHostContext | null => tobotHostContext(bossProfile, t('messages.toboss'))
  const openProfile = (who: 'tobot' | 'toboss') => {
    setProfileBot(who)
    setMenu(null)
    setRowMenu(null)
    setBubbleMenu(null)
    setDialog('profile')
  }
  const openToBotProfile = () => openProfile('tobot')
  const openBossProfile = () => openProfile('toboss')
  const knownName = (userId: string | null | undefined): string => {
    if (!userId) return ''
    if (snapshot.me && userId === snapshot.me.userId) return snapshot.me.displayName
    return snapshot.people[userId]?.displayName ?? ''
  }
  const portraitFor = (userId: string | null | undefined, fallbackName: string): MessagePortrait => {
    const row = userId ? portraits[userId] : undefined
    const mine = Boolean((meId && userId === meId) || (!meId && !userId))
    const savedName = mine ? profile?.displayName.trim() || '' : ''
    const savedPhone = mine ? profile?.phone || '' : ''
    const savedAvatar = mine ? accountAvatar || selfPortrait.avatar : ''
    const memberName = row?.name.trim() || ''
    return {
      name: savedName || memberName || fallbackName.trim() || (mine ? selfPortrait.name : ''),
      phone: savedPhone || row?.phone || (mine ? selfPortrait.phone : ''),
      avatar: savedAvatar || row?.avatar || ''
    }
  }
  const personalBotIn = (id: string) => !snapshot.orgId && tobotGroups.includes(id)
  const groupFacesOf = (entry: ImInboxEntry): GroupFace[] => {
    const members = snapshot.details[entry.id]?.members.filter((member) => !member.left) ?? []
    const ordered = [
      ...members.filter((member) => member.userId !== meId),
      ...members.filter((member) => member.userId === meId)
    ]
    const faces: GroupFace[] = ordered.map((member) => {
      const portrait = portraitFor(member.userId, knownName(member.userId))
      return {
        kind: 'person',
        name: portrait.name,
        phone: portrait.phone,
        avatar: portrait.avatar,
        colorKey: member.userId
      }
    })
    if (personalBotIn(entry.id)) faces.push({ kind: 'bot', bot: 'tobot', label: botLabel, profile: tobotProfile })
    if (tobossGroups.includes(entry.id)) faces.push({ kind: 'bot', bot: 'toboss', label: bossLabel })
    return faces
  }
  const needle = query.trim().toLocaleLowerCase()
  const youWereMentioned = (entry: ImInboxEntry) =>
    entryMentionsYou(entry, snapshot.threads[entry.id]?.messages, {
      myId: meId ?? null,
      myName: snapshot.me?.displayName ?? '',
      everyone: t('messages.mention.all')
    })
  const visible = snapshot.conversations.filter((entry) => {
    if (!needle) return true
    const hint = youWereMentioned(entry) ? t('messages.mention.you') : ''
    return `${titleOf(entry)} ${previewText(entry, snapshot, t)} ${hint}`.toLocaleLowerCase().includes(needle)
  })
  const pinned = visible.filter((entry) => entry.pinned)
  const rest = visible.filter((entry) => !entry.pinned)
  const selected = snapshot.conversations.find((entry) => entry.id === selectedId) ?? null
  selectedKindRef.current = selected?.kind
  const thread = selected ? snapshot.threads[selected.id] : undefined
  const detail = selected ? snapshot.details[selected.id] : undefined
  const myRole = detail?.members.find((member) => member.userId === meId && !member.left)?.role
  const personOnline = (userId: string | null | undefined) =>
    Boolean(snapshot.presenceKnown && userId && snapshot.online.includes(userId))
  const typing = selected ? snapshot.typing[selected.id] : undefined
  const typingName = typing && typing.until > now && typing.userId !== meId ? personName(snapshot, typing.userId) : ''
  const activeMembers = detail?.members.filter((member) => !member.left) ?? []
  const rows: Row[] = [
    ...(thread?.messages ?? []).map(
      (message): Row => ({ key: message.id, at: message.createdAt, kind: 'message', message })
    ),
    ...(thread?.pending ?? []).map(
      (pending): Row => ({ key: pending.clientMsgId, at: pending.createdAt, kind: 'pending', pending })
    )
  ]
  const lastKey = rows.at(-1)?.key

  const myMembership = detail?.members.find((member) => member.userId === meId && !member.left)
  const shownName = (userId: string | null | undefined) => {
    if (!userId) return ''
    if (selected?.kind === 'group') {
      const nickname = detail?.members.find((member) => member.userId === userId && !member.left)?.nickname?.trim()
      if (nickname) return nickname
    }
    return knownName(userId) || personName(snapshot, userId)
  }
  const speechBlock =
    selected?.kind === 'group' && myMembership
      ? myMembership.speechBanned
        ? 'banned'
        : detail?.muteAll && myMembership.role === 'member'
          ? 'all'
          : null
      : null

  useEffect(() => {
    typingSentAt.current = 0
  }, [selected?.id])

  useEffect(() => {
    if (snapshot.status !== 'no-org') return
    const tobot = snapshot.conversations.find((entry) => entry.kind === 'transfer')
    if (!tobot || selectedId === tobot.id) return
    if (selectedId && snapshot.conversations.some((entry) => entry.id === selectedId)) return
    composer.focus(tobot.id)
    setSelectedId(tobot.id)
    void store.open(tobot.id).catch(() => undefined)
  }, [snapshot.status, snapshot.conversations, selectedId, store])

  useEffect(() => {
    if (selectedId !== BUILTIN_TOBOT_ID) return
    const real = snapshot.conversations.find((entry) => entry.kind === 'transfer' && entry.id !== BUILTIN_TOBOT_ID)
    if (!real) return
    composer.handoff(BUILTIN_TOBOT_ID, real.id)
    setSelectedId(real.id)
    void store.open(real.id).catch(() => undefined)
  }, [selectedId, snapshot.conversations, store])

  useLayoutEffect(() => {
    const publish = () => {
      const visible = globalThis.document.visibilityState !== 'hidden' && globalThis.document.hasFocus()
      setMessageFocus({ selectedId, lookedAtId: visible ? selectedId : null })
    }
    publish()
    globalThis.window.addEventListener('focus', publish)
    globalThis.window.addEventListener('blur', publish)
    globalThis.document.addEventListener('visibilitychange', publish)
    return () => {
      globalThis.window.removeEventListener('focus', publish)
      globalThis.window.removeEventListener('blur', publish)
      globalThis.document.removeEventListener('visibilitychange', publish)
      setMessageFocus({ selectedId: null, lookedAtId: null })
    }
  }, [selectedId])

  const selectRef = useRef<(id: string) => void>(() => undefined)
  useEffect(() => {
    const openPending = () => {
      const id = consumePendingMessage()
      if (id) selectRef.current(id)
    }
    openPending()
    globalThis.window.addEventListener(OPEN_MESSAGE_EVENT, openPending)
    return () => globalThis.window.removeEventListener(OPEN_MESSAGE_EVENT, openPending)
  }, [])

  const dropGroupAgent = (agent: ResidentAgent) => {
    setGroupReplies((current) => {
      let changed = false
      const next: Record<string, GroupReply[]> = {}
      for (const [conversationId, lines] of Object.entries(current)) {
        const kept = lines.filter((line) => (line.agent ?? 'tobot') !== agent)
        if (kept.length !== lines.length) changed = true
        if (kept.length > 0) next[conversationId] = kept
      }
      return changed ? next : current
    })
  }

  const adoptResident = (boss: boolean, id: string) => {
    if (boss && id) noteBossSession(id)
    const ref = boss ? tobossSessionRef : tobotSessionRef
    const setId = boss ? setTobossSessionId : setTobotSessionId
    const viewing = boss ? selectedKindRef.current === 'boss' : selectedKindRef.current === 'transfer'
    if (!id || ref.current === id) {
      if (viewing) bindTobotSession(ref.current)
      return
    }
    ref.current = id
    setId(id)
    if (!boss) {
      transcriptEpoch.current += 1
      bindTobotSession(id)
      pendingAgentLine.current = null
      setAgentLines([])
      setAgentWaiting(false)
      setTobotLive(null)
      setTobotApproval(null)
      dropGroupAgent('tobot')
      return
    }
    if (!viewing) return
    transcriptEpoch.current += 1
    bindTobotSession(id)
    pendingAgentLine.current = null
    setAgentLines([])
    setAgentWaiting(false)
    setTobotLive(null)
    setTobotApproval(null)
  }

  const warmResident = (boss: boolean, host: ToBotHostContext | null) => {
    const path = boss ? '/agent-sessions/toboss/ensure' : '/agent-sessions/tobot/ensure'
    let stopped = false
    const timers: ReturnType<typeof globalThis.setTimeout>[] = []
    const delays = [0, 500, 1500]
    const attempt = (index: number) => {
      if (stopped || index >= delays.length) return
      timers.push(
        globalThis.setTimeout(() => {
          if (stopped) return
          void withHarnessComponents(host ? { hostContext: host } : {})
            .then((body) => {
              if (stopped) return undefined
              return dataApiService.post(path, { body })
            })
            .then(
              (ready) => {
                if (stopped) return
                stopped = true
                const id = sessionIdFromEnsure(ready)
                if (id) adoptResident(boss, id)
              },
              () => attempt(index + 1)
            )
        }, delays[index])
      )
    }
    attempt(0)
    return () => {
      stopped = true
      for (const timer of timers) globalThis.clearTimeout(timer)
    }
  }

  useEffect(() => {
    if (selected?.kind !== 'transfer') return
    return warmResident(false, tobotHostContext(tobotProfile, t('messages.file_transfer')))
  }, [memberEpoch, selected?.id, selected?.kind, t, tobotProfile])

  useEffect(() => {
    if (selected?.kind !== 'boss') return
    return warmResident(true, tobotHostContext(bossProfile, t('messages.toboss')))
  }, [bossProfile, memberEpoch, selected?.id, selected?.kind, t])

  useEffect(() => {
    if (selected?.kind === 'boss') return
    bindTobotSession(tobotSessionRef.current)
  }, [selected?.kind, tobotSessionId])

  const agentThread = selected?.kind === 'transfer' || selected?.kind === 'boss'
  const activeResidentId = selected?.kind === 'boss' ? tobossSessionId : tobotSessionId
  const activeResidentRef = () => (selectedKindRef.current === 'boss' ? tobossSessionRef : tobotSessionRef)

  const flushDataWrite = (lines: AgentLine[]) => {
    const job = pendingDataWrite.current
    if (!job) return
    const reply = assistantForPrompt(lines, job.prompt)
    if (!reply || reply.status === 'pending') return
    pendingDataWrite.current = null
    if (reply.status !== 'success' || !reply.text.trim()) return
    void deliverToBotReply(job.channel, job.prompt, reply.text)
  }
  const settleAgentLines = (page: unknown, epoch: number) => {
    if (epoch !== transcriptEpoch.current) return
    const server = agentLinesFrom(page).filter((line) => !hiddenAgentIds.current.has(line.id))
    const pending = pendingAgentLine.current
    const shown = withPendingUser(server, pending)
    setAgentLines(shown)
    setTobotApproval(approvalFromLines(shown))
    flushDataWrite(server)
    if (pending && assistantAfter(server, pending)) {
      pendingAgentLine.current = null
      setAgentWaiting(false)
    }
  }
  const settleAgentLinesRef = useRef(settleAgentLines)
  settleAgentLinesRef.current = settleAgentLines

  useEffect(() => {
    let stopped = false
    const epoch = transcriptEpoch.current
    void dataApiService
      .get(`/agent-sessions/${tobotSessionRef.current}/messages`, { query: { limit: 50 } })
      .then((page) => {
        if (stopped || epoch !== transcriptEpoch.current) return
        const digest = latestAgentDigest(agentLinesFrom(page))
        if (!digest) return
        setSavedDigest((current) => current ?? digest)
      })
      .catch(() => undefined)
    return () => {
      stopped = true
    }
  }, [tobotSessionId])

  const bossListed = snapshot.conversations.some((entry) => entry.id === BUILTIN_TOBOSS_ID)
  useEffect(() => {
    if (!bossListed) return
    let stopped = false
    const epoch = transcriptEpoch.current
    void dataApiService
      .get(`/agent-sessions/${tobossSessionRef.current}/messages`, { query: { limit: 50 } })
      .then((page) => {
        if (stopped || epoch !== transcriptEpoch.current) return
        const digest = latestAgentDigest(omitBossRoomTurns(agentLinesFrom(page)))
        if (!digest) return
        setSavedBossDigest((current) => current ?? digest)
      })
      .catch(() => undefined)
    return () => {
      stopped = true
    }
  }, [bossListed, tobossSessionId])

  useEffect(() => {
    if (!agentThread) {
      pendingAgentLine.current = null
      setAgentLines([])
      setAgentWaiting(false)
      return
    }
    let stopped = false
    const epoch = transcriptEpoch.current
    const sessionId = activeResidentRef().current
    void dataApiService
      .get(`/agent-sessions/${sessionId}/messages`, { query: { limit: 50 } })
      .then((page) => {
        if (!stopped) settleAgentLinesRef.current(page, epoch)
      })
      .catch(() => undefined)
    return () => {
      stopped = true
    }
  }, [agentThread, selected?.id, activeResidentId])

  const agentLinesRef = useRef(agentLines)
  agentLinesRef.current = agentLines
  useEffect(() => {
    if (!agentThread) return
    let stopped = false
    const tick = () => {
      const lines = agentLinesRef.current
      const follow =
        agentWaiting ||
        lines.length === 0 ||
        lines.some((line) => line.role === 'assistant' && line.status === 'pending')
      if (!follow) return
      const epoch = transcriptEpoch.current
      const sessionId = activeResidentRef().current
      void dataApiService
        .get(`/agent-sessions/${sessionId}/messages`, { query: { limit: 50 } })
        .then((page) => {
          if (!stopped) settleAgentLinesRef.current(page, epoch)
        })
        .catch(() => undefined)
    }
    tick()
    const timer = globalThis.setInterval(tick, 800)
    return () => {
      stopped = true
      globalThis.clearInterval(timer)
    }
  }, [agentThread, agentWaiting, selected?.id, activeResidentId])

  useEffect(() => {
    setToBotGroups(readToBotGroups(snapshot.orgId, tobotSessionId))
  }, [snapshot.orgId, tobotSessionId])

  useEffect(() => {
    setToBossGroups(readBossGroups(snapshot.orgId, tobossSessionId))
  }, [snapshot.orgId, tobossSessionId])

  useEffect(() => {
    let stopped = false
    const orgId = snapshot.orgId
    void (async () => {
      let self = EMPTY_PORTRAIT
      try {
        const account = await readMemberAccount()
        publishMemberProfile(account)
        self = { name: account.displayName, phone: account.phone, avatar: account.avatar ?? '' }
      } catch {
        // The inbox names still label people when the member profile is unreachable.
      }
      const next: Record<string, MessagePortrait> = {}
      if (orgId) {
        try {
          const rows = await listOrgMembers(orgId)
          for (const row of rows) {
            const avatar = row.avatar?.startsWith('data:image/') ? row.avatar : ''
            next[row.id] = { name: row.name, phone: row.phone ?? '', avatar }
          }
        } catch {
          // Phone and photo stay empty. Nickname text still comes from the inbox.
        }
      }
      if (stopped) return
      writeCachedPortraits(next, self)
      setSelfPortrait(self)
      setPortraits(next)
      setMemberEpoch((current) => current + 1)
    })()
    return () => {
      stopped = true
    }
  }, [snapshot.orgId])

  useEffect(() => {
    const picking = dialog === 'group' || dialog === 'add' || dialog === 'org' || forwardMode !== null
    if (!picking) return
    store.refreshDirectory()
    const orgId = snapshot.orgId
    if (!orgId) return
    let stopped = false
    void listOrgMembers(orgId)
      .then((rows) => {
        if (stopped) return
        setPortraits((current) => {
          const next = { ...current }
          for (const row of rows) {
            const avatar = row.avatar?.startsWith('data:image/') ? row.avatar : ''
            next[row.id] = { name: row.name, phone: row.phone ?? '', avatar }
          }
          mergeCachedPortraits(next)
          return next
        })
      })
      .catch(() => undefined)
    return () => {
      stopped = true
    }
  }, [dialog, forwardMode, snapshot.orgId, store])

  const bossGroupPending = Object.values(groupReplies).some((lines) =>
    lines.some((line) => line.status === 'pending' && line.agent === 'toboss')
  )
  const tobotGroupPending = Object.values(groupReplies).some((lines) =>
    lines.some((line) => line.status === 'pending' && (line.agent ?? 'tobot') === 'tobot')
  )
  const groupPending = bossGroupPending || tobotGroupPending
  const streamBoss = selected?.kind === 'boss' || (bossGroupPending && !tobotGroupPending)
  const streamSessionId = streamBoss ? tobossSessionId : tobotSessionId
  useEffect(() => {
    if (!tobotGroupPending) return
    let stopped = false
    const tick = () => {
      const epoch = transcriptEpoch.current
      void dataApiService
        .get(`/agent-sessions/${tobotSessionRef.current}/messages`, { query: { limit: 50 } })
        .then((page) => {
          if (stopped || epoch !== transcriptEpoch.current) return
          const lines = agentLinesFrom(page)
          flushDataWrite(lines)
          setGroupReplies((current) => settleGroupReplies(current, lines))
        })
        .catch(() => undefined)
    }
    const timer = globalThis.setInterval(tick, 800)
    return () => {
      stopped = true
      globalThis.clearInterval(timer)
    }
  }, [tobotGroupPending])

  useEffect(() => {
    if (!bossGroupPending) return
    let stopped = false
    const tick = () => {
      const epoch = transcriptEpoch.current
      void dataApiService
        .get(`/agent-sessions/${tobossSessionRef.current}/messages`, { query: { limit: 50 } })
        .then((page) => {
          if (stopped || epoch !== transcriptEpoch.current) return
          setGroupReplies((current) => settleGroupReplies(current, agentLinesFrom(page), 'toboss'))
        })
        .catch(() => undefined)
    }
    const timer = globalThis.setInterval(tick, 800)
    return () => {
      stopped = true
      globalThis.clearInterval(timer)
    }
  }, [bossGroupPending])

  const refreshTobotRef = useRef<() => void>(() => undefined)
  refreshTobotRef.current = () => {
    const epoch = transcriptEpoch.current
    const boss = selectedKindRef.current === 'boss' || streamBoss
    const sessionId = boss ? tobossSessionRef.current : tobotSessionRef.current
    void dataApiService
      .get(`/agent-sessions/${sessionId}/messages`, { query: { limit: 50 } })
      .then((page) => {
        if (epoch !== transcriptEpoch.current) return
        settleAgentLinesRef.current(page, epoch)
        setGroupReplies((current) => settleGroupReplies(current, agentLinesFrom(page), boss ? 'toboss' : 'tobot'))
      })
      .catch(() => undefined)
  }

  useEffect(() => {
    if (!agentThread && !groupPending) return
    const topic = tobotTopic(streamSessionId)
    return subscribeTobotStream(
      (payload) => {
        noteTobotTool(tobotToolNames.current, payload, topic)
        const approval = approvalFromChunk(payload, tobotToolNames.current, topic)
        if (approval) setTobotApproval(approval)
        const kind = payload.chunk.type
        if (kind === 'error' || kind === 'finish') setTobotApproval(null)
        setTobotLive((current) => applyTobotChunk(current, payload, topic))
      },
      () => {
        setTobotApproval(null)
        refreshTobotRef.current()
      },
      { sessionId: streamSessionId }
    )
  }, [agentThread, groupPending, streamSessionId])

  useEffect(() => {
    if (!tobotLive) return
    const settledLine = agentLines.some(
      (line) => line.id === tobotLive.anchorId && line.status !== 'pending' && line.text.length > 0
    )
    const settledGroup = Object.values(groupReplies).some((replies) =>
      replies.some((line) => line.id === tobotLive.anchorId && line.status !== 'pending')
    )
    if (settledLine || settledGroup) setTobotLive(null)
  }, [agentLines, groupReplies, tobotLive])

  const bossPrivate = selected?.kind === 'boss'
  const transcriptLines = bossPrivate ? omitBossRoomTurns(agentLines) : agentLines
  const privateLive =
    bossPrivate && tobotLive && isBossRoomTurnAnchor(agentLines, tobotLive.anchorId) ? null : tobotLive
  const shownAgentLines = overlayAgentLines<AgentLine>(transcriptLines, privateLive, (id, text, parts) => ({
    id,
    role: 'assistant',
    text,
    status: 'pending',
    files: [],
    parts,
    at: 0
  })).filter((line) => !hiddenAgentIds.current.has(line.id))
  const liveAssistantId = livePendingId(shownAgentLines)
  let abandonedAssistantId: string | null = null
  for (const line of shownAgentLines) {
    if (line.role !== 'assistant' || line.text || line.files.length > 0) continue
    if (line.status !== 'pending' && line.status !== 'error') continue
    if (line.status === 'pending' && line.id === liveAssistantId) continue
    abandonedAssistantId = line.id
  }
  const shownGroupReplies = (
    selected?.kind === 'group' ? overlayPendingReply(groupReplies[selected.id] ?? [], tobotLive) : []
  ).filter((line) => !hiddenAgentIds.current.has(line.id))
  const threadBottom = [
    lastKey ?? '',
    shownAgentLines.at(-1)?.id ?? '',
    String(shownAgentLines.at(-1)?.text.length ?? 0),
    String(shownAgentLines.at(-1)?.parts?.length ?? 0),
    agentWaiting ? '1' : '0',
    shownGroupReplies.at(-1)?.id ?? '',
    String(shownGroupReplies.at(-1)?.text.length ?? 0)
  ].join(':')

  useLayoutEffect(() => {
    const element = threadRef.current
    if (!element) return
    const switched = shownThread.current !== (selected?.id ?? null)
    shownThread.current = selected?.id ?? null
    if (switched || stickToBottom.current) {
      element.scrollTop = element.scrollHeight
      stickToBottom.current = true
    }
  }, [selected?.id, threadBottom])

  useLayoutEffect(() => {
    const caret = pendingCaret.current
    if (caret == null) return
    pendingCaret.current = null
    const area = composerRef.current
    if (!area) return
    area.setSelectionRange(caret, caret)
  }, [draft])

  useEffect(() => {
    const scroller = threadRef.current
    const content = threadContentRef.current
    if (!scroller || !content || typeof ResizeObserver === 'undefined') return
    const observer = new globalThis.ResizeObserver(() => {
      if (!stickToBottom.current) return
      scroller.scrollTop = scroller.scrollHeight
    })
    observer.observe(content)
    return () => observer.disconnect()
  }, [selected?.id])

  useEffect(() => {
    if (menu === 'info') return
    setMemberMenu(null)
    setMemberConfirm(null)
  }, [menu])

  useEffect(() => {
    if (!menu && !rowMenu && !bubbleMenu && !memberMenu) return
    const close = (event: globalThis.MouseEvent) => {
      if (memberConfirmRef.current) return
      const target = event.target as globalThis.Node
      for (const ref of [plusRef, emojiRef, infoPanelRef, moreRef, contextRef, bubbleMenuRef, memberMenuRef]) {
        if (ref.current?.contains(target)) return
      }
      setMenu(null)
      setRowMenu(null)
      setBubbleMenu(null)
      setMemberMenu(null)
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (memberMenu) {
        setMemberMenu(null)
        return
      }
      setMenu(null)
      setRowMenu(null)
      setBubbleMenu(null)
    }
    globalThis.document.addEventListener('mousedown', close)
    globalThis.document.addEventListener('keydown', onKey)
    return () => {
      globalThis.document.removeEventListener('mousedown', close)
      globalThis.document.removeEventListener('keydown', onKey)
    }
  }, [menu, rowMenu, bubbleMenu, memberMenu])

  const historyConversationId =
    historyOpen && selected && selected.kind !== 'transfer' && selected.kind !== 'boss' ? selected.id : null
  useEffect(() => {
    if (!historyConversationId) {
      setHistoryLoading(false)
      return
    }
    let live = true
    setHistoryLoading(true)
    void (async () => {
      for (let page = 0; page < 8; page += 1) {
        if (!live) return
        const current = store.getSnapshot().threads[historyConversationId]
        if (!current?.loaded || !current.hasOlder) break
        const loaded = await store.loadOlder(historyConversationId)
        if (!live || loaded <= 0) break
      }
      if (live) setHistoryLoading(false)
    })()
    return () => {
      live = false
    }
  }, [historyConversationId, store])

  useEffect(() => {
    const boss = selected?.kind === 'boss'
    if (!historyOpen || (selected?.kind !== 'transfer' && !boss)) {
      setHistoryAgentLines(null)
      return
    }
    let live = true
    setHistoryLoading(true)
    const sessionRef = boss ? tobossSessionRef : tobotSessionRef
    const sessionId = sessionRef.current
    void (async () => {
      const pages: AgentLine[][] = []
      let cursor: string | undefined
      try {
        for (let page = 0; page < 20; page += 1) {
          const loaded = await dataApiService.get(`/agent-sessions/${sessionId}/messages`, {
            query: cursor ? { limit: 200, cursor } : { limit: 200 }
          })
          if (!live || sessionRef.current !== sessionId) return
          pages.push(agentLinesFrom(loaded))
          const next =
            loaded && typeof loaded === 'object' ? (loaded as { nextCursor?: unknown }).nextCursor : undefined
          if (typeof next !== 'string' || next.length === 0) break
          cursor = next
        }
      } catch {
        // The dialog shows the pages already read.
      }
      if (!live || sessionRef.current !== sessionId) return
      setHistoryAgentLines(pages.reverse().flat())
      setHistoryLoading(false)
    })()
    return () => {
      live = false
    }
  }, [historyOpen, selected?.kind, tobotSessionId, tobossSessionId])

  useEffect(
    () => () => {
      if (focusTimer.current != null) globalThis.window.clearTimeout(focusTimer.current)
    },
    []
  )

  const fail = (error: unknown) => {
    if (error instanceof ImActionError && error.code === 'too_large') setNotice(t('messages.error.too_large'))
    else if (error instanceof ImActionError && error.code === 'empty') setNotice(t('messages.error.empty_file'))
    else if (planeRejected(error)) setNotice(t('messages.error.send_failed'))
    else {
      const message = error instanceof Error ? error.message : String(error)
      if (message === 'not_found' || /\bnot found\b/i.test(message)) setNotice(t('messages.error.not_found'))
      else if (message === 'forbidden' || message === 'unauthorized') setNotice(t('messages.error.forbidden'))
      else setNotice(t('messages.error.request', { error: message }))
    }
  }

  const decideApproval = (approved: boolean) => {
    const approval = tobotApproval
    if (!approval) return
    setTobotApproval(null)
    void ipcApi
      .request('ai.tool.respond_approval', {
        approvalId: approval.approvalId,
        approved,
        topicId: tobotTopic(tobotSessionRef.current)
      })
      .then((result) => {
        if (!result.ok) throw new Error('approval was not accepted')
      })
      .catch((error: unknown) => {
        setTobotApproval(approval)
        fail(error)
      })
  }

  const select = (id: string) => {
    setPicked(null)
    setForwardMode(null)
    setStagedFile(null)
    setStagedImage(null)
    composer.focus(id)
    setSelectedId(id)
    setMemberQuery('')
    setMenu(null)
    setRowMenu(null)
    setBubbleMenu(null)
    setHistoryOpen(false)
    setFocusId(null)
    if (focusTimer.current != null) {
      globalThis.window.clearTimeout(focusTimer.current)
      focusTimer.current = null
    }
    setNotice('')
    void store.open(id).then((opened) => {
      if (!opened || opened === id) return
      if (composer.active() === id) composer.handoff(id, opened)
      setSelectedId((current) => (current === id ? opened : current))
    }, fail)
  }
  selectRef.current = select

  const clearShots = (conversationId: string) => {
    composer.dropFiles(conversationId)
  }

  const stageFiles = (files: Iterable<globalThis.File>) => {
    if (!selected) return
    const list = [...files]
    if (list.length === 0) return
    const next: Array<{ id: string; file: globalThis.File; preview?: string }> = []
    for (const file of list) {
      if (file.size <= 0) {
        fail(new ImActionError('empty'))
        return
      }
      if (file.size > IM_ATTACHMENT_MAX_BYTES) {
        fail(new ImActionError('too_large'))
        return
      }
      next.push({
        id: `${file.name}-${file.size}-${Date.now()}-${next.length}`,
        file,
        preview: previewUrlFor(file)
      })
    }
    setNotice('')
    composer.writeFiles(selected.id, (current) => [...current, ...next])
    composerRef.current?.focus()
  }

  const stageScreenshot = (file: globalThis.File) => stageFiles([file])

  const loadQuotedAttachment = async (quote: MessageQuote): Promise<globalThis.File | null> => {
    if (quote.kind !== 'image' && quote.kind !== 'file') return null
    const line = shownAgentLines.find((item) => item.id === quote.messageId)
    const agentFile = line?.files.find((file) => (quote.kind === 'image' ? file.mediaType.startsWith('image/') : true))
    if (agentFile) {
      try {
        const blob = await readAgentFile(agentFile.path)
        return new globalThis.File([blob], agentFile.name || 'file', {
          type: agentFile.mediaType || blob.type || 'application/octet-stream'
        })
      } catch {
        return null
      }
    }
    const message = thread?.messages.find((item) => item.id === quote.messageId)
    if (!message || (message.body.kind !== 'image' && message.body.kind !== 'file')) return null
    if (message.body.kind !== quote.kind) return null
    try {
      return await store.attachmentFile(message.id, message.body.attachment.name, message.body.attachment.mime)
    } catch {
      return null
    }
  }

  const send = () => {
    if (!selected || speechBlock) return
    const conversationId = selected.id
    const text = draft.trim()
    const files = shots.map((shot) => shot.file)
    if (!text && files.length === 0) return
    for (const file of files) {
      if (file.size <= 0) {
        fail(new ImActionError('empty'))
        return
      }
      if (file.size > IM_ATTACHMENT_MAX_BYTES) {
        fail(new ImActionError('too_large'))
        return
      }
    }
    const outgoing = quoteDraft && text ? encodeQuotedText(text, quoteDraft) : text
    const quoted = quoteDraft
    setNotice('')
    composer.writeText(conversationId, '')
    if (text) composer.writeQuote(conversationId, null)
    clearShots(conversationId)
    stickToBottom.current = true
    if (selected.kind === 'transfer' || selected.kind === 'boss') {
      const restoreShots = () =>
        composer.writeFiles(
          conversationId,
          files.map((file) => ({ id: `${file.name}-${file.size}-restore`, file, preview: previewUrlFor(file) }))
        )
      void (async () => {
        const extra = quoted ? await loadQuotedAttachment(quoted) : null
        const list = extra ? [...files, extra] : files
        if (list.length > 0) {
          await sendAgentFiles(list, outgoing, () => {
            composer.writeText(conversationId, text)
            composer.writeQuote(conversationId, quoted)
            restoreShots()
          })
          return
        }
        postAgentTurn([{ type: 'text', text: outgoing }], userLine(outgoing), () => {
          composer.writeText(conversationId, text)
          composer.writeQuote(conversationId, quoted)
        })
      })()
      return
    }
    const mentionsLabel = (label: string, canonical: string) =>
      Boolean(text) &&
      (textMentionsBot(text, label, t('messages.mention.all')) ||
        (label !== canonical && textMentionsBot(text, canonical, t('messages.mention.all'))))
    const mentionsBot = mentionsLabel(botLabel, t('messages.file_transfer'))
    const mentionsBoss =
      Boolean(text) &&
      teamBotMentionLabels(bossLabel, t('messages.toboss')).some((label) => textMentionsNamedBot(text, label))
    const addressed = selected.kind === 'group' && personalBotIn(selected.id) && mentionsBot
    const hostBoss = selected.kind === 'group' && tobossGroups.includes(selected.id) && mentionsBoss && bossListed
    // History files stay with the personal ToBot. A team ToBot group turn carries only this send.
    const priorLoads = addressed
      ? recentThreadMedia(thread?.messages ?? [], files.length).map((source) =>
          store.attachmentFile(source.id, source.name, source.mime).then(
            (file) => file,
            () => null
          )
        )
      : []
    if (files.length > 0) {
      for (const file of files) void store.sendFile(conversationId, file).catch(fail)
    }
    if (!text) return
    const offerHost = () => {
      if (!hostBoss || !meId) return
      const sources = bossSources(conversationId, thread?.messages ?? [])
      const job: BossJob = {
        key: `${conversationId}:${meId}:${text}`,
        conversationId,
        senderId: meId,
        ask: text,
        prompt: bossRoomPrompt({
          room: titleOf(selected),
          asker: shownName(meId) || personName(snapshot, meId),
          ask: text,
          lines: bossTranscript(sources, bossLabel)
        }),
        files: files.slice(0, TOBOT_CONTEXT_CAP)
      }
      const result = offerSharedBossJob(job, Date.now())
      if (result.decision === 'soon') setNotice(t('messages.toboss.soon'))
      else if (result.decision === 'full') setNotice(t('messages.toboss.full'))
      else if (result.decision === 'queued' || result.decision === 'replaced') setNotice(t('messages.toboss.queued'))
      if (result.decision === 'start') runBossJob(job)
    }
    if (hostBoss && !addressed) {
      offerHost()
      void store.sendText(conversationId, outgoing).catch(fail)
      return
    }
    if (addressed || hostBoss) {
      composer.writeText(conversationId, '')
      const asked = new Set<ResidentAgent>(['tobot'])
      offerHost()
      setGroupReplies((current) => ({
        ...current,
        [conversationId]: [
          ...withoutAskedPending(current[conversationId] ?? [], text, asked),
          {
            id: `pending-${conversationId}-${text}`,
            prompt: text,
            text: '',
            status: 'pending',
            agent: 'tobot'
          }
        ]
      }))
      void store
        .sendText(conversationId, outgoing)
        .then(async () => {
          const loaded = (await Promise.all(priorLoads)).filter((file): file is globalThis.File => file !== null)
          const uploads = [...files.slice(0, TOBOT_CONTEXT_CAP), ...loaded].slice(0, TOBOT_CONTEXT_CAP)
          if (addressed) await askToBot(conversationId, text, uploads)
        })
        .catch((error: unknown) => {
          setGroupReplies((current) => ({
            ...current,
            [conversationId]: withoutAskedPending(current[conversationId] ?? [], text, asked)
          }))
          fail(error)
        })
      return
    }
    composer.writeText(conversationId, '')
    void store.sendText(conversationId, outgoing).catch(fail)
  }

  const residentTurnIsRunning = async (boss: boolean, sessionId?: string) => {
    const id = sessionId ?? (boss ? tobossSessionRef.current : tobotSessionRef.current)
    try {
      const body = await dataApiService.get(`/agent-sessions/${id}/turn`)
      return 'status' in body && body.status === 'attached'
    } catch {
      return false
    }
  }

  const postResidentTurn = async (
    boss: boolean,
    parts: Array<{ type: 'text'; text: string } | TobotFilePart>,
    extraDirectives: readonly string[] = [],
    room?: { sessionId: string; conversationId: string }
  ) => {
    const deadline = Date.now() + 90_000
    let lastError: unknown = new Error('Load failed')
    const host = boss ? bossHost() : tobotHost()
    const ensurePath = boss ? '/agent-sessions/toboss/ensure' : '/agent-sessions/tobot/ensure'
    const sessionRef = boss ? tobossSessionRef : tobotSessionRef
    const prompt = parts
      .filter((part): part is { type: 'text'; text: string } => part.type === 'text')
      .map((part) => part.text)
      .join('\n')
    const data = await prepareToBotDataTurn(prompt)
    const taskNote = (await workTurnNote()).slice(0, 480)
    const mailNote = (await mailTurnNote()).slice(0, 480)
    const accessNote = tobotAccessNote(profile?.phone || selfPortrait.phone || '').slice(0, 480)
    const directives = [...data.directives]
    for (const note of [taskNote, mailNote, accessNote, ...extraDirectives]) {
      if (!note) continue
      if (directives.length >= 8) directives.pop()
      directives.push(note)
    }
    pendingDataWrite.current = data.write ? { prompt, channel: data.write } : null
    let turnSession = room?.sessionId ?? sessionRef.current
    while (Date.now() <= deadline) {
      try {
        const ready = await dataApiService.post(ensurePath, {
          body: await withHarnessComponents(host ? { hostContext: host } : {})
        })
        const id = sessionIdFromEnsure(ready)
        if (id) adoptResident(boss, id)
        if (room) {
          const resolved = bossRoomSessionId(sessionRef.current, room.conversationId)
          if (!resolved) throw new Error('invalid session id')
          turnSession = resolved
          await dataApiService.post('/agent-sessions/toboss/room/ensure', {
            body: await withHarnessComponents({
              conversationId: room.conversationId,
              ...(host ? { hostContext: host } : {})
            })
          })
        } else {
          turnSession = sessionRef.current
        }
        await dataApiService.post(`/agent-sessions/${turnSession}/turns`, {
          body: await withHarnessComponents({
            userMessageParts: parts,
            ...(host ? { hostContext: host } : {}),
            ...(directives.length > 0 ? { turnContext: { directives } } : {})
          })
        })
        return
      } catch (error) {
        lastError = error
        if (!planeRejected(error) || !(await residentTurnIsRunning(boss, turnSession)) || Date.now() > deadline) break
        await wait(400)
      }
    }
    throw lastError
  }

  const postToBotTurn = (parts: Array<{ type: 'text'; text: string } | TobotFilePart>) => postResidentTurn(false, parts)

  const postAgentTurn = (
    parts: Array<{ type: 'text'; text: string } | TobotFilePart>,
    pending: AgentLine,
    restore?: () => void
  ) => {
    const boss = selectedKindRef.current === 'boss'
    pendingAgentLine.current = pending
    setAgentWaiting(true)
    setAgentLines((current) => withPendingUser(current, pending))
    const epoch = transcriptEpoch.current
    const deliver = async () => {
      try {
        await postResidentTurn(boss, parts)
        if (epoch !== transcriptEpoch.current) return
        const sessionId = (boss ? tobossSessionRef : tobotSessionRef).current
        const page = await dataApiService
          .get(`/agent-sessions/${sessionId}/messages`, { query: { limit: 50 } })
          .catch(() => null)
        if (page) settleAgentLines(page, epoch)
      } catch (error) {
        if (epoch !== transcriptEpoch.current) return
        if (pendingAgentLine.current?.id === pending.id) pendingAgentLine.current = null
        setAgentWaiting(false)
        setAgentLines((current) => current.filter((line) => line.id !== pending.id))
        restore?.()
        fail(error)
      }
    }
    void deliver()
  }

  const sendAgentFiles = async (list: globalThis.File[], caption = '', restore?: () => void) => {
    const parts: Array<{ type: 'text'; text: string } | TobotFilePart> = []
    try {
      for (const file of list) {
        const part = await uploadTobotFile(file)
        if (file.type.startsWith('image/')) {
          const path = pathFromFileUrl(part.url)
          const preview = previewUrlFor(file)
          if (preview) rememberPreview(path, preview)
        }
        parts.push(part)
      }
    } catch (error) {
      restore?.()
      fail(error)
      return
    }
    if (caption) parts.push({ type: 'text', text: caption })
    const files = parts.flatMap((part) => {
      if (part.type !== 'file') return []
      const path = pathFromFileUrl(part.url)
      return [
        {
          name: part.filename,
          mediaType: part.mediaType,
          path,
          sizeBytes: part.sizeBytes,
          previewUrl: previewFor(path)
        }
      ]
    })
    postAgentTurn(parts, userLine(caption, files), restore)
  }

  const onPaste = (event: ReactClipboardEvent<globalThis.HTMLTextAreaElement>) => {
    const files = [...event.clipboardData.files]
    if (files.length === 0) return
    event.preventDefault()
    stageFiles(files)
  }

  const onThreadScroll = () => {
    const element = threadRef.current
    if (!element || !selected) return
    stickToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80
    if (element.scrollTop > 40 || loadingOlder || !thread?.hasOlder) return
    const before = element.scrollHeight
    setLoadingOlder(true)
    store
      .loadOlder(selected.id)
      .then(() =>
        globalThis.requestAnimationFrame(() => {
          element.scrollTop += element.scrollHeight - before
        })
      )
      .catch(fail)
      .finally(() => setLoadingOlder(false))
  }

  const settings = (action: () => Promise<void>) => {
    setMenu(null)
    setRowMenu(null)
    setBubbleMenu(null)
    void action().catch(fail)
  }

  const applyInPanel = (action: () => Promise<void>) => {
    void action().catch(fail)
  }

  const openBubbleMenu = (
    event: ReactMouseEvent,
    text: string,
    recall: { conversationId: string; messageId: string } | null = null,
    quote: MessageQuote | null = null,
    selectId: string | null = null,
    deleteId: string | null = selectId
  ) => {
    event.preventDefault()
    event.stopPropagation()
    setMenu(null)
    setRowMenu(null)
    setBubbleMenu({ x: event.clientX, y: event.clientY, text, recall, quote, selectId, deleteId })
  }

  const deleteBubble = (id: string) => {
    if (!selected || !id) return
    const conversationId = selected.id
    if (quoteDraft?.messageId === id) composer.writeQuote(conversationId, null)
    setPicked((current) => (current ? current.filter((item) => item !== id) : current))
    if (thread?.pending.some((item) => item.clientMsgId === id)) {
      store.discardPending(conversationId, id)
      return
    }
    if (thread?.messages.some((message) => message.id === id)) {
      store.dismiss(conversationId, id)
      return
    }
    const groupLine = (groupReplies[conversationId] ?? []).find((line) => line.id === id)
    const sessionKind = groupLine?.agent ?? (selected.kind === 'boss' ? 'toboss' : 'tobot')
    hiddenAgentIds.current.add(id)
    if (pendingAgentLine.current?.id === id) pendingAgentLine.current = null
    setAgentLines((current) => current.filter((line) => line.id !== id))
    setHistoryAgentLines((current) => (current ? current.filter((line) => line.id !== id) : current))
    setGroupReplies((current) => {
      let changed = false
      const next: Record<string, GroupReply[]> = {}
      for (const [key, lines] of Object.entries(current)) {
        const kept = lines.filter((line) => line.id !== id)
        if (kept.length !== lines.length) changed = true
        if (kept.length > 0) next[key] = kept
      }
      return changed ? next : current
    })
    const rest = agentLines.filter((line) => line.id !== id)
    const digest = latestAgentDigest(selected.kind === 'boss' ? omitBossRoomTurns(rest) : rest)
    if (selected.kind === 'boss') setSavedBossDigest(digest)
    else if (selected.kind === 'transfer') setSavedDigest(digest)
    if (id.startsWith('local-') || id.startsWith('pending-')) return
    const sessionId = sessionKind === 'toboss' ? tobossSessionRef.current : tobotSessionRef.current
    void dataApiService.delete(`/agent-sessions/${sessionId}/messages/${id}`).catch((error: unknown) => {
      hiddenAgentIds.current.delete(id)
      fail(error)
      refreshTobotRef.current()
    })
  }

  const togglePick = (id: string) => {
    setPicked((current) => {
      if (!current) return current
      return current.includes(id) ? current.filter((item) => item !== id) : [...current, id]
    })
  }

  const forwardChats = snapshot.conversations.flatMap<ForwardChat>((entry) => {
    if (entry.id === selected?.id) return []
    if (entry.kind !== 'direct' && entry.kind !== 'group' && entry.kind !== 'transfer') return []
    const title = titleOf(entry)
    if (entry.kind === 'group') {
      return [{ id: entry.id, title, tobot: false, kind: 'group', name: title, faces: groupFacesOf(entry) }]
    }
    if (entry.kind === 'transfer') return [{ id: entry.id, title, tobot: true, kind: 'transfer', name: title }]
    const portrait = portraitFor(entry.peerUserId, title)
    return [
      {
        id: entry.id,
        title,
        tobot: false,
        kind: 'direct',
        name: portrait.name || title,
        phone: portrait.phone,
        avatar: portrait.avatar,
        colorKey: entry.peerUserId ?? undefined
      }
    ]
  })

  const forwardTo = async (choice: ForwardChoice): Promise<boolean> => {
    if (!picked || picked.length === 0 || !forwardMode) return false
    const chosen = new Set(picked)
    type ForwardFile =
      | { source: 'im'; id: string; name: string; mime: string | null }
      | { source: 'agent'; path: string; name: string; mime: string }
    const pieces: Array<{ at: number; sender: string; text: string; labelOnly: boolean; files: ForwardFile[] }> = []
    const senderOf = (message: ImMessage) => {
      if (message.body.kind === 'text' && decodeBossWire(message.body.text)?.kind === 'reply') return bossLabel
      return message.sender.kind === 'service' ? t('messages.system_sender') : personName(snapshot, message.sender.id)
    }
    const mineName = portraitFor(meId, knownName(meId)).name || knownName(meId) || t('messages.me')
    for (const message of thread?.messages ?? []) {
      if (!chosen.has(message.id) || message.recalled) continue
      const sender = senderOf(message)
      if (message.body.kind === 'text') {
        const record = parseChatRecord(message.body.text)
        const text = record ? record.title : visibleText(message.body.text).trim()
        if (text) pieces.push({ at: message.createdAt, sender, text, labelOnly: false, files: [] })
      } else if (message.body.kind === 'image' || message.body.kind === 'file') {
        pieces.push({
          at: message.createdAt,
          sender,
          text:
            message.body.kind === 'image'
              ? t('messages.preview.image')
              : `${t('messages.preview.file')} ${message.body.attachment.name}`.trim(),
          labelOnly: true,
          files: [
            {
              source: 'im',
              id: message.id,
              name: message.body.attachment.name,
              mime: message.body.attachment.mime
            }
          ]
        })
      } else if (message.body.kind === 'voice') {
        pieces.push({ at: message.createdAt, sender, text: t('messages.preview.voice'), labelOnly: false, files: [] })
      }
    }
    for (const line of shownAgentLines) {
      if (!chosen.has(line.id)) continue
      const sender = line.role === 'user' ? mineName : selected?.kind === 'boss' ? bossLabel : botLabel
      const text = visibleText(line.text).trim()
      const files: ForwardFile[] = line.files.map((file) => ({
        source: 'agent',
        path: file.path,
        name: file.name,
        mime: file.mediaType
      }))
      if (!text && files.length === 0) continue
      const label = files
        .map((file) =>
          file.mime?.startsWith('image/')
            ? t('messages.preview.image')
            : `${t('messages.preview.file')} ${file.name}`.trim()
        )
        .join(' ')
      pieces.push({ at: line.at || Date.now(), sender, text: text || label, labelOnly: !text, files })
    }
    for (const line of shownGroupReplies) {
      if (!chosen.has(line.id)) continue
      const text = visibleText(line.text).trim()
      const sender = (line.agent ?? 'tobot') === 'toboss' ? bossLabel : botLabel
      if (text) pieces.push({ at: Date.now(), sender, text, labelOnly: false, files: [] })
    }
    pieces.sort((left, right) => left.at - right.at)
    if (pieces.length === 0) return false
    const loadFile = async (source: ForwardFile): Promise<globalThis.File | null> => {
      try {
        if (source.source === 'im') return await store.attachmentFile(source.id, source.name, source.mime)
        const blob = await readAgentFile(source.path)
        return new globalThis.File([blob], source.name || 'file', {
          type: source.mime || blob.type || 'application/octet-stream'
        })
      } catch {
        return null
      }
    }
    const lines = pieces.flatMap((piece) =>
      piece.text.trim() ? [{ sender: piece.sender, text: piece.text.trim() }] : []
    )
    const recordTitle = !selected
      ? t('messages.forward_record')
      : selected.kind === 'group'
        ? t('messages.forward_record_group', { name: titleOf(selected) })
        : t('messages.forward_record_pair', {
            left: mineName,
            right: selected.kind === 'boss' ? bossLabel : selected.kind === 'transfer' ? botLabel : titleOf(selected)
          })
    try {
      let targetId = ''
      let tobot = false
      if (choice.kind === 'person') {
        const existing = snapshot.conversations.find(
          (entry) => entry.kind === 'direct' && entry.peerUserId === choice.userId
        )
        targetId = existing?.id ?? (await store.startDirect(choice.userId))
      } else {
        targetId = choice.id
        tobot = choice.tobot
      }
      if (tobot) {
        if (forwardMode === 'merge') {
          const parts: Array<{ type: 'text'; text: string } | TobotFilePart> = []
          for (const piece of pieces) {
            for (const source of piece.files) {
              const file = await loadFile(source)
              if (file) parts.push(await uploadTobotFile(file))
            }
          }
          const body = lines.map((line) => `${line.sender}: ${line.text}`).join('\n')
          parts.push({ type: 'text', text: `${t('messages.forward_transcript', { title: recordTitle })}\n${body}` })
          await postToBotTurn(parts)
        } else {
          for (const piece of pieces) {
            const parts: Array<{ type: 'text'; text: string } | TobotFilePart> = []
            let uploaded = 0
            for (const source of piece.files) {
              const file = await loadFile(source)
              if (!file) continue
              parts.push(await uploadTobotFile(file))
              uploaded += 1
            }
            if (piece.text && (!piece.labelOnly || uploaded === 0)) parts.push({ type: 'text', text: piece.text })
            if (parts.length > 0) await postToBotTurn(parts)
          }
        }
      } else if (forwardMode === 'merge') {
        await store.sendText(targetId, encodeChatRecord(recordTitle, lines))
      } else {
        for (const piece of pieces) {
          let sentFile = false
          for (const source of piece.files) {
            const file = await loadFile(source)
            if (!file) continue
            await store.sendFile(targetId, file)
            sentFile = true
          }
          if (piece.text && (!piece.labelOnly || !sentFile)) await store.sendText(targetId, piece.text)
        }
      }
      setPicked(null)
      setForwardMode(null)
      return true
    } catch (error) {
      fail(error)
      return false
    }
  }

  const copyBubbleText = (text: string) => {
    setBubbleMenu(null)
    const write = globalThis.navigator.clipboard?.writeText
    if (!write) {
      setNotice(t('common.copy_failed'))
      return
    }
    void write.call(globalThis.navigator.clipboard, text).catch(() => setNotice(t('common.copy_failed')))
  }

  const setToBot = (conversationId: string, present: boolean) => {
    setToBotGroups((current) => {
      const next = present ? [...new Set([...current, conversationId])] : current.filter((id) => id !== conversationId)
      const orgId = snapshot.orgId
      if (orgId)
        globalThis.localStorage.setItem(tobotGroupStorageKey(orgId, tobotSessionRef.current), JSON.stringify(next))
      return next
    })
  }

  const rememberBoss = (conversationId: string, present: boolean) => {
    setToBossGroups((current) => {
      if (current.includes(conversationId) === present) return current
      const next = present ? [...current, conversationId] : current.filter((id) => id !== conversationId)
      const orgId = snapshot.orgId
      if (orgId) writeBossGroups(orgId, tobossSessionRef.current, next)
      return next
    })
  }

  const setToBoss = (conversationId: string, present: boolean) => {
    rememberBoss(conversationId, present)
    if (!bossListed) return
    void store.sendText(conversationId, encodeBossWire('', present ? 'join' : 'leave')).catch(fail)
  }

  const createGroup = async (userIds: string[], name: string) => {
    const members = userIds.filter((id) => id !== BUILTIN_TOBOT_ID && id !== BUILTIN_TOBOSS_ID)
    const fallback = [snapshot.me?.displayName ?? '', ...members.map((id) => personName(snapshot, id))]
      .filter(Boolean)
      .join(t('messages.list_separator'))
    const title = (name || fallback).slice(0, 64)
    const id = await store.createGroup(title, members)
    select(id)
  }

  const askResident = (
    boss: boolean,
    conversationId: string,
    text: string,
    files: globalThis.File[] = [],
    modelText?: string
  ): Promise<string | undefined> => {
    const posted = modelText || text
    const epoch = transcriptEpoch.current
    const agent: ResidentAgent = boss ? 'toboss' : 'tobot'
    const sessionRef = boss ? tobossSessionRef : tobotSessionRef
    // A group turn is still pending on the first read. Wait until the answer is saved.
    const waitForReply = boss && modelText !== undefined
    const room = waitForReply ? bossRoomSessionId(sessionRef.current, conversationId) : null
    if (waitForReply && !room) return Promise.resolve(undefined)
    return Promise.all(files.map((file) => uploadTobotFile(file)))
      .then((fileParts) =>
        postResidentTurn(
          boss,
          [...fileParts, { type: 'text', text: posted }],
          boss && modelText ? [BOSS_ROOM_DIRECTIVE] : [],
          room ? { sessionId: room, conversationId } : undefined
        )
      )
      .then(async () => {
        const deadline = Date.now() + 180_000
        let idle = 0
        for (;;) {
          if (epoch !== transcriptEpoch.current) return undefined
          const page = await dataApiService
            .get(`/agent-sessions/${room ?? sessionRef.current}/messages`, { query: { limit: 50 } })
            .catch(() => null)
          if (epoch !== transcriptEpoch.current) return undefined
          const reply = page ? assistantAfter(agentLinesFrom(page), userLine(posted)) : undefined
          if (reply?.status === 'success' && reply.text.trim()) {
            setGroupReplies((current) => ({
              ...current,
              [conversationId]: (current[conversationId] ?? [])
                .filter(
                  (line) =>
                    line.id !== reply.id &&
                    !(
                      (line.modelText || line.prompt) === posted &&
                      line.status === 'pending' &&
                      (line.agent ?? 'tobot') === agent
                    )
                )
                .concat({
                  id: reply.id,
                  prompt: text,
                  modelText: posted,
                  text: reply.text,
                  status: reply.status,
                  parts: reply.parts,
                  agent
                })
            }))
            return reply.text
          }
          if (reply && reply.status !== 'pending') return undefined
          if (!waitForReply || Date.now() > deadline) return undefined
          idle = (await residentTurnIsRunning(boss, room ?? undefined)) ? 0 : idle + 1
          if (idle >= 2) return undefined
          await wait(400)
        }
      })
  }

  const askToBot = (conversationId: string, text: string, files: globalThis.File[] = []) =>
    askResident(false, conversationId, text, files)

  const askToBoss = (conversationId: string, text: string, files: globalThis.File[] = [], modelText?: string) =>
    askResident(true, conversationId, text, files, modelText)

  const bossSources = (conversationId: string, messages: readonly ImMessage[]): BossRoomMessage[] =>
    messages.map((message) => {
      const senderId = message.sender.kind === 'member' ? message.sender.id : ''
      const named =
        conversationId === selected?.id
          ? shownName(senderId) || personName(snapshot, senderId)
          : personName(snapshot, senderId)
      return {
        id: message.id,
        createdAt: message.createdAt,
        recalled: message.recalled,
        senderId,
        senderName: message.sender.kind === 'service' ? t('messages.system_sender') : named,
        text: message.body.kind === 'text' ? message.body.text : null
      }
    })

  const runBossJob = (job: BossJob) => {
    const conversationId = job.conversationId
    setGroupReplies((current) => ({
      ...current,
      [conversationId]: [
        ...withoutAskedPending(current[conversationId] ?? [], job.ask, new Set<ResidentAgent>(['toboss'])),
        {
          id: `pending-${conversationId}-${job.ask}-toboss`,
          prompt: job.ask,
          modelText: job.prompt,
          text: '',
          status: 'pending',
          agent: 'toboss'
        }
      ]
    }))
    void (async () => {
      let answered = false
      try {
        const reply = await askToBoss(conversationId, job.ask, job.files ?? [], job.prompt)
        const answer = reply?.trim() ?? ''
        if (!answer) return
        try {
          await store.sendText(conversationId, encodeBossWire(answer, 'reply'))
          answered = true
          setGroupReplies((current) => ({
            ...current,
            [conversationId]: (current[conversationId] ?? []).filter(
              (line) => !(line.agent === 'toboss' && line.prompt === job.ask)
            )
          }))
        } catch (error) {
          fail(error)
        }
      } catch (error) {
        fail(error)
      } finally {
        if (!answered) {
          setGroupReplies((current) => ({
            ...current,
            [conversationId]: (current[conversationId] ?? []).map((line) =>
              line.agent === 'toboss' && line.prompt === job.ask && line.status === 'pending'
                ? { ...line, status: 'error' }
                : line
            )
          }))
        }
        const next = finishSharedBossJob(Date.now(), answered)
        if (next) runBossJob(next)
      }
    })()
  }
  runBossJobRef.current = runBossJob

  useLayoutEffect(() => {
    registerBossJobRunner((job) => runBossJobRef.current(job))
    return () => registerBossJobRunner(null)
  }, [])

  const bossLabels = bossLabel === t('messages.toboss') ? [bossLabel] : [bossLabel, t('messages.toboss')]
  const bossWarmKey = snapshot.conversations
    .filter((entry) =>
      bossRoomNeedsWarm(
        {
          kind: entry.kind,
          previewText: entry.preview && !entry.preview.recalled ? entry.preview.text : null
        },
        Boolean(snapshot.threads[entry.id]?.loaded),
        tobossGroups,
        bossLabels,
        entry.id
      )
    )
    .slice(0, BOSS_WARM_CAP)
    .map((entry) => entry.id)
    .join('\n')

  useEffect(() => {
    if (!bossListed || !bossWarmKey) return
    const ids = bossWarmKey.split('\n')
    let cancelled = false
    void (async () => {
      for (const id of ids) {
        if (cancelled) return
        await store.preload(id).catch(() => undefined)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [bossListed, bossWarmKey, store])

  useEffect(() => {
    const labels = teamBotMentionLabels(bossLabel, t('messages.toboss'))
    const rooms: Array<{ id: string; title: string; messages: BossRoomMessage[] }> = []
    for (const [conversationId, loaded] of Object.entries(snapshot.threads)) {
      if (!loaded.loaded) continue
      const entry = snapshot.conversations.find((item) => item.id === conversationId)
      if (!entry || entry.kind !== 'group') continue
      rooms.push({
        id: conversationId,
        title: titleOf(entry),
        messages: bossSources(conversationId, loaded.messages)
      })
    }
    const plan = acceptBossCatchUp({
      now: Date.now(),
      labels,
      bossName: bossLabel,
      bossListed,
      memberGroupIds: tobossGroups,
      rooms
    })
    for (const item of plan.membership) rememberBoss(item.id, item.present)
    for (const job of plan.started) runBossJob(job)
  }, [bossLabel, bossListed, snapshot.conversations, snapshot.threads, tobossGroups])

  const startChat = async (userId: string) => {
    select(await store.startDirect(userId))
  }

  const timeLabel = (atTime: number) =>
    formatConversationTime(atTime, now, t('messages.time.yesterday'), t('messages.time.just_now'))
  const headerTitle = selected
    ? selected.kind === 'group'
      ? `${titleOf(selected)} (${(detail ? activeMembers.length : selected.memberCount) + (personalBotIn(selected.id) ? 1 : 0) + (tobossGroups.includes(selected.id) ? 1 : 0)})`
      : titleOf(selected)
    : t('messages.title')
  const mePortrait = portraitFor(meId, knownName(meId))
  const selectedPeer =
    selected?.kind === 'direct' ? portraitFor(selected.peerUserId, knownName(selected.peerUserId)) : null

  const renderConversation = (entry: ImInboxEntry) => {
    const active = entry.id === selectedId
    const title = titleOf(entry)
    const peer = entry.kind === 'direct' ? portraitFor(entry.peerUserId, knownName(entry.peerUserId)) : null
    const agent = entry.kind === 'transfer' ? tobotDigest : entry.kind === 'boss' ? bossDigest : null
    const summary = agent?.text || previewText(entry, snapshot, t)
    const at = agent?.at || entry.preview?.createdAt || entry.updatedAt
    return (
      <button
        key={entry.id}
        type="button"
        data-ui="messages.conversation"
        data-conversation-id={entry.id}
        aria-current={active ? 'page' : undefined}
        onContextMenu={(event) => {
          event.preventDefault()
          setMenu(null)
          setBubbleMenu(null)
          setRowMenu({ id: entry.id, x: event.clientX, y: event.clientY })
        }}
        onClick={() => select(entry.id)}
        className={cn(
          'flex min-h-[68px] w-full items-center gap-2.5 px-3 py-2 text-left outline-none',
          'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
          active ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
        )}>
        <ConversationAvatar
          entry={entry}
          title={peer?.name || title}
          productLogo={productLogo}
          faces={entry.kind === 'group' ? groupFacesOf(entry) : undefined}
          phone={peer?.phone}
          avatar={peer?.avatar}
          colorKey={entry.peerUserId ?? undefined}
          online={entry.kind === 'direct' && personOnline(entry.peerUserId)}
        />
        <span className="grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)_auto] items-start gap-x-2 gap-y-0.5">
          <span
            data-ui="messages.conversation-title"
            title={title}
            className="min-w-0 truncate text-[14px] leading-5 font-medium text-[#171717]">
            {title}
          </span>
          <span className="flex shrink-0 items-center gap-1 pt-0.5 text-[12px] leading-5 font-normal text-[#8a8a8a]">
            {entry.pinned ? <Pin className="size-3 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden /> : null}
            {at ? <span data-ui="messages.conversation-time">{timeLabel(at)}</span> : null}
          </span>
          <span
            data-ui="messages.conversation-preview"
            className="flex min-w-0 text-[12px] leading-4 font-normal text-[#8a8a8a]">
            {youWereMentioned(entry) ? (
              <span data-ui="messages.mention-you" className="mr-1 shrink-0 text-[#E23B3B]">
                {t('messages.mention.you')}
              </span>
            ) : null}
            <span className="min-w-0 truncate">{summary}</span>
          </span>
          <span className="flex shrink-0 items-center justify-end gap-1">
            {entry.muted ? <BellOff className="size-3.5 shrink-0 text-[#b0b0b0]" strokeWidth={1.7} /> : null}
            {entry.unread > 0 ? (
              <span
                data-ui="messages.unread-count"
                className={cn(
                  'flex h-4 min-w-4 shrink-0 items-center justify-center rounded-full px-1 text-[10px] leading-none font-medium text-white',
                  entry.muted ? 'bg-[#b0b0b0]' : 'bg-[#E23B3B]'
                )}>
                {entry.unread > 99 ? '99+' : entry.unread}
              </span>
            ) : null}
          </span>
        </span>
      </button>
    )
  }

  const renderStatus = (row: Row) => {
    if (row.kind === 'pending') {
      if (row.pending.state === 'sending') {
        return (
          <p data-ui="messages.status" className="m-0 mt-1 text-[11px] text-[#8a8a8a]">
            {t('messages.status.sending')}
          </p>
        )
      }
      return (
        <p data-ui="messages.status" className="m-0 mt-1 flex gap-2 text-[11px] text-[#E23B3B]">
          <span>{t('messages.status.failed')}</span>
          <button
            type="button"
            className="underline outline-none"
            onClick={() => selected && void store.retrySend(selected.id, row.pending.clientMsgId).catch(fail)}>
            {t('messages.retry')}
          </button>
          <button
            type="button"
            className="text-[#8a8a8a] underline outline-none"
            onClick={() => selected && store.discardPending(selected.id, row.pending.clientMsgId)}>
            {t('messages.discard')}
          </button>
        </p>
      )
    }
    const state = readState(row.message, detail, meId)
    if (!state) return null
    const label =
      state.kind === 'unread_count'
        ? t('messages.status.unread_count', { count: state.count })
        : state.kind === 'read'
          ? t('messages.status.read')
          : t('messages.status.unread')
    return (
      <p data-ui="messages.status" className="m-0 mt-1 text-[11px] text-[#8a8a8a]">
        {label}
      </p>
    )
  }

  const renderRow = (row: Row, index: number) => {
    const divider = showTimeDivider(
      rows.map((item) => item.at),
      index
    )
    const message = row.kind === 'message' ? row.message : null
    const rawText =
      message?.body.kind === 'text'
        ? message.body.text
        : row.kind === 'pending' && row.pending.kind === 'text'
          ? row.pending.text
          : null
    const bossWire = rawText ? decodeBossWire(rawText) : null
    const bossReply = bossWire?.kind === 'reply'
    const mine =
      !bossReply && (row.kind === 'pending' || (message?.sender.kind === 'member' && message.sender.id === meId))
    let content: ReactNode
    if (bossWire?.kind === 'join' || bossWire?.kind === 'leave') {
      content = (
        <p data-ui="messages.toboss-membership" className="m-0 text-center text-[12px] text-[#8a8a8a]">
          {t(bossWire.kind === 'join' ? 'messages.toboss.joined' : 'messages.toboss.left')}
        </p>
      )
    } else if (message?.body.kind === 'system') {
      content = (
        <p className="m-0 text-center text-[12px] text-[#8a8a8a]">
          {systemNotice(message.body.code, message.body.params, snapshot, t)}
        </p>
      )
    } else if (message?.recalled) {
      content = (
        <p className="m-0 text-center text-[12px] text-[#8a8a8a]">{recallNotice(message.sender, snapshot, t)}</p>
      )
    } else {
      const fromName = bossReply
        ? bossLabel
        : message
          ? shownName(message.sender.id) || personName(snapshot, message.sender.id)
          : (snapshot.me?.displayName ?? '')
      const speaker = portraitFor(
        mine ? meId : message?.sender.id,
        mine ? knownName(meId) : knownName(message?.sender.id) || fromName
      )
      content = (
        <div className={cn('group/message flex items-start gap-2', mine && 'justify-end')}>
          {mine ? null : bossReply ? (
            <ToBossMark size={36} label={bossLabel} />
          ) : (
            <PersonAvatar
              name={speaker.name}
              phone={speaker.phone}
              avatar={speaker.avatar}
              colorKey={message?.sender.id}
            />
          )}
          <div
            className={cn(
              'flex min-w-0 flex-col',
              bossReply ? 'w-full max-w-[70%]' : 'max-w-[70%]',
              mine ? 'items-end' : 'items-start'
            )}>
            {!mine && selected?.kind === 'group' ? (
              <p className="m-0 mb-1 text-[12px] text-[#8a8a8a]">{fromName}</p>
            ) : null}
            <div className={cn('flex min-w-0 items-end gap-2', bossReply && 'w-full', mine && 'flex-row-reverse')}>
              {bossReply && message && bossWire ? (
                <AgentMarkdown
                  id={message.id}
                  text={bossWire.text}
                  streaming={false}
                  onContextMenu={(event) =>
                    openBubbleMenu(
                      event,
                      bossWire.text,
                      canRecall(message, meId, now) && selected
                        ? { conversationId: selected.id, messageId: message.id }
                        : null,
                      quoteFor(message),
                      message.id
                    )
                  }
                />
              ) : message ? (
                <MessageBody
                  store={store}
                  message={message}
                  mine={mine}
                  mentions={selected?.kind === 'group' ? mentionLabels : undefined}
                  withdrawnIds={withdrawnIds}
                  onQuoteOpen={revealMessage}
                  onTextContextMenu={(event, text) =>
                    openBubbleMenu(
                      event,
                      text,
                      canRecall(message, meId, now) && selected
                        ? { conversationId: selected.id, messageId: message.id }
                        : null,
                      quoteFor(message),
                      message.id
                    )
                  }
                />
              ) : row.kind === 'pending' ? (
                <PendingBody
                  store={store}
                  pending={row.pending}
                  mine={mine}
                  mentions={selected?.kind === 'group' ? mentionLabels : undefined}
                  withdrawnIds={withdrawnIds}
                  onQuoteOpen={revealMessage}
                  onTextContextMenu={
                    row.pending.kind === 'text'
                      ? (event, text) => openBubbleMenu(event, text, null, null, null, row.pending.clientMsgId)
                      : undefined
                  }
                />
              ) : null}
              {message && canRecall(message, meId, now) ? (
                <button
                  type="button"
                  data-ui="messages.recall"
                  onClick={() => selected && void store.recall(selected.id, message.id).catch(fail)}
                  className="shrink-0 text-[11px] text-[#8a8a8a] opacity-0 outline-none group-hover/message:opacity-100 hover:text-[#171717] focus-visible:opacity-100">
                  {t('messages.recall_action')}
                </button>
              ) : null}
            </div>
            {mine ? renderStatus(row) : null}
          </div>
          {mine ? (
            <PersonAvatar name={speaker.name} phone={speaker.phone} avatar={speaker.avatar} colorKey={meId} />
          ) : null}
        </div>
      )
    }
    const quotedMessage =
      message && !message.recalled && message.body.kind !== 'system' && message.body.kind !== 'text' ? message : null
    const openQuoted = (target: ImMessage) => (event: ReactMouseEvent) => {
      const quote = quoteFor(target)
      if (!quote || !selected) return
      openBubbleMenu(
        event,
        quote.excerpt,
        canRecall(target, meId, now) ? { conversationId: selected.id, messageId: target.id } : null,
        quote,
        target.id
      )
    }
    const canPick = Boolean(
      message &&
      !message.recalled &&
      message.body.kind !== 'system' &&
      message.body.kind !== 'recalled' &&
      bossWire?.kind !== 'join' &&
      bossWire?.kind !== 'leave'
    )
    const pickLabel = message ? (quoteFor(message)?.excerpt ?? message.id) : ''
    const toggleRow = (event: ReactMouseEvent) => {
      if (!picked || !message) return
      const target = event.target
      if (target instanceof globalThis.Element && target.closest('button, a')) return
      togglePick(message.id)
    }
    return (
      <div
        key={row.key}
        data-ui="messages.row"
        data-message-id={message?.id}
        data-message-focus={message && focusId === message.id ? 'true' : undefined}
        className={cn('flex flex-col gap-3', message && focusId === message.id && 'rounded-[8px] bg-[#f7f7f7]')}
        onClick={picked && canPick ? toggleRow : undefined}
        onContextMenu={quotedMessage ? openQuoted(quotedMessage) : undefined}>
        {divider ? <p className="m-0 text-center text-[12px] text-[#b0b0b0]">{timeLabel(row.at)}</p> : null}
        <div className="flex items-start gap-2">
          {picked && canPick && message ? (
            <PickBox checked={picked.includes(message.id)} label={pickLabel} onToggle={() => togglePick(message.id)} />
          ) : null}
          <div className="min-w-0 flex-1">{content}</div>
        </div>
      </div>
    )
  }

  const onResizeStart = (event: ReactMouseEvent) => {
    event.preventDefault()
    const startX = event.clientX
    const startWidth = listWidth
    const pageWidth = pageRef.current?.clientWidth ?? 0
    const maxWidth = pageWidth > 0 ? Math.max(220, pageWidth - 320) : 560
    const move = (moveEvent: globalThis.MouseEvent) =>
      setListWidth(Math.min(maxWidth, Math.max(220, startWidth + (moveEvent.clientX - startX))))
    const up = () => {
      globalThis.document.removeEventListener('mousemove', move)
      globalThis.document.removeEventListener('mouseup', up)
      globalThis.document.body.style.cursor = ''
      globalThis.document.body.style.userSelect = ''
    }
    globalThis.document.body.style.cursor = 'col-resize'
    globalThis.document.body.style.userSelect = 'none'
    globalThis.document.addEventListener('mousemove', move)
    globalThis.document.addEventListener('mouseup', up)
  }

  const onComposerResizeStart = (event: ReactMouseEvent) => {
    event.preventDefault()
    const startY = event.clientY
    const startHeight = composerHeight
    const pageHeight = pageRef.current?.clientHeight ?? 0
    const maxHeight = pageHeight > 0 ? Math.max(72, pageHeight - 220) : 360
    const move = (moveEvent: globalThis.MouseEvent) =>
      setComposerHeight(Math.min(maxHeight, Math.max(72, startHeight + (startY - moveEvent.clientY))))
    const up = () => {
      globalThis.document.removeEventListener('mousemove', move)
      globalThis.document.removeEventListener('mouseup', up)
      globalThis.document.body.style.cursor = ''
      globalThis.document.body.style.userSelect = ''
    }
    globalThis.document.body.style.cursor = 'ns-resize'
    globalThis.document.body.style.userSelect = 'none'
    globalThis.document.addEventListener('mousemove', move)
    globalThis.document.addEventListener('mouseup', up)
  }

  const menuItem =
    'flex h-8 w-full items-center gap-2 rounded-[6px] px-2 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f5f5f5]'
  const openDigest = useMemo(() => latestAgentDigest(shownAgentLines), [shownAgentLines])
  useEffect(() => {
    if (!openDigest || selected?.kind !== 'transfer') return
    setSavedDigest((current) =>
      current && current.text === openDigest.text && current.at === openDigest.at ? current : openDigest
    )
  }, [openDigest, selected?.kind])
  useEffect(() => {
    if (!openDigest || selected?.kind !== 'boss') return
    setSavedBossDigest((current) =>
      current && current.text === openDigest.text && current.at === openDigest.at ? current : openDigest
    )
  }, [openDigest, selected?.kind])
  const tobotDigest = selected?.kind === 'transfer' ? (openDigest ?? savedDigest) : savedDigest
  const bossDigest = selected?.kind === 'boss' ? (openDigest ?? savedBossDigest) : savedBossDigest
  const groupReplyNodes =
    selected?.kind === 'group'
      ? shownGroupReplies.map((line) => (
          <div
            key={line.id}
            data-ui="messages.row"
            data-message-id={line.id}
            data-message-focus={focusId === line.id ? 'true' : undefined}
            data-agent-status={line.status}
            className={cn('flex flex-col gap-3', focusId === line.id && 'rounded-[8px] bg-[#f7f7f7]')}>
            <div className="flex items-start gap-2">
              {line.agent === 'toboss' ? (
                <ToBossMark size={36} label={bossLabel} animated={line.status === 'pending'} />
              ) : (
                <ToBotMark profile={tobotProfile} size={36} label={botLabel} />
              )}
              {line.text || (line.parts?.length ?? 0) > 0 ? (
                <div className="flex w-full max-w-[70%] min-w-0 flex-col items-start gap-2">
                  <TobotTurnView
                    parts={line.parts ?? []}
                    text={line.text}
                    streaming={line.status === 'pending'}
                    onContextMenu={(event) => {
                      const shown = visibleText(line.text)
                      openBubbleMenu(
                        event,
                        shown,
                        null,
                        shown.trim()
                          ? makeQuote(
                              line.id,
                              line.agent === 'toboss' ? BUILTIN_TOBOSS_ID : BUILTIN_TOBOT_ID,
                              line.agent === 'toboss' ? bossLabel : botLabel,
                              'text',
                              shown
                            )
                          : null,
                        line.id
                      )
                    }}
                  />
                  {line.status === 'pending' && tobotApproval ? (
                    <TobotApprovalCard
                      toolName={tobotApproval.toolName}
                      onAllow={() => decideApproval(true)}
                      onDeny={() => decideApproval(false)}
                    />
                  ) : null}
                </div>
              ) : line.status === 'pending' ? (
                tobotApproval ? (
                  <TobotApprovalCard
                    toolName={tobotApproval.toolName}
                    onAllow={() => decideApproval(true)}
                    onDeny={() => decideApproval(false)}
                  />
                ) : (
                  <ThinkingIndicator />
                )
              ) : null}
            </div>
          </div>
        ))
      : null
  const withPortrait = (person: (typeof snapshot.directory)[number]) => {
    const portrait = portraitFor(person.userId, knownName(person.userId) || person.displayName)
    return {
      ...person,
      displayName: portrait.name || person.displayName,
      phone: portrait.phone,
      avatar: portrait.avatar
    }
  }
  const others = snapshot.directory.filter((person) => person.userId !== meId).map(withPortrait)
  const addable = others.filter((person) => !activeMembers.some((member) => member.userId === person.userId))
  const mention = mentionFocused && selected?.kind === 'group' ? mentionQuery(draft, composerCaret) : null
  const mentionKey = mention ? `${mention.start}:${mention.filter}` : null
  const mentionOpen = Boolean(mention && mentionDismissed !== mentionKey)
  const mentionPool: MentionOption[] =
    selected?.kind === 'group'
      ? [
          { id: 'all', kind: 'all', label: t('messages.mention.all') },
          ...(personalBotIn(selected.id) ? [{ id: 'tobot', kind: 'bot' as const, label: botLabel }] : []),
          ...(tobossGroups.includes(selected.id) ? [{ id: 'toboss', kind: 'bot' as const, label: bossLabel }] : []),
          ...activeMembers
            .filter((member) => member.userId !== meId)
            .map((member) => {
              const portrait = portraitFor(
                member.userId,
                knownName(member.userId) || personName(snapshot, member.userId)
              )
              return {
                id: member.userId,
                kind: 'member' as const,
                label: shownName(member.userId) || portrait.name.trim(),
                phone: portrait.phone,
                avatar: portrait.avatar,
                online: personOnline(member.userId)
              }
            })
            .filter((person) => person.label)
            .sort((left, right) => left.label.localeCompare(right.label, 'zh-CN'))
        ]
      : []
  const mentionOptions = mentionOpen ? filterMentions(mentionPool, mention?.filter ?? '') : []
  const mentionActive = mentionOptions.length === 0 ? 0 : Math.min(mentionIndex, mentionOptions.length - 1)
  const mentionLabels =
    selected?.kind === 'group'
      ? highlightLabels({
          everyone: t('messages.mention.all'),
          myName: snapshot.me?.displayName ?? '',
          bot: personalBotIn(selected.id) ? botLabel : null,
          bots: tobossGroups.includes(selected.id) ? teamBotMentionLabels(bossLabel, t('messages.toboss')) : [],
          people: (detail?.members ?? []).map((member) => ({
            label:
              shownName(member.userId) ||
              portraitFor(member.userId, knownName(member.userId) || personName(snapshot, member.userId)).name,
            me: member.userId === meId
          }))
        })
      : []
  const chooseMention = (option: MentionOption) => {
    if (!mention || !selected) return
    const next = applyMention(draft, mention, option.label)
    pendingCaret.current = next.caret
    composer.writeText(selected.id, next.text)
    setComposerCaret(next.caret)
    setMentionIndex(0)
  }
  const insertEmoji = (emoji: string) => {
    if (!selected) return
    const area = composerRef.current
    const current = area?.value ?? draft
    const start = area?.selectionStart ?? composerCaret
    const end = area?.selectionEnd ?? start
    const boundedStart = Math.max(0, Math.min(start, current.length))
    const boundedEnd = Math.max(boundedStart, Math.min(end, current.length))
    // selectionStart counts UTF-16 code units, the same units as String.length.
    const caret = boundedStart + emoji.length
    pendingCaret.current = caret
    composer.writeText(selected.id, current.slice(0, boundedStart) + emoji + current.slice(boundedEnd))
    setComposerCaret(caret)
    setMenu(null)
    area?.focus()
  }
  const composerDisabled = !(ready || personalToBot) || !selected || speechBlock !== null
  const onComposerDragEnter = (event: ReactDragEvent<globalThis.HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    event.preventDefault()
    fileDragDepth.current += 1
    if (!composerDisabled) setFileDragOver(true)
  }
  const onComposerDragOver = (event: ReactDragEvent<globalThis.HTMLDivElement>) => {
    // Without this, WebKit never fires drop on the composer.
    event.preventDefault()
    if (!event.dataTransfer) return
    event.dataTransfer.dropEffect = composerDisabled || !dragEventCarriesFiles(event) ? 'none' : 'copy'
  }
  const onComposerDragLeave = (event: ReactDragEvent<globalThis.HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    fileDragDepth.current = Math.max(0, fileDragDepth.current - 1)
    if (fileDragDepth.current === 0) setFileDragOver(false)
  }
  const onComposerDrop = (event: ReactDragEvent<globalThis.HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    event.preventDefault()
    event.stopPropagation()
    fileDragDepth.current = 0
    setFileDragOver(false)
    if (composerDisabled) return
    stageFiles(event.dataTransfer.files)
  }

  const quoteFor = (message: ImMessage): MessageQuote | null => {
    if (message.recalled || message.body.kind === 'system' || message.body.kind === 'recalled') return null
    const senderId = message.sender.kind === 'member' ? message.sender.id : ''
    const senderName =
      message.sender.kind === 'service' ? t('messages.system_sender') : personName(snapshot, message.sender.id)
    if (message.body.kind === 'text') {
      const wire = decodeBossWire(message.body.text)
      if (wire?.kind === 'join' || wire?.kind === 'leave') return null
      const record = parseChatRecord(message.body.text)
      const text = record ? record.title : visibleText(message.body.text).trim()
      if (!text) return null
      if (wire?.kind === 'reply') return makeQuote(message.id, BUILTIN_TOBOSS_ID, bossLabel, 'text', text)
      return makeQuote(message.id, senderId, senderName, 'text', text)
    }
    if (message.body.kind === 'file') {
      return makeQuote(
        message.id,
        senderId,
        senderName,
        'file',
        `${t('messages.preview.file')} ${message.body.attachment.name}`.trim()
      )
    }
    if (message.body.kind === 'image')
      return makeQuote(message.id, senderId, senderName, 'image', t('messages.preview.image'))
    if (message.body.kind === 'voice')
      return makeQuote(message.id, senderId, senderName, 'voice', t('messages.preview.voice'))
    return null
  }

  const quoteLine = (line: AgentLine): MessageQuote | null => {
    const text = visibleText(line.text).trim()
    if (!text) return null
    const mine = line.role === 'user'
    const boss = selected?.kind === 'boss'
    return makeQuote(
      line.id,
      mine ? (meId ?? '') : boss ? BUILTIN_TOBOSS_ID : BUILTIN_TOBOT_ID,
      mine ? mePortrait.name || knownName(meId) : boss ? bossLabel : botLabel,
      'text',
      text
    )
  }

  const withdrawnIds = new Set(
    (thread?.messages ?? []).filter((message) => message.recalled).map((message) => message.id)
  )

  const historyRecords: HistoryRecord[] = !selected
    ? []
    : agentThread
      ? (historyOpen && historyAgentLines ? historyAgentLines : shownAgentLines).flatMap((line) => {
          const mine = line.role === 'user'
          const boss = selected.kind === 'boss'
          const senderName = mine ? mePortrait.name || knownName(meId) : boss ? bossLabel : botLabel
          const shared = {
            id: line.id,
            at: line.at,
            senderName,
            phone: mine ? mePortrait.phone : undefined,
            avatar: mine ? mePortrait.avatar : null,
            colorKey: mine ? meId : boss ? BUILTIN_TOBOSS_ID : BUILTIN_TOBOT_ID
          }
          const records: HistoryRecord[] = []
          if (visibleText(line.text).trim()) {
            records.push(toHistoryRecord({ ...shared, key: `${line.id}:text`, kind: 'text', text: line.text }))
          }
          for (const file of line.files) {
            const image = file.mediaType.startsWith('image/')
            records.push(
              toHistoryRecord({
                ...shared,
                key: `${line.id}:${file.path}`,
                kind: image ? 'image' : 'file',
                text: image ? t('messages.preview.image') : `${t('messages.preview.file')} ${file.name}`.trim()
              })
            )
          }
          return records
        })
      : (thread?.messages ?? []).flatMap((message) => {
          if (message.recalled || message.body.kind === 'system' || message.body.kind === 'recalled') return []
          const senderId = message.sender.kind === 'member' ? message.sender.id : ''
          const senderName =
            message.sender.kind === 'service' ? t('messages.system_sender') : personName(snapshot, senderId)
          const portrait = portraitFor(senderId, senderName)
          const shared = {
            id: message.id,
            at: message.createdAt,
            senderName: portrait.name || senderName,
            phone: portrait.phone,
            avatar: portrait.avatar,
            colorKey: senderId || undefined
          }
          if (message.body.kind === 'text') {
            const wire = decodeBossWire(message.body.text)
            if (wire?.kind === 'join' || wire?.kind === 'leave') return []
            if (wire?.kind === 'reply') {
              return [
                toHistoryRecord({
                  ...shared,
                  senderName: bossLabel,
                  avatar: null,
                  colorKey: BUILTIN_TOBOSS_ID,
                  kind: 'text',
                  text: wire.text
                })
              ]
            }
            return [toHistoryRecord({ ...shared, kind: 'text', text: message.body.text })]
          }
          if (message.body.kind === 'file') {
            return [
              toHistoryRecord({
                ...shared,
                kind: 'file',
                text: `${t('messages.preview.file')} ${message.body.attachment.name}`.trim()
              })
            ]
          }
          if (message.body.kind === 'image') {
            return [toHistoryRecord({ ...shared, kind: 'image', text: t('messages.preview.image') })]
          }
          if (message.body.kind === 'voice') {
            return [toHistoryRecord({ ...shared, kind: 'voice', text: t('messages.preview.voice') })]
          }
          return []
        })

  const focusMessage = (id: string) => {
    setFocusId(id)
    if (focusTimer.current != null) globalThis.window.clearTimeout(focusTimer.current)
    focusTimer.current = globalThis.window.setTimeout(() => {
      focusTimer.current = null
      setFocusId((current) => (current === id ? null : current))
    }, 1600)
  }

  const revealMessage = (id: string) => {
    setHistoryOpen(false)
    setBubbleMenu(null)
    const conversationId = selected?.id
    if (!conversationId) return
    stickToBottom.current = false
    const escaped = globalThis.window.CSS?.escape ? globalThis.window.CSS.escape(id) : id.replace(/["\\]/g, '\\$&')
    const transfer = selected?.kind === 'transfer' || selected?.kind === 'boss'
    void (async () => {
      const locate = () => threadRef.current?.querySelector(`[data-message-id="${escaped}"]`) ?? null
      for (let attempt = 0; attempt < 12 && !locate(); attempt += 1) {
        if (transfer) break
        const current = store.getSnapshot().threads[conversationId]
        if (!current?.hasOlder) break
        const loaded = await store.loadOlder(conversationId)
        if (loaded <= 0) break
        await new Promise<void>((resolve) => {
          globalThis.requestAnimationFrame(() => globalThis.requestAnimationFrame(() => resolve()))
        })
      }
      const node = locate()
      if (!(node instanceof globalThis.HTMLElement)) return
      if (typeof node.scrollIntoView === 'function') node.scrollIntoView({ block: 'center' })
      focusMessage(id)
    })()
  }

  const memberNeedle = memberQuery.trim().toLocaleLowerCase()
  const listedMembers = activeMembers.filter((member) => {
    if (!memberNeedle) return true
    const name = shownName(member.userId) || personName(snapshot, member.userId)
    return name.toLocaleLowerCase().includes(memberNeedle)
  })

  return (
    <div ref={pageRef} data-ui="messages.page" className="flex h-full min-h-0 bg-white">
      <aside
        className={cn('relative flex shrink-0 flex-col bg-white', pageSidebarEdgeClassName)}
        style={{ width: listWidth }}>
        <div className="flex items-center gap-2 px-3 pt-3 pb-2">
          <SearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('messages.search_placeholder')}
            placeholder={t('messages.search_placeholder')}
            containerClassName="h-[30px] min-w-0 flex-1 border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]"
          />
          {snapshot.status === 'no-org' ? null : (
            <div ref={plusRef} className="relative">
              <button
                type="button"
                aria-label={t('messages.action.new')}
                aria-haspopup="menu"
                aria-expanded={menu === 'plus'}
                disabled={!ready}
                onClick={() => setMenu((current) => (current === 'plus' ? null : 'plus'))}
                className="flex size-[30px] items-center justify-center rounded-[8px] bg-[#f5f5f5] text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-50">
                <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
              </button>
              {menu === 'plus' ? (
                <div
                  role="menu"
                  data-ui="messages.plus-menu"
                  className="absolute top-9 right-0 z-20 w-[160px] rounded-[10px] border border-[#ebebeb] bg-white p-1 shadow-[0_8px_22px_rgba(20,40,80,0.08)]">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setMenu(null)
                      setDialog('group')
                    }}
                    className={menuItem}>
                    <Users className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                    {t('messages.group_chat')}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setMenu(null)
                      setDialog('org')
                    }}
                    className={menuItem}>
                    <Network className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                    {t('messages.org')}
                  </button>
                </div>
              ) : null}
            </div>
          )}
        </div>
        <div className="relative min-h-0 flex-1">
          <div data-ui="messages.list" className="relative z-0 h-full overflow-x-hidden overflow-y-auto">
            {!ready && snapshot.status !== 'no-org' ? (
              snapshot.status === 'loading' || snapshot.status === 'idle' ? (
                <ConversationListSkeleton />
              ) : null
            ) : pinned.length === 0 && rest.length === 0 ? (
              needle ? (
                <div data-ui="messages.no-results">
                  <EmptyState
                    compact
                    icon={SearchX}
                    title={t('messages.no_results')}
                    description={t('messages.no_results_hint')}
                    actionLabel={t('messages.clear_search')}
                    onAction={() => setQuery('')}
                  />
                </div>
              ) : (
                <div data-ui="messages.empty">
                  <EmptyState
                    compact
                    title={t('messages.empty_list')}
                    description={t('messages.empty_list_hint')}
                    actionLabel={t('messages.org')}
                    onAction={() => setDialog('org')}
                    secondaryLabel={t('messages.group_chat')}
                    onSecondary={() => setDialog('group')}
                  />
                </div>
              )
            ) : (
              <div className="flex flex-col">
                {pinned.length > 0 ? (
                  <div data-ui="messages.pinned" className="sticky top-0 z-10 border-b border-[#f0f0f0] bg-white">
                    {pinned.map(renderConversation)}
                  </div>
                ) : null}
                {rest.map(renderConversation)}
              </div>
            )}
          </div>
        </div>
        <div
          role="separator"
          aria-orientation="vertical"
          aria-valuenow={listWidth}
          aria-label={t('messages.resize')}
          data-ui="messages.resize"
          onMouseDown={onResizeStart}
          className="absolute inset-y-0 right-0 z-20 w-2 translate-x-1/2 cursor-col-resize"
        />
      </aside>

      <section data-ui="messages.thread" className="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
          <header className="flex h-12 shrink-0 items-center gap-2 border-b border-[#f0f0f0] px-4">
            <h1
              className="m-0 min-w-0 flex-1 truncate text-[15px] font-medium text-[#171717]"
              title={headerTitle}
              onContextMenu={
                selected?.kind === 'transfer' || selected?.kind === 'boss'
                  ? (event) => {
                      event.preventDefault()
                      if (selected?.kind === 'boss') openBossProfile()
                      else openToBotProfile()
                    }
                  : undefined
              }>
              {headerTitle}
            </h1>
            {ready && !snapshot.connected ? (
              <span data-ui="messages.offline" className="shrink-0 text-[12px] font-normal text-[#b0b0b0]">
                {t('messages.reconnecting')}
              </span>
            ) : null}
            {selected ? (
              <div className="flex shrink-0 items-center gap-1">
                <button
                  type="button"
                  data-ui="messages.history-open"
                  aria-label={t('messages.history.open')}
                  onClick={() => {
                    setMenu(null)
                    setHistoryOpen(true)
                  }}
                  className="flex size-8 items-center justify-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5]">
                  <Search size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                </button>
                <div ref={moreRef} className="relative">
                  <button
                    type="button"
                    aria-label={t('messages.action.more')}
                    aria-expanded={menu === 'info'}
                    onClick={() => setMenu((current) => (current === 'info' ? null : 'info'))}
                    className={cn(
                      'flex size-8 items-center justify-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10',
                      menu === 'info' && 'bg-[#f2f3f5]'
                    )}>
                    <Ellipsis size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                  </button>
                </div>
              </div>
            ) : null}
          </header>

          {!(ready || personalToBot) ? (
            <ThreadState snapshot={snapshot} onRetry={() => store.retry()} t={t} />
          ) : !selected ? (
            <div data-ui="messages.thread-empty" className="flex min-h-0 flex-1 flex-col">
              <EmptyState
                icon={MessagesSquare}
                title={t('messages.pick_conversation')}
                description={t(
                  snapshot.conversations.length > 0 ? 'messages.pick_conversation_hint' : 'messages.empty_list_hint'
                )}
                actionLabel={t('messages.org')}
                onAction={() => setDialog('org')}
                secondaryLabel={t('messages.group_chat')}
                onSecondary={() => setDialog('group')}
              />
            </div>
          ) : (
            <Scrollbar
              ref={threadRef}
              data-ui="messages.thread-scroll"
              onScroll={onThreadScroll}
              className="min-h-0 flex-1 bg-white">
              <div ref={threadContentRef} className="flex min-h-full flex-col gap-2 px-5 py-3">
                {loadingOlder ? (
                  <p className="m-0 text-center text-[12px] text-[#b0b0b0]">{t('messages.state.loading')}</p>
                ) : null}
                {agentThread && (shownAgentLines.length > 0 || agentWaiting) ? (
                  <>
                    {shownAgentLines.map((line) => (
                      <div
                        key={line.id}
                        data-ui="messages.row"
                        data-message-id={line.id}
                        data-message-focus={focusId === line.id ? 'true' : undefined}
                        className={cn('flex flex-col gap-3', focusId === line.id && 'rounded-[8px] bg-[#f7f7f7]')}>
                        <div className={cn('flex items-start gap-2', line.role === 'user' && 'justify-end')}>
                          {picked ? (
                            <PickBox
                              checked={picked.includes(line.id)}
                              label={visibleText(line.text).trim() || line.id}
                              onToggle={() => togglePick(line.id)}
                            />
                          ) : null}
                          {line.role === 'assistant' ? (
                            <ConversationAvatar
                              entry={selected}
                              title={titleOf(selected)}
                              productLogo={productLogo}
                              animated={line.status === 'pending'}
                              size={36}
                            />
                          ) : null}
                          <div
                            className={cn(
                              'flex min-w-0 flex-col gap-1',
                              line.role === 'user' ? 'items-end' : 'w-full max-w-[70%] items-start'
                            )}>
                            {line.files.map((file) => {
                              const image = file.mediaType.startsWith('image/')
                              const excerpt = image
                                ? t('messages.preview.image')
                                : `${t('messages.preview.file')} ${file.name}`.trim()
                              return (
                                <div
                                  key={file.path}
                                  onContextMenu={(event) =>
                                    openBubbleMenu(
                                      event,
                                      excerpt,
                                      null,
                                      makeQuote(
                                        line.id,
                                        line.role === 'user'
                                          ? (meId ?? '')
                                          : selected.kind === 'boss'
                                            ? BUILTIN_TOBOSS_ID
                                            : BUILTIN_TOBOT_ID,
                                        line.role === 'user'
                                          ? mePortrait.name || knownName(meId)
                                          : selected.kind === 'boss'
                                            ? bossLabel
                                            : botLabel,
                                        image ? 'image' : 'file',
                                        excerpt
                                      ),
                                      line.id
                                    )
                                  }>
                                  <AgentFileView
                                    name={file.name}
                                    mediaType={file.mediaType}
                                    path={file.path}
                                    sizeBytes={file.sizeBytes}
                                    previewUrl={file.previewUrl}
                                  />
                                </div>
                              )
                            })}
                            {line.role === 'assistant' && (line.text || (line.parts?.length ?? 0) > 0) ? (
                              <>
                                <TobotTurnView
                                  parts={line.parts ?? []}
                                  text={line.text}
                                  streaming={line.status === 'pending'}
                                  onContextMenu={(event) =>
                                    openBubbleMenu(event, visibleText(line.text), null, quoteLine(line), line.id)
                                  }
                                />
                                {line.status === 'pending' && line.id === liveAssistantId && tobotApproval ? (
                                  <TobotApprovalCard
                                    toolName={tobotApproval.toolName}
                                    onAllow={() => decideApproval(true)}
                                    onDeny={() => decideApproval(false)}
                                  />
                                ) : null}
                              </>
                            ) : line.role === 'assistant' &&
                              line.status === 'pending' &&
                              line.id === liveAssistantId ? (
                              tobotApproval ? (
                                <TobotApprovalCard
                                  toolName={tobotApproval.toolName}
                                  onAllow={() => decideApproval(true)}
                                  onDeny={() => decideApproval(false)}
                                />
                              ) : (
                                <ThinkingIndicator />
                              )
                            ) : line.id === abandonedAssistantId ? (
                              <TextBubble
                                text={t('messages.tobot.turn_failed')}
                                mine={false}
                                withdrawnIds={withdrawnIds}
                                onQuoteOpen={revealMessage}
                                onContextMenu={(event) =>
                                  openBubbleMenu(event, t('messages.tobot.turn_failed'), null, null, line.id)
                                }
                              />
                            ) : line.role === 'assistant' &&
                              !line.text &&
                              line.files.length === 0 &&
                              (line.status === 'pending' || line.status === 'error') ? null : line.text ||
                              line.files.length === 0 ? (
                              <TextBubble
                                text={line.text}
                                mine={line.role === 'user'}
                                withdrawnIds={withdrawnIds}
                                onQuoteOpen={revealMessage}
                                onContextMenu={(event) =>
                                  openBubbleMenu(event, visibleText(line.text), null, quoteLine(line), line.id)
                                }
                              />
                            ) : null}
                          </div>
                          {line.role === 'user' ? (
                            <PersonAvatar
                              name={mePortrait.name}
                              phone={mePortrait.phone}
                              avatar={mePortrait.avatar}
                              colorKey={meId}
                            />
                          ) : null}
                        </div>
                      </div>
                    ))}
                    {tobotApproval && !liveAssistantId ? (
                      <div data-ui="messages.row" data-agent-status="pending" className="flex flex-col gap-3">
                        <div className="flex items-start gap-2">
                          <ConversationAvatar
                            entry={selected}
                            title={titleOf(selected)}
                            productLogo={productLogo}
                            animated
                            size={36}
                          />
                          <TobotApprovalCard
                            toolName={tobotApproval.toolName}
                            onAllow={() => decideApproval(true)}
                            onDeny={() => decideApproval(false)}
                          />
                        </div>
                      </div>
                    ) : agentWaiting && !liveAssistantId && shownAgentLines.at(-1)?.role !== 'assistant' ? (
                      <div data-ui="messages.row" data-agent-status="pending" className="flex flex-col gap-3">
                        <div className="flex items-start gap-2">
                          <ConversationAvatar
                            entry={selected}
                            title={titleOf(selected)}
                            productLogo={productLogo}
                            animated
                            size={36}
                          />
                          <ThinkingIndicator />
                        </div>
                      </div>
                    ) : null}
                  </>
                ) : rows.length > 0 && !agentThread ? (
                  <>
                    {rows.map(renderRow)}
                    {groupReplyNodes}
                  </>
                ) : thread?.loaded ? (
                  <>
                    <ThreadStart
                      avatar={
                        <ConversationAvatar
                          entry={selected}
                          title={selectedPeer?.name || titleOf(selected)}
                          productLogo={productLogo}
                          faces={selected.kind === 'group' ? groupFacesOf(selected) : undefined}
                          phone={selectedPeer?.phone}
                          avatar={selectedPeer?.avatar}
                          colorKey={selected.peerUserId ?? undefined}
                          animated
                          size={56}
                        />
                      }
                      title={t(
                        selected.kind === 'direct'
                          ? 'messages.thread_start_direct'
                          : selected.kind === 'transfer'
                            ? 'messages.thread_start_transfer'
                            : selected.kind === 'boss'
                              ? 'messages.thread_start_boss'
                              : 'messages.thread_start_group',
                        { name: titleOf(selected) }
                      )}
                      hint={t(
                        selected.kind === 'boss'
                          ? 'messages.thread_start_boss_hint'
                          : selected.kind === 'transfer'
                            ? 'messages.thread_start_transfer_hint'
                            : 'messages.thread_start_hint'
                      )}
                    />
                    {groupReplyNodes}
                  </>
                ) : (
                  <p data-ui="messages.thread-loading" className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">
                    {t('messages.state.loading')}
                  </p>
                )}
              </div>
            </Scrollbar>
          )}

          {(ready || personalToBot) && selected && picked ? (
            <div
              data-ui="messages.forward-bar"
              className="flex h-[88px] shrink-0 items-center justify-center gap-12 border-t border-[#f0f0f0] bg-white">
              <button
                type="button"
                disabled={picked.length === 0}
                onClick={() => setForwardMode('each')}
                className="flex flex-col items-center gap-1 text-[12px] text-[#171717] outline-none disabled:text-[#b0b0b0]">
                <Forward className="size-5" strokeWidth={1.7} />
                {t('messages.forward_each')}
              </button>
              <button
                type="button"
                disabled={picked.length === 0}
                onClick={() => setForwardMode('merge')}
                className="flex flex-col items-center gap-1 text-[12px] text-[#171717] outline-none disabled:text-[#b0b0b0]">
                <Layers className="size-5" strokeWidth={1.7} />
                {t('messages.forward_merge')}
              </button>
              <button
                type="button"
                aria-label={t('common.close')}
                onClick={() => setPicked(null)}
                className="flex size-8 items-center justify-center rounded-full text-[#8a8a8a] outline-none hover:bg-[#f5f5f5] hover:text-[#171717]">
                <X className="size-4" strokeWidth={1.7} />
              </button>
            </div>
          ) : (ready || personalToBot) && selected ? (
            <form
              className="relative z-10 shrink-0 px-3 pt-2 pb-3"
              onSubmit={(event) => {
                event.preventDefault()
                send()
              }}>
              {notice ? (
                <p
                  data-ui="messages.notice"
                  role="alert"
                  className="m-0 mb-2 flex items-center justify-between gap-2 rounded-[8px] bg-[#FFF7F7] px-3 py-1.5 text-[12px] text-[#C43B3B]">
                  <span className="min-w-0 truncate">{notice}</span>
                  <button type="button" className="shrink-0 underline outline-none" onClick={() => setNotice('')}>
                    {t('common.close')}
                  </button>
                </p>
              ) : null}
              {typingName ? (
                <p data-ui="messages.typing" className="m-0 mb-2 text-[12px] text-[#8a8a8a]">
                  {t('messages.typing', { name: typingName })}
                </p>
              ) : null}
              <div className="relative">
                {mentionOpen ? (
                  <MentionPicker
                    label={t('messages.mention.picker')}
                    empty={t('messages.no_people')}
                    options={mentionOptions}
                    activeIndex={mentionActive}
                    onActive={setMentionIndex}
                    onChoose={chooseMention}
                  />
                ) : null}
                <div
                  role="separator"
                  aria-orientation="horizontal"
                  aria-valuenow={composerHeight}
                  aria-label={t('messages.resize_input')}
                  data-ui="messages.composer-resize"
                  onMouseDown={onComposerResizeStart}
                  className="absolute inset-x-0 top-0 z-20 h-2 -translate-y-1/2 cursor-ns-resize"
                />
                <div
                  data-ui="messages.composer-shell"
                  data-drag-over={fileDragOver ? 'true' : undefined}
                  onDragEnter={onComposerDragEnter}
                  onDragOver={onComposerDragOver}
                  onDragLeave={onComposerDragLeave}
                  onDrop={onComposerDrop}
                  className="relative rounded-[10px] border border-[#ebebeb] bg-white">
                  <ComposerFileDropOverlay active={fileDragOver} roundedClassName="rounded-[10px]" />
                  {quoteDraft ? (
                    <div
                      data-ui="messages.quote-draft"
                      className="mx-2 mt-2 flex items-start gap-2 rounded-[6px] bg-[#f5f5f5] px-2 py-1.5">
                      <span className="min-w-0 flex-1 border-l-2 border-[#d6d6d6] pl-2">
                        <span className="block truncate text-[12px] leading-4 text-[#3f3f3f]">
                          {quoteDraft.senderName}
                        </span>
                        <span className="block truncate text-[12px] leading-4 text-[#8a8a8a]">
                          {quoteDraft.excerpt}
                        </span>
                      </span>
                      <button
                        type="button"
                        aria-label={t('messages.quote_cancel')}
                        onClick={() => selected && composer.writeQuote(selected.id, null)}
                        className="flex size-5 shrink-0 items-center justify-center rounded-[4px] text-[#8a8a8a] outline-none hover:bg-[#ebebeb] hover:text-[#171717]">
                        <X className="size-3.5" strokeWidth={1.7} />
                      </button>
                    </div>
                  ) : null}
                  {shots.length > 0 ? (
                    <div data-ui="messages.screenshot-pending" className="flex gap-2 overflow-x-auto px-3 pt-3 pb-1">
                      {shots.map((shot) => {
                        const remove = () => {
                          if (shot.preview && stagedImage?.src === shot.preview) setStagedImage(null)
                          if (stagedFile === shot.file) setStagedFile(null)
                          if (shot.preview) globalThis.URL.revokeObjectURL(shot.preview)
                          if (selected)
                            composer.writeFiles(selected.id, (current) => current.filter((item) => item.id !== shot.id))
                        }
                        if (shot.preview) {
                          return (
                            <div key={shot.id} className="relative size-[72px] shrink-0">
                              <button
                                type="button"
                                aria-label={shot.file.name}
                                onClick={() => setStagedImage({ name: shot.file.name, src: shot.preview! })}
                                className="block size-[72px] overflow-hidden rounded-[10px] border border-[#ebebeb] outline-none focus-visible:ring-2 focus-visible:ring-black/10">
                                <img src={shot.preview} alt={shot.file.name} className="size-[72px] object-cover" />
                              </button>
                              <button
                                type="button"
                                data-ui="messages.screenshot-remove"
                                aria-label={t('common.close')}
                                onClick={remove}
                                className="absolute top-1 right-1 grid size-5 place-items-center rounded-full border border-white bg-[#171717] text-white shadow-[0_1px_3px_rgba(23,23,23,0.45)] outline-none hover:bg-black">
                                <X className="size-3" strokeWidth={2.2} />
                              </button>
                            </div>
                          )
                        }
                        return (
                          <div
                            key={shot.id}
                            className="flex h-14 max-w-[240px] shrink-0 items-center rounded-[10px] border border-[#ebebeb] bg-[#f7f7f7] pr-1">
                            <button
                              type="button"
                              aria-label={shot.file.name}
                              onClick={() => {
                                void openMessageFileEditor(shot.file, shot.id).then((opened) => {
                                  if (!opened) setStagedFile(shot.file)
                                })
                              }}
                              className="flex min-w-0 flex-1 items-center gap-2.5 py-1 pl-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10">
                              <MessageFileMark name={shot.file.name} className={MESSAGE_FILE_ICON_CLASS} />
                              <span className="min-w-0 flex-1">
                                <span className="block truncate text-[13px] leading-4 text-[#171717]">
                                  {shot.file.name}
                                </span>
                                <span className="block text-[12px] leading-4 text-[#8a8a8a]">
                                  {fileSizeLabel(shot.file.size)}
                                </span>
                              </span>
                            </button>
                            <button
                              type="button"
                              data-ui="messages.screenshot-remove"
                              aria-label={t('common.close')}
                              onClick={remove}
                              className="grid size-6 shrink-0 place-items-center rounded-full text-[#8a8a8a] outline-none hover:bg-[#ebebeb] hover:text-[#171717]">
                              <X className="size-3.5" strokeWidth={1.7} />
                            </button>
                          </div>
                        )
                      })}
                    </div>
                  ) : null}
                  {stagedFile ? (
                    <MessageFilePreview
                      open
                      name={stagedFile.name}
                      mime={stagedFile.type}
                      onOpenChange={(next) => {
                        if (!next) setStagedFile(null)
                      }}
                      load={async () => ({ blob: stagedFile })}
                    />
                  ) : null}
                  {stagedImage ? (
                    <ImagePreviewOverlay
                      alt={stagedImage.name}
                      open
                      src={stagedImage.src}
                      onOpenChange={(next) => {
                        if (!next) setStagedImage(null)
                      }}
                    />
                  ) : null}
                  {speechBlock ? (
                    <p data-ui="messages.speech-block" className="m-0 px-3 pt-2 text-[12px] leading-4 text-[#8a8a8a]">
                      {t(speechBlock === 'banned' ? 'messages.speech.banned' : 'messages.speech.muted_all')}
                    </p>
                  ) : null}
                  <div className="overflow-hidden rounded-t-[10px]">
                    <textarea
                      ref={composerRef}
                      data-ui="messages.composer"
                      value={draft}
                      rows={3}
                      disabled={composerDisabled}
                      aria-label={t('messages.compose_placeholder')}
                      aria-expanded={mentionOpen}
                      aria-controls={mentionOpen ? 'messages-mention-picker' : undefined}
                      aria-activedescendant={
                        mentionOpen && mentionOptions[mentionActive]
                          ? `messages-mention-${mentionOptions[mentionActive].id}`
                          : undefined
                      }
                      placeholder={t('messages.compose_placeholder')}
                      onFocus={() => setMentionFocused(true)}
                      onBlur={() => {
                        globalThis.window.setTimeout(() => {
                          if (composerRef.current === globalThis.document.activeElement) return
                          setMentionFocused(false)
                        }, 0)
                      }}
                      onSelect={(event) => setComposerCaret(event.currentTarget.selectionStart ?? 0)}
                      onClick={(event) => setComposerCaret(event.currentTarget.selectionStart ?? 0)}
                      onKeyUp={(event) => setComposerCaret(event.currentTarget.selectionStart ?? 0)}
                      onChange={(event) => {
                        const element = event.target
                        const value = element.value
                        if (selected) composer.writeText(selected.id, value)
                        setComposerCaret(element.selectionStart ?? value.length)
                        setMentionFocused(true)
                        setMentionIndex(0)
                        if (!value.trim() || !selected || (selected.kind !== 'direct' && selected.kind !== 'group')) {
                          return
                        }
                        const at = Date.now()
                        if (at - typingSentAt.current < 3000) return
                        typingSentAt.current = at
                        void store.notifyTyping(selected.id)
                      }}
                      onPaste={onPaste}
                      onKeyDown={(event) => {
                        if (event.nativeEvent.isComposing) return
                        const area = event.currentTarget
                        if (!mentionOpen && event.key === 'Backspace' && area.selectionStart === area.selectionEnd) {
                          const removal = mentionBeforeCaret(area.value, area.selectionStart ?? 0, mentionLabels)
                          if (removal) {
                            event.preventDefault()
                            const next = area.value.slice(0, removal.start) + area.value.slice(removal.end)
                            pendingCaret.current = removal.start
                            if (selected) composer.writeText(selected.id, next)
                            setComposerCaret(removal.start)
                            return
                          }
                        }
                        if (mentionOpen && event.key === 'Escape') {
                          event.preventDefault()
                          event.stopPropagation()
                          setMentionDismissed(mentionKey)
                          return
                        }
                        if (!mentionOpen && event.key === 'Escape' && quoteDraft) {
                          event.preventDefault()
                          if (selected) composer.writeQuote(selected.id, null)
                          return
                        }
                        if (
                          mentionOpen &&
                          mentionOptions.length > 0 &&
                          (event.key === 'ArrowDown' || event.key === 'ArrowUp')
                        ) {
                          event.preventDefault()
                          const count = mentionOptions.length
                          setMentionIndex((current) => {
                            const index = Math.min(current, count - 1)
                            return event.key === 'ArrowDown' ? (index + 1) % count : (index - 1 + count) % count
                          })
                          return
                        }
                        if (
                          mentionOpen &&
                          mentionOptions.length > 0 &&
                          (event.key === 'Enter' || event.key === 'Tab') &&
                          !event.shiftKey
                        ) {
                          event.preventDefault()
                          const option = mentionOptions[Math.min(mentionIndex, mentionOptions.length - 1)]
                          if (option) chooseMention(option)
                          return
                        }
                        if (event.key === 'Enter' && !event.shiftKey) {
                          event.preventDefault()
                          send()
                        }
                      }}
                      style={{ height: composerHeight }}
                      className="m-0 block w-full resize-none border-0 bg-white px-3 py-2 text-[13px] text-[#171717] outline-none placeholder:text-[#b0b0b0] disabled:cursor-not-allowed"
                    />
                  </div>
                  <div className="flex items-center justify-between gap-2 px-1.5 pb-1.5">
                    <div className="flex items-center gap-0.5">
                      <div ref={emojiRef} className="relative">
                        <ToolButton
                          label={t('messages.action.emoji')}
                          disabled={composerDisabled}
                          pressed={menu === 'emoji'}
                          onClick={() => setMenu((current) => (current === 'emoji' ? null : 'emoji'))}>
                          <Smile className="size-4" strokeWidth={1.7} />
                        </ToolButton>
                        <MessageEmojiPanel open={menu === 'emoji'} onPick={insertEmoji} />
                      </div>
                      <ToolButton
                        label={t('messages.action.file')}
                        disabled={composerDisabled}
                        onClick={() => fileRef.current?.click()}>
                        <Folder className="size-4" strokeWidth={1.7} />
                      </ToolButton>
                      <ToolButton
                        label={t('messages.action.image')}
                        disabled={composerDisabled}
                        onClick={() => imageRef.current?.click()}>
                        <ImageIcon className="size-4" strokeWidth={1.7} />
                      </ToolButton>
                      <ComposerScreenshotControl disabled={composerDisabled} onCapture={stageScreenshot} />
                    </div>
                    <button
                      type="submit"
                      data-ui="messages.send"
                      disabled={composerDisabled || (!draft.trim() && shots.length === 0)}
                      className={cn(
                        'h-8 rounded-[8px] px-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                        !composerDisabled && (draft.trim() || shots.length > 0)
                          ? 'bg-[#171717] text-white'
                          : 'border border-[#e6e6e6] bg-white text-[#8a8a8a]'
                      )}>
                      {t('messages.send')}
                    </button>
                  </div>
                </div>
              </div>
              <input
                ref={fileRef}
                type="file"
                multiple
                data-ui="messages.file-input"
                className="sr-only"
                tabIndex={-1}
                aria-hidden
                onChange={(event) => {
                  stageFiles([...(event.target.files ?? [])])
                  event.target.value = ''
                }}
              />
              <input
                ref={imageRef}
                type="file"
                accept="image/*"
                multiple
                data-ui="messages.image-input"
                className="sr-only"
                tabIndex={-1}
                aria-hidden
                onChange={(event) => {
                  stageFiles([...(event.target.files ?? [])])
                  event.target.value = ''
                }}
              />
            </form>
          ) : null}
        </div>
        <InfoDrawer open={menu === 'info' && selected != null} panelRef={infoPanelRef}>
          {selected ? (
            <>
              <div className="sticky top-0 z-10 flex h-9 items-center justify-end bg-white px-2">
                <button
                  type="button"
                  aria-label={t('common.close')}
                  onClick={() => setMenu(null)}
                  className="flex size-6 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                  <X size={14} strokeWidth={1.7} aria-hidden />
                </button>
              </div>
              {selected.kind === 'group' ? (
                <div className="px-4 pt-1">
                  <div className="flex h-[30px] items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5">
                    <Search size={16} className="shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
                    <input
                      value={memberQuery}
                      aria-label={t('messages.search_members')}
                      placeholder={t('messages.search_members')}
                      onChange={(event) => setMemberQuery(event.target.value)}
                      className="h-[30px] min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
                    />
                  </div>
                </div>
              ) : (
                <div className="flex items-center gap-3 px-4 pt-1 pb-3">
                  <ConversationAvatar
                    entry={selected}
                    title={titleOf(selected)}
                    productLogo={productLogo}
                    phone={portraitFor(selected.peerUserId, titleOf(selected)).phone}
                    avatar={portraitFor(selected.peerUserId, titleOf(selected)).avatar}
                    colorKey={selected.peerUserId ?? undefined}
                    size={48}
                  />
                  <p
                    className="m-0 min-w-0 flex-1 truncate text-[14px] leading-5 font-medium text-[#171717]"
                    title={titleOf(selected)}>
                    {titleOf(selected)}
                  </p>
                </div>
              )}
              {selected.kind === 'group' ? (
                <div data-ui="messages.member-grid" className="grid grid-cols-4 gap-x-2 gap-y-4 px-3 py-4">
                  {listedMembers.map((member) => {
                    const portrait = portraitFor(member.userId, knownName(member.userId))
                    const name = shownName(member.userId) || personName(snapshot, member.userId)
                    return (
                      <div key={member.userId} className="flex min-w-0 flex-col items-center gap-1">
                        <PersonAvatar
                          name={portrait.name}
                          phone={portrait.phone}
                          avatar={portrait.avatar}
                          colorKey={member.userId}
                          online={personOnline(member.userId)}
                          className="size-12"
                        />
                        <span className="w-full truncate text-center text-[11px] leading-4 text-[#3f3f3f]" title={name}>
                          {name}
                        </span>
                      </div>
                    )
                  })}
                  {personalBotIn(selected.id) &&
                  (!memberNeedle || botLabel.toLocaleLowerCase().includes(memberNeedle)) ? (
                    <div className="flex min-w-0 flex-col items-center gap-1">
                      <ToBotMark profile={tobotProfile} size={48} label={botLabel} />
                      <span
                        className="w-full truncate text-center text-[11px] leading-4 text-[#3f3f3f]"
                        title={botLabel}>
                        {botLabel}
                      </span>
                    </div>
                  ) : null}
                  {tobossGroups.includes(selected.id) &&
                  (!memberNeedle || bossLabel.toLocaleLowerCase().includes(memberNeedle)) ? (
                    <div className="flex min-w-0 flex-col items-center gap-1">
                      <ToBossMark size={48} label={bossLabel} />
                      <span
                        className="w-full truncate text-center text-[11px] leading-4 text-[#3f3f3f]"
                        title={bossLabel}>
                        {bossLabel}
                      </span>
                    </div>
                  ) : null}
                  <button
                    type="button"
                    aria-label={t('messages.add_member')}
                    onClick={() => {
                      setMenu(null)
                      setDialog('add')
                    }}
                    className="flex min-w-0 flex-col items-center gap-1 outline-none">
                    <span className="flex size-12 items-center justify-center rounded-[8px] border border-dashed border-[#d6d6d6] text-[#8a8a8a]">
                      <Plus className="size-5" strokeWidth={1.7} aria-hidden />
                    </span>
                    <span className="w-full truncate text-center text-[11px] leading-4 text-[#8a8a8a]">
                      {t('messages.add_short')}
                    </span>
                  </button>
                </div>
              ) : null}
              {selected.kind === 'group' ? (
                myRole === 'owner' || myRole === 'admin' ? (
                  <InfoEditRow
                    label={t('messages.rename')}
                    value={selected.title ?? ''}
                    placeholder=""
                    dataUi="messages.rename"
                    resetKey={selected.id}
                    onCommit={(title) => {
                      if (!title || [...title].length > 64) {
                        setNotice(t(title ? 'messages.group_name_too_long' : 'messages.group_name'))
                        return false
                      }
                      return store.rename(selected.id, title).catch((error) => {
                        fail(error)
                        throw error
                      })
                    }}
                  />
                ) : (
                  <InfoTextRow label={t('messages.rename')} value={selected.title ?? ''} dataUi="messages.rename" />
                )
              ) : null}
              {selected.kind !== 'transfer' && selected.kind !== 'boss' ? (
                <InfoEditRow
                  label={t('messages.remark')}
                  value={selected.remark ?? ''}
                  placeholder={t(
                    selected.kind === 'group' ? 'messages.remark_placeholder_group' : 'messages.remark_placeholder'
                  )}
                  dataUi="messages.remark"
                  resetKey={selected.id}
                  onCommit={(remark) => {
                    if ([...remark].length > 64) {
                      setNotice(t('messages.remark_too_long'))
                      return false
                    }
                    return store.setRemark(selected.id, remark).catch((error) => {
                      fail(error)
                      throw error
                    })
                  }}
                />
              ) : null}
              {selected.kind === 'group' && myMembership ? (
                <InfoEditRow
                  label={t('messages.nickname')}
                  value={myMembership.nickname ?? ''}
                  placeholder={t('messages.nickname_placeholder')}
                  dataUi="messages.nickname"
                  resetKey={`${selected.id}:${myMembership.userId}`}
                  onCommit={(nickname) => {
                    if ([...nickname].length > 32) {
                      setNotice(t('messages.nickname_too_long'))
                      return false
                    }
                    return store.setNickname(selected.id, nickname).catch((error) => {
                      fail(error)
                      throw error
                    })
                  }}
                />
              ) : null}
              {selected.kind === 'transfer' || selected.kind === 'boss' ? (
                <>
                  <InfoLinkRow
                    label={t('messages.tobot.edit')}
                    onClick={() => (selected.kind === 'boss' ? openBossProfile() : openToBotProfile())}
                  />
                  <button
                    type="button"
                    onClick={() => {
                      setProfileBot(selected.kind === 'boss' ? 'toboss' : 'tobot')
                      setMenu(null)
                      setDialog('reset')
                    }}
                    className={cn(infoRowClass, 'text-[#b42318]')}>
                    {t(selected.kind === 'boss' ? 'messages.toboss.reset' : 'messages.tobot.reset')}
                  </button>
                </>
              ) : (
                <InfoLinkRow
                  label={t('messages.history.open')}
                  dataUi="messages.history-open-info"
                  onClick={() => {
                    setMenu(null)
                    setHistoryOpen(true)
                  }}
                />
              )}
              {selected.lastSequence > 0 || selected.id === BUILTIN_TOBOT_ID || selected.id === BUILTIN_TOBOSS_ID ? (
                <button
                  type="button"
                  onClick={() => applyInPanel(() => store.markUnread(selected.id))}
                  className={infoRowClass}>
                  {t('messages.unread')}
                </button>
              ) : null}
              <InfoSwitchRow
                label={t('messages.mute')}
                pressed={selected.muted}
                onClick={() => applyInPanel(() => store.setMuted(selected.id, !selected.muted))}
              />
              <InfoSwitchRow
                label={t('messages.pin_chat')}
                pressed={selected.pinned}
                onClick={() => applyInPanel(() => store.setPinned(selected.id, !selected.pinned))}
              />
              {selected.kind === 'group' && (myRole === 'owner' || myRole === 'admin') ? (
                <InfoSwitchRow
                  label={t('messages.speech.mute_all')}
                  pressed={detail?.muteAll === true}
                  dataUi="messages.mute-all"
                  onClick={() => applyInPanel(() => store.setMuteAll(selected.id, !detail?.muteAll))}
                />
              ) : null}
              {selected.kind === 'group' && (myRole === 'owner' || myRole === 'admin') ? (
                <div>
                  <p className="m-0 px-4 pt-3 pb-1 text-[12px] leading-4 text-[#8a8a8a]">{t('messages.members')}</p>
                  <ul className="m-0 list-none p-0">
                    {listedMembers.map((member) => {
                      const portrait = portraitFor(member.userId, knownName(member.userId))
                      const name = shownName(member.userId) || personName(snapshot, member.userId)
                      const removable =
                        member.userId !== meId &&
                        member.role !== 'owner' &&
                        !(myRole === 'admin' && member.role === 'admin')
                      const canBan = removable
                      const canManageRole = myRole === 'owner' && member.userId !== meId
                      const roleLabel =
                        member.role === 'owner'
                          ? t('messages.role.owner')
                          : member.role === 'admin'
                            ? t('messages.role.admin')
                            : ''
                      const meta = [roleLabel, member.speechBanned ? t('messages.speech.banned_label') : '']
                        .filter(Boolean)
                        .join(' · ')
                      return (
                        <li
                          key={member.userId}
                          data-ui="messages.member"
                          className="flex min-h-12 items-center gap-2.5 border-b border-[#efefef] px-4 hover:bg-[#f7f7f7]">
                          <PersonAvatar
                            name={portrait.name}
                            phone={portrait.phone}
                            avatar={portrait.avatar}
                            colorKey={member.userId}
                            online={personOnline(member.userId)}
                            className="size-8"
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-[13px] leading-5 text-[#171717]" title={name}>
                              {name}
                            </span>
                            {meta ? (
                              <span className="block truncate text-[12px] leading-4 text-[#8a8a8a]">{meta}</span>
                            ) : null}
                          </span>
                          {canManageRole || canBan || removable ? (
                            <button
                              type="button"
                              data-ui="messages.member-actions"
                              aria-label={t('messages.member_actions', { name })}
                              aria-expanded={memberMenu?.userId === member.userId}
                              onClick={(event) => {
                                const rect = event.currentTarget.getBoundingClientRect()
                                setMemberMenu((current) =>
                                  current?.userId === member.userId
                                    ? null
                                    : { userId: member.userId, x: rect.right, y: rect.bottom + 4 }
                                )
                              }}
                              className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] focus-visible:ring-2 focus-visible:ring-black/10">
                              <Ellipsis className="size-4" strokeWidth={1.7} aria-hidden />
                            </button>
                          ) : null}
                        </li>
                      )
                    })}
                    {personalBotIn(selected.id) ? (
                      <li
                        data-ui="messages.member"
                        data-member="tobot"
                        className="flex min-h-9 items-center gap-2 border-b border-[#efefef] px-4">
                        <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{botLabel}</span>
                        <button
                          type="button"
                          onClick={() => setToBot(selected.id, false)}
                          className="shrink-0 rounded-[6px] px-1 text-[12px] text-[#b42318] outline-none hover:bg-[#f7f7f7]">
                          {t('messages.remove_tobot', { name: botLabel })}
                        </button>
                      </li>
                    ) : null}
                    {tobossGroups.includes(selected.id) ? (
                      <li
                        data-ui="messages.member"
                        data-member="toboss"
                        className="flex min-h-9 items-center gap-2 border-b border-[#efefef] px-4">
                        <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{bossLabel}</span>
                        {bossListed ? (
                          <button
                            type="button"
                            onClick={() => setToBoss(selected.id, false)}
                            className="shrink-0 rounded-[6px] px-1 text-[12px] text-[#b42318] outline-none hover:bg-[#f7f7f7]">
                            {t('messages.remove_tobot', { name: bossLabel })}
                          </button>
                        ) : null}
                      </li>
                    ) : null}
                  </ul>
                </div>
              ) : null}
              {selected.kind === 'group' && myRole === 'owner' ? (
                <button
                  type="button"
                  data-ui="messages.dissolve"
                  onClick={() => {
                    setMenu(null)
                    setDialog('dissolve')
                  }}
                  className="mt-2 flex h-9 w-full items-center justify-center text-[13px] text-[#b42318] outline-none hover:bg-[#f7f7f7]">
                  {t('messages.dissolve')}
                </button>
              ) : null}
              {selected.kind === 'group' && myRole && myRole !== 'owner' ? (
                <button
                  type="button"
                  onClick={() => {
                    setMenu(null)
                    setDialog('leave')
                  }}
                  className="mt-2 flex h-9 w-full items-center justify-center text-[13px] text-[#b42318] outline-none hover:bg-[#f7f7f7]">
                  {t('messages.leave')}
                </button>
              ) : null}
            </>
          ) : null}
        </InfoDrawer>
      </section>
      {memberMenu && selected?.kind === 'group'
        ? (() => {
            const member = listedMembers.find((item) => item.userId === memberMenu.userId)
            if (!member || !myRole) return null
            const name = shownName(member.userId) || personName(snapshot, member.userId)
            const removable =
              member.userId !== meId && member.role !== 'owner' && !(myRole === 'admin' && member.role === 'admin')
            const canBan = removable
            const canManageRole = myRole === 'owner' && member.userId !== meId
            const items = (canManageRole ? 2 : 0) + (canBan ? 1 : 0) + (removable ? 1 : 0)
            const width = 168
            const height = 8 + items * 32
            const x = Math.max(8, Math.min(memberMenu.x - width, globalThis.window.innerWidth - width - 8))
            const y = Math.max(8, Math.min(memberMenu.y, globalThis.window.innerHeight - height - 8))
            return (
              <div
                ref={memberMenuRef}
                role="menu"
                data-ui="messages.member-menu"
                className="fixed z-40 w-[168px] rounded-[10px] border border-[#e5e5e5] bg-white p-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)]"
                style={{ left: x, top: y }}>
                {canManageRole ? (
                  <button
                    type="button"
                    role="menuitem"
                    data-ui="messages.role"
                    className={menuItemClass}
                    onClick={() => {
                      setMemberMenu(null)
                      applyInPanel(() =>
                        store.setMemberRole(selected.id, member.userId, member.role === 'admin' ? 'member' : 'admin')
                      )
                    }}>
                    {t(member.role === 'admin' ? 'messages.role.make_member' : 'messages.role.make_admin')}
                  </button>
                ) : null}
                {canBan ? (
                  <button
                    type="button"
                    role="menuitem"
                    data-ui="messages.speech-ban"
                    className={menuItemClass}
                    onClick={() => {
                      setMemberMenu(null)
                      applyInPanel(() => store.setMemberSpeech(selected.id, member.userId, !member.speechBanned))
                    }}>
                    {t(member.speechBanned ? 'messages.speech.unban' : 'messages.speech.ban')}
                  </button>
                ) : null}
                {canManageRole ? (
                  <button
                    type="button"
                    role="menuitem"
                    data-ui="messages.transfer"
                    className={menuItemClass}
                    onClick={() => {
                      setMemberMenu(null)
                      setMemberConfirm({ kind: 'transfer', userId: member.userId, name })
                    }}>
                    {t('messages.transfer_owner')}
                  </button>
                ) : null}
                {removable ? (
                  <button
                    type="button"
                    role="menuitem"
                    data-ui="messages.remove-member"
                    className={menuDangerClass}
                    onClick={() => {
                      setMemberMenu(null)
                      setMemberConfirm({ kind: 'remove', userId: member.userId, name })
                    }}>
                    {t('messages.remove_member')}
                  </button>
                ) : null}
              </div>
            )
          })()
        : null}
      {rowMenu
        ? (() => {
            const entry = snapshot.conversations.find((item) => item.id === rowMenu.id)
            if (!entry) return null
            const width = 188
            const height = entry.kind === 'transfer' || entry.kind === 'boss' ? 148 : 112
            const x = Math.max(8, Math.min(rowMenu.x, globalThis.window.innerWidth - width - 8))
            const y = Math.max(8, Math.min(rowMenu.y, globalThis.window.innerHeight - height - 8))
            return (
              <div
                ref={contextRef}
                role="menu"
                data-ui="messages.context-menu"
                className="fixed z-40 w-[188px] rounded-[10px] border border-[#ebebeb] bg-white p-1 shadow-[0_8px_22px_rgba(20,40,80,0.08)]"
                style={{ left: x, top: y }}>
                <ConversationActions
                  entry={entry}
                  onPinned={() => settings(() => store.setPinned(entry.id, !entry.pinned))}
                  onMuted={() => settings(() => store.setMuted(entry.id, !entry.muted))}
                  onUnread={() => settings(() => store.markUnread(entry.id))}
                  onEdit={
                    entry.kind === 'boss' ? openBossProfile : entry.kind === 'transfer' ? openToBotProfile : undefined
                  }
                />
              </div>
            )
          })()
        : null}
      {bubbleMenu
        ? (() => {
            const width = 188
            const itemCount =
              1 +
              (bubbleMenu.selectId ? 1 : 0) +
              (bubbleMenu.quote ? 1 : 0) +
              (bubbleMenu.recall ? 1 : 0) +
              (bubbleMenu.deleteId ? 1 : 0)
            const height = 8 + itemCount * 32
            const x = Math.max(8, Math.min(bubbleMenu.x, globalThis.window.innerWidth - width - 8))
            const y = Math.max(8, Math.min(bubbleMenu.y, globalThis.window.innerHeight - height - 8))
            return (
              <div
                ref={bubbleMenuRef}
                role="menu"
                data-ui="messages.bubble-menu"
                className="fixed z-40 w-[188px] rounded-[10px] border border-[#ebebeb] bg-white p-1 shadow-[0_8px_22px_rgba(20,40,80,0.08)]"
                style={{ left: x, top: y }}
                onContextMenu={(event) => event.preventDefault()}>
                <button
                  type="button"
                  role="menuitem"
                  className={menuItemClass}
                  onClick={() => copyBubbleText(bubbleMenu.text)}>
                  <Copy className={menuIconClass} strokeWidth={1.7} aria-hidden />
                  {t('common.copy')}
                </button>
                {bubbleMenu.selectId ? (
                  <button
                    type="button"
                    role="menuitem"
                    className={menuItemClass}
                    onClick={() => {
                      const id = bubbleMenu.selectId
                      setBubbleMenu(null)
                      if (id) setPicked([id])
                    }}>
                    <Forward className={menuIconClass} strokeWidth={1.7} aria-hidden />
                    {t('messages.forward')}
                  </button>
                ) : null}
                {bubbleMenu.quote ? (
                  <button
                    type="button"
                    role="menuitem"
                    className={menuItemClass}
                    onClick={() => {
                      const quote = bubbleMenu.quote
                      setBubbleMenu(null)
                      if (quote && selected) composer.writeQuote(selected.id, quote)
                      composerRef.current?.focus()
                    }}>
                    <Quote className={menuIconClass} strokeWidth={1.7} aria-hidden />
                    {t('messages.quote')}
                  </button>
                ) : null}
                {bubbleMenu.recall ? (
                  <button
                    type="button"
                    role="menuitem"
                    className={menuItemClass}
                    onClick={() => {
                      const target = bubbleMenu.recall
                      setBubbleMenu(null)
                      if (target) void store.recall(target.conversationId, target.messageId).catch(fail)
                    }}>
                    <Undo2 className={menuIconClass} strokeWidth={1.7} aria-hidden />
                    {t('messages.recall_action')}
                  </button>
                ) : null}
                {bubbleMenu.deleteId ? (
                  <button
                    type="button"
                    role="menuitem"
                    data-ui="messages.delete-action"
                    className={menuDangerClass}
                    onClick={() => {
                      const id = bubbleMenu.deleteId
                      setBubbleMenu(null)
                      if (id) deleteBubble(id)
                    }}>
                    <Trash2 className={menuIconClass} strokeWidth={1.7} aria-hidden />
                    {t('messages.delete')}
                  </button>
                ) : null}
              </div>
            )
          })()
        : null}

      <ForwardDialog
        open={forwardMode !== null}
        mode={forwardMode ?? 'each'}
        chats={forwardChats}
        people={snapshot.directory
          .filter((person) => person.userId !== meId)
          .map((person) => {
            const portrait = portraitFor(person.userId, knownName(person.userId) || person.displayName)
            return {
              ...person,
              displayName: portrait.name || person.displayName,
              phone: portrait.phone,
              avatar: portrait.avatar
            }
          })}
        departments={snapshot.departments}
        onOpenChange={(open) => {
          if (!open) setForwardMode(null)
        }}
        onConfirm={forwardTo}
      />
      {selected ? (
        <HistoryDialog
          open={historyOpen}
          onOpenChange={setHistoryOpen}
          name={titleOf(selected)}
          records={historyRecords}
          formatTime={timeLabel}
          loading={historyLoading}
          store={store}
          onPick={revealMessage}
        />
      ) : null}
      <ToBotProfileDialog
        open={dialog === 'profile'}
        onOpenChange={(open) => setDialog(open ? 'profile' : null)}
        storageKey={profileBot === 'toboss' ? TOBOSS_PROFILE_KEY : TOBOT_PROFILE_KEY}
        fallbackKey={profileBot === 'toboss' ? 'messages.toboss' : 'messages.file_transfer'}
      />
      <OrgDialog
        open={dialog === 'org'}
        onOpenChange={(open) => setDialog(open ? 'org' : null)}
        departments={snapshot.departments}
        people={snapshot.directory.map(withPortrait)}
        meId={meId}
        onStartChat={startChat}
      />
      <PeoplePickerDialog
        open={dialog === 'group'}
        onOpenChange={(open) => setDialog(open ? 'group' : null)}
        title={t('messages.group_chat')}
        confirmLabel={t('messages.create_group')}
        people={others}
        withName
        onConfirm={createGroup}
      />
      <PeoplePickerDialog
        open={dialog === 'add'}
        onOpenChange={(open) => setDialog(open ? 'add' : null)}
        title={t('messages.add_member')}
        confirmLabel={t('messages.add_member')}
        people={addable}
        withName={false}
        bots={
          selected?.kind === 'group' &&
          snapshot.conversations.some((entry) => entry.id === BUILTIN_TOBOSS_ID) &&
          !tobossGroups.includes(selected.id)
            ? [{ id: BUILTIN_TOBOSS_ID, label: bossLabel }]
            : undefined
        }
        onConfirm={async (userIds) => {
          if (!selected) return
          if (userIds.includes(BUILTIN_TOBOSS_ID)) setToBoss(selected.id, true)
          const humans = userIds.filter((id) => id !== BUILTIN_TOBOT_ID && id !== BUILTIN_TOBOSS_ID)
          if (humans.length > 0) await store.addMembers(selected.id, humans)
        }}
      />
      <ConfirmDialog
        open={dialog === 'reset'}
        onOpenChange={(open) => setDialog(open ? 'reset' : null)}
        title={t(profileBot === 'toboss' ? 'messages.toboss.reset_title' : 'messages.tobot.reset_title')}
        description={t(profileBot === 'toboss' ? 'messages.toboss.reset_body' : 'messages.tobot.reset_body')}
        confirmText={t(profileBot === 'toboss' ? 'messages.toboss.reset_action' : 'messages.tobot.reset_action')}
        cancelText={t('common.cancel')}
        destructive
        onConfirm={async () => {
          const boss = profileBot === 'toboss'
          try {
            await dataApiService.post(boss ? '/agent-sessions/toboss/reset' : '/agent-sessions/tobot/reset', {
              body: {}
            })
            transcriptEpoch.current += 1
            pendingAgentLine.current = null
            pendingDataWrite.current = null
            tobotToolNames.current.clear()
            setAgentWaiting(false)
            if (!boss || selectedKindRef.current === 'boss') setAgentLines([])
            dropGroupAgent(boss ? 'toboss' : 'tobot')
            if (boss) setSavedBossDigest(null)
            else setSavedDigest(null)
            setTobotLive(null)
            setTobotApproval(null)
          } catch (error) {
            fail(error)
            return false
          }
          return undefined
        }}
      />
      <ConfirmDialog
        open={memberConfirm?.kind === 'transfer'}
        onOpenChange={(open) => {
          if (!open) setMemberConfirm((current) => (current?.kind === 'transfer' ? null : current))
        }}
        title={t('messages.transfer_confirm_title')}
        description={memberConfirm ? t('messages.transfer_confirm', { name: memberConfirm.name }) : undefined}
        confirmText={t('messages.transfer_owner')}
        cancelText={t('common.cancel')}
        onConfirm={async () => {
          if (!selected || memberConfirm?.kind !== 'transfer') return undefined
          try {
            await store.transferOwner(selected.id, memberConfirm.userId)
          } catch (error) {
            fail(error)
            return false
          }
          return undefined
        }}
      />
      <ConfirmDialog
        open={memberConfirm?.kind === 'remove'}
        onOpenChange={(open) => {
          if (!open) setMemberConfirm((current) => (current?.kind === 'remove' ? null : current))
        }}
        title={t('messages.remove_confirm_title')}
        description={memberConfirm ? t('messages.remove_confirm', { name: memberConfirm.name }) : undefined}
        confirmText={t('messages.remove_member')}
        cancelText={t('common.cancel')}
        destructive
        onConfirm={async () => {
          if (!selected || memberConfirm?.kind !== 'remove') return undefined
          try {
            await store.removeMember(selected.id, memberConfirm.userId)
          } catch (error) {
            fail(error)
            return false
          }
          return undefined
        }}
      />
      <ConfirmDialog
        open={dialog === 'leave'}
        onOpenChange={(open) => setDialog(open ? 'leave' : null)}
        title={t('messages.leave_confirm_title')}
        description={selected ? t('messages.leave_confirm', { name: titleOf(selected) }) : undefined}
        confirmText={t('messages.leave')}
        cancelText={t('common.cancel')}
        destructive
        onConfirm={async () => {
          if (!selected) return
          try {
            await store.leave(selected.id)
            composer.focus(null)
            setSelectedId(null)
          } catch (error) {
            fail(error)
          }
        }}
      />
      <ConfirmDialog
        open={dialog === 'dissolve'}
        onOpenChange={(open) => setDialog(open ? 'dissolve' : null)}
        title={t('messages.dissolve_confirm_title')}
        description={selected ? t('messages.dissolve_confirm', { name: titleOf(selected) }) : undefined}
        confirmText={t('messages.dissolve')}
        cancelText={t('common.cancel')}
        destructive
        onConfirm={async () => {
          if (!selected) return
          try {
            await store.dissolve(selected.id)
            composer.focus(null)
            setSelectedId(null)
          } catch (error) {
            fail(error)
          }
        }}
      />
    </div>
  )
}

function ToolButton({
  label,
  pressed = false,
  disabled = false,
  onClick,
  children
}: {
  label: string
  pressed?: boolean
  disabled?: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className="flex size-8 items-center justify-center rounded-[8px] text-[#5f5f5f] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40 disabled:hover:bg-transparent">
      {children}
    </button>
  )
}
