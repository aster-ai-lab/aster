import { ConfirmDialog, DialogDescription, DialogTitle, PopConfirm } from '@a3s-lab/ui'
import { Paperclip, Pencil, PenLine, Trash2, Users, X } from 'lucide-react'
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject
} from 'react'
import { useTranslation } from 'react-i18next'

import {
  listMailColleagues,
  resolveMailColleagues,
  sendMail,
  setMailState,
  type MailColleague,
  type MailFile,
  type MailMessage
} from '@renderer/mail/mailApi'
import { useTeamMailAccess } from '@renderer/mail/useTeamMailAccess'
import { cn } from '@renderer/utils/style'

import {
  filledButtonClassName,
  orgFieldClassName,
  orgSectionLabelClassName,
  quietButtonClassName,
  rowActionClassName
} from '../org/orgUi'
import {
  attachmentProblem,
  downloadMailAttachment,
  readMailAttachment,
  type AttachmentProblem
} from './mailAttachments'
import {
  addNamedRecipients,
  EMPTY_COMPOSE,
  BODY_CAP,
  chordKeys,
  chordLabel,
  commandEnterLabel,
  composeDirty,
  composeFailure,
  consume,
  flushTokens,
  listedRecipients,
  mergeTokens,
  RECIPIENT_CAP,
  recipientTone,
  SUBJECT_CAP,
  type ComposeFocus,
  type ComposeState,
  type Recipient
} from './mailCompose'
import { mailBodyText } from './mailDocument'
import { MailEditor, type MailEditorHandle } from './MailEditor'
import {
  defaultSignature,
  loadSignatures,
  newSignatureId,
  openingBody,
  saveSignatures,
  SIGNATURE_CAP,
  SIGNATURE_NAME_CAP,
  SIGNATURE_TEXT_CAP,
  signatureHtml,
  type MailSignature
} from './mailSignatures'
import { filterColleagues, MailTeamPanel } from './MailTeamPicker'

const fieldClassName = cn(orgFieldClassName, 'outline-none focus-visible:ring-2 focus-visible:ring-black/10')

function attachmentMessage(problem: AttachmentProblem, translate: (key: string) => string): string {
  if (problem === 'too_large') return translate('mail.attachment.too_large')
  if (problem === 'too_many') return translate('mail.attachment.too_many')
  if (problem === 'total') return translate('mail.attachment.total')
  return translate('mail.attachment.invalid')
}

type Suggestion = { id: string; name: string; detail: string }

function RecipientField({
  id,
  label,
  tokens,
  draft,
  inputRef,
  invalid,
  describedBy,
  placeholder,
  suggestions,
  highlight,
  onHighlight,
  onPick,
  onFocus,
  onChange,
  onAdvance
}: {
  id: string
  label: string
  tokens: Recipient[]
  draft: string
  inputRef: RefObject<HTMLInputElement | null>
  invalid: boolean
  describedBy?: string
  placeholder?: string
  suggestions?: Suggestion[]
  highlight?: number
  onHighlight?: (index: number) => void
  onPick?: (id: string) => void
  onFocus?: () => void
  onChange: (tokens: Recipient[], draft: string) => void
  onAdvance: () => void
}) {
  const { t } = useTranslation()
  const listId = `${id}-suggestions`
  const open = (suggestions?.length ?? 0) > 0
  const apply = (nextDraft: string, force: boolean) => {
    const { committed, rest } = consume(nextDraft, force)
    onChange(mergeTokens(tokens, committed), rest)
  }

  return (
    <div className="relative min-w-0 flex-1">
      <div
        className="flex min-h-9 min-w-0 flex-wrap items-center gap-1 px-1 py-1"
        onMouseDown={(event) => {
          if (event.target !== event.currentTarget) return
          event.preventDefault()
          inputRef.current?.focus()
        }}>
        {tokens.map((token) => (
          <span
            key={token.value}
            className={cn(
              'inline-flex h-6 max-w-full items-center rounded-[6px] bg-white pl-1.5 text-[13px] text-[#171717]',
              recipientTone(token.value) === 'bad' && 'text-[#b42318]'
            )}>
            <span className="truncate" title={token.value}>
              {token.label && token.valid ? token.label : token.value}
            </span>
            <button
              type="button"
              aria-label={t('mail.compose.remove', {
                address: token.label && token.valid ? `${token.label} ${token.value}` : token.value
              })}
              className="grid size-6 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
              onClick={() =>
                onChange(
                  tokens.filter((item) => item.value !== token.value),
                  draft
                )
              }>
              <X size={14} strokeWidth={1.7} />
            </button>
          </span>
        ))}
        <input
          id={id}
          ref={inputRef}
          aria-label={label}
          aria-invalid={invalid || undefined}
          aria-describedby={describedBy}
          aria-expanded={open || undefined}
          aria-controls={open ? listId : undefined}
          aria-autocomplete={open ? 'list' : undefined}
          placeholder={tokens.length === 0 ? placeholder : undefined}
          value={draft}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          autoComplete="off"
          className="h-6 min-w-[8rem] flex-1 border-0 bg-transparent px-1 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
          onFocus={onFocus}
          onBlur={() => {
            if (draft.trim() && !open) apply(draft, true)
          }}
          onChange={(event) => apply(event.target.value, false)}
          onPaste={(event) => {
            const text = event.clipboardData.getData('text')
            if (!/[,;\s]/.test(text)) return
            event.preventDefault()
            const { committed } = consume(`${draft}${text}`, true)
            onChange(mergeTokens(tokens, committed), '')
          }}
          onKeyDown={(event) => {
            if (open && suggestions && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
              event.preventDefault()
              const next = event.key === 'ArrowDown' ? (highlight ?? 0) + 1 : (highlight ?? 0) - 1
              onHighlight?.((next + suggestions.length) % suggestions.length)
              return
            }
            if (open && event.key === 'Escape') {
              event.preventDefault()
              event.stopPropagation()
              onHighlight?.(-1)
              return
            }
            if (event.key === 'Backspace' && draft === '' && tokens.length > 0) {
              event.preventDefault()
              onChange(tokens.slice(0, -1), '')
              return
            }
            if (event.key !== 'Enter' || event.metaKey || event.ctrlKey) return
            event.preventDefault()
            if (open && suggestions && (highlight ?? 0) >= 0) {
              const picked = suggestions[highlight ?? 0]
              if (picked) onPick?.(picked.id)
              return
            }
            if (draft.trim()) {
              apply(draft, true)
              return
            }
            onAdvance()
          }}
        />
      </div>
      {open && suggestions ? (
        <ul
          id={listId}
          role="listbox"
          data-ui="mail.team.suggest"
          className="absolute top-full right-0 left-0 z-20 mt-1 max-h-60 overflow-auto rounded-[8px] border border-[#efefef] bg-white py-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
          {suggestions.map((item, index) => (
            <li key={item.id} role="presentation">
              <button
                type="button"
                role="option"
                aria-selected={index === highlight}
                className={cn(
                  'flex h-9 w-full items-center gap-2 px-2.5 text-left text-[13px] text-[#171717]',
                  index === highlight ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
                )}
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => onHighlight?.(index)}
                onClick={() => onPick?.(item.id)}>
                <span className="min-w-0 flex-1 truncate">{item.name}</span>
                {item.detail ? (
                  <span className="shrink-0 truncate text-[12px] text-[#8a8a8a]">{item.detail}</span>
                ) : null}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}

function FileChips({ files, onRemove }: { files: MailFile[]; onRemove: (index: number) => void }) {
  const { t } = useTranslation()
  if (files.length === 0) return null
  return (
    <ul data-ui="mail.compose.files" className="m-0 flex list-none flex-wrap gap-2 px-6 pb-3">
      {files.map((file, index) => (
        <li
          key={`${file.name}-${index}`}
          className="inline-flex h-7 max-w-full items-center rounded-[8px] bg-[#f5f5f5] pl-2 text-[13px]">
          <span className="truncate" title={file.name}>
            {file.name}
          </span>
          <button
            type="button"
            aria-label={t('mail.attachment.remove', { name: file.name })}
            className="grid size-7 shrink-0 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5] hover:text-[#171717]"
            onClick={() => onRemove(index)}>
            <X size={14} strokeWidth={1.7} />
          </button>
        </li>
      ))}
    </ul>
  )
}

export function MailAttachmentList({ files }: { files: MailFile[] }) {
  const { t } = useTranslation()
  if (files.length === 0) return null
  return (
    <div data-ui="mail.attachments" className="px-4 pt-3">
      <p className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{t('mail.reader.attachments')}</p>
      <ul className="m-0 mt-1 flex list-none flex-wrap gap-2">
        {files.map((file, index) => (
          <li key={`${file.name}-${index}`}>
            <button
              type="button"
              className="h-7 max-w-[240px] truncate rounded-[8px] bg-[#f5f5f5] px-2 text-[13px] text-[#171717] hover:bg-[#f2f3f5]"
              onClick={() => downloadMailAttachment(file)}>
              {file.name}
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

function SignaturePanel({ onInsert, onClose }: { onInsert: (text: string) => void; onClose: () => void }) {
  const { t } = useTranslation()
  const [items, setItems] = useState<MailSignature[]>(() => loadSignatures())
  const [editing, setEditing] = useState<MailSignature | null>(null)
  const [managing, setManaging] = useState(false)
  const [name, setName] = useState('')
  const [text, setText] = useState('')
  const [error, setError] = useState('')
  const listRef = useRef<HTMLUListElement>(null)
  const nameRef = useRef<HTMLInputElement>(null)
  const editingRef = useRef(editing)
  editingRef.current = editing

  const store = (next: MailSignature[]) => {
    const saved = saveSignatures(next)
    setItems(saved)
    return saved
  }

  const begin = (signature: MailSignature | null) => {
    setEditing(signature ?? { id: '', name: '', text: '', isDefault: items.length === 0 })
    setName(signature?.name ?? '')
    setText(signature?.text ?? '')
    setError('')
  }

  const commit = () => {
    const nextName = name.trim().slice(0, SIGNATURE_NAME_CAP)
    const nextText = text.trim().slice(0, SIGNATURE_TEXT_CAP)
    if (!nextName || !nextText) return
    if (!editing?.id && items.length >= SIGNATURE_CAP) {
      setError(t('mail.compose.signature_too_many'))
      return
    }
    const id = editing?.id || newSignatureId()
    const row: MailSignature = {
      id,
      name: nextName,
      text: nextText,
      isDefault: editing?.isDefault ?? items.length === 0
    }
    const without = items.filter((item) => item.id !== id)
    store([...without, row])
    setEditing(null)
  }

  const saveChord = (event: ReactKeyboardEvent) => {
    if (!(event.metaKey || event.ctrlKey) || event.shiftKey || event.altKey || event.key.toLowerCase() !== 's') return
    event.preventDefault()
    event.stopPropagation()
    commit()
  }

  useEffect(() => {
    if (editing) nameRef.current?.focus()
  }, [editing])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      if (editingRef.current) {
        setEditing(null)
        return
      }
      onClose()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [onClose])

  const moveSignature = (event: ReactKeyboardEvent) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    const buttons = [...(listRef.current?.querySelectorAll<HTMLButtonElement>('[data-signature-insert]') ?? [])]
    if (buttons.length === 0) return
    event.preventDefault()
    const current = buttons.findIndex((button) => button === document.activeElement)
    const delta = event.key === 'ArrowDown' ? 1 : -1
    buttons[(current + delta + buttons.length) % buttons.length]?.focus()
  }

  return (
    <div
      id="mail-signature-menu"
      data-ui="mail.signatures"
      role="region"
      aria-label={t('mail.compose.signature')}
      className="absolute top-full right-0 z-30 mt-1 flex w-80 flex-col rounded-[8px] border border-[#efefef] bg-white py-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)]"
      onKeyDown={moveSignature}>
      {editing ? (
        <div className="flex flex-col gap-2 px-2.5 py-2">
          <label className="flex flex-col gap-1">
            <span className={orgSectionLabelClassName}>{t('mail.compose.signature_name')}</span>
            <input
              ref={nameRef}
              aria-label={t('mail.compose.signature_name')}
              value={name}
              maxLength={SIGNATURE_NAME_CAP}
              className={fieldClassName}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                saveChord(event)
                if (event.key !== 'Enter') return
                event.preventDefault()
                event.stopPropagation()
                commit()
              }}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className={orgSectionLabelClassName}>{t('mail.compose.signature_text')}</span>
            <textarea
              aria-label={t('mail.compose.signature_text')}
              value={text}
              maxLength={SIGNATURE_TEXT_CAP}
              rows={4}
              className={cn(fieldClassName, 'min-h-[88px] resize-y py-2')}
              onChange={(event) => setText(event.target.value)}
              onKeyDown={saveChord}
            />
          </label>
          {error ? (
            <p role="alert" className="m-0 text-[12px] leading-4 text-[#b42318]">
              {error}
            </p>
          ) : null}
          <div className="flex gap-2">
            <button type="button" className={filledButtonClassName(30)} onClick={commit}>
              {t('mail.compose.signature_save')}
            </button>
            <button type="button" className={quietButtonClassName(30)} onClick={() => setEditing(null)}>
              {t('mail.compose.cancel')}
            </button>
          </div>
        </div>
      ) : (
        <>
          {items.length === 0 ? (
            <p className="m-0 px-3 py-6 text-center text-[13px] text-[#8a8a8a]">{t('mail.compose.signature_empty')}</p>
          ) : (
            <ul ref={listRef} className="m-0 flex max-h-60 list-none flex-col overflow-auto px-1">
              {items.map((item) => (
                <li
                  key={item.id}
                  className="flex min-h-9 min-w-0 items-center gap-1 rounded-[8px] px-1.5 hover:bg-[#f7f7f7]">
                  {managing ? (
                    <span className="min-w-0 flex-1 truncate px-1 text-[13px] font-medium text-[#171717]">
                      {item.name}
                    </span>
                  ) : (
                    <button
                      type="button"
                      data-signature-insert=""
                      aria-label={`${t('mail.compose.signature_insert')} ${item.name}`}
                      className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-[8px] px-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10"
                      onClick={() => onInsert(item.text)}>
                      <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[#171717]">
                        {item.name}
                      </span>
                      <span className="shrink-0 text-[12px] text-[#8a8a8a]">{t('mail.compose.signature_insert')}</span>
                    </button>
                  )}
                  {item.isDefault ? (
                    <span className="shrink-0 text-[12px] text-[#8a8a8a]">{t('mail.compose.signature_default')}</span>
                  ) : null}
                  {managing ? (
                    <>
                      {item.isDefault ? null : (
                        <button
                          type="button"
                          className="h-6 shrink-0 px-1 text-[12px] text-[#8a8a8a] hover:text-[#171717]"
                          onClick={() => store(items.map((row) => ({ ...row, isDefault: row.id === item.id })))}>
                          {t('mail.compose.signature_set_default')}
                        </button>
                      )}
                      <button
                        type="button"
                        aria-label={t('mail.compose.signature_edit')}
                        className={rowActionClassName}
                        onClick={() => begin(item)}>
                        <Pencil size={14} strokeWidth={1.7} />
                      </button>
                      <PopConfirm
                        tone="danger"
                        title={t('mail.compose.signature_delete')}
                        description={item.name}
                        confirmLabel={t('mail.compose.signature_delete')}
                        cancelLabel={t('mail.compose.cancel')}
                        onConfirm={() => {
                          store(items.filter((row) => row.id !== item.id))
                        }}>
                        <button
                          type="button"
                          aria-label={t('mail.compose.signature_delete')}
                          className={cn(rowActionClassName, 'text-[#b42318]')}>
                          <Trash2 size={14} strokeWidth={1.7} />
                        </button>
                      </PopConfirm>
                    </>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
          <div className="flex items-center gap-2 border-t border-[#efefef] px-2.5 py-2">
            <button type="button" className={quietButtonClassName(30)} onClick={() => begin(null)}>
              {t('mail.compose.signature_add')}
            </button>
            {items.length > 0 ? (
              <button
                type="button"
                className="h-[30px] px-2 text-[13px] text-[#8a8a8a] hover:text-[#171717]"
                onClick={() => setManaging((open) => !open)}>
                {managing ? t('mail.compose.signature_done') : t('mail.compose.signature_manage')}
              </button>
            ) : null}
          </div>
          {items.length > 0 && !managing ? (
            <p className="m-0 px-2.5 pb-1.5 text-[12px] leading-4 text-[#8a8a8a]">{t('mail.compose.signature_keys')}</p>
          ) : null}
        </>
      )}
    </div>
  )
}

export function MailComposeSurface({
  address,
  initial,
  focusField,
  showTitle,
  teamOrgId,
  onSent,
  onClose,
  onRegisterDismiss,
  onDirtyChange
}: {
  address: string
  initial: ComposeState
  focusField: ComposeFocus
  showTitle: boolean
  teamOrgId?: string
  onSent: (message: MailMessage, draft: boolean) => void
  onClose: () => void
  onRegisterDismiss?: (dismiss: () => void) => void
  onDirtyChange?: (dirty: boolean) => void
}) {
  const { t } = useTranslation()
  const [compose, setCompose] = useState<ComposeState>(() => {
    const signature = defaultSignature()?.text ?? ''
    return {
      ...initial,
      attachments: initial.attachments ?? [],
      body: openingBody(initial.kind, initial.body, signature)
    }
  })
  const baseline = useRef(compose.body)
  const [showCc, setShowCc] = useState(initial.cc.length > 0 || initial.ccDraft.trim().length > 0)
  const [confirmDiscard, setConfirmDiscard] = useState(false)
  const [composeError, setComposeError] = useState('')
  const [sending, setSending] = useState(false)
  const [signaturesOpen, setSignaturesOpen] = useState(false)
  const [colleagues, setColleagues] = useState<MailColleague[] | null>(null)
  const [teamLoading, setTeamLoading] = useState(false)
  const [teamError, setTeamError] = useState('')
  const [pickerOpen, setPickerOpen] = useState(false)
  const [pickerField, setPickerField] = useState<'to' | 'cc'>('to')
  const [suggestOpen, setSuggestOpen] = useState(false)
  const [highlight, setHighlight] = useState(0)
  const [resolving, setResolving] = useState(false)
  const access = useTeamMailAccess(teamOrgId)
  const canPickTeam = access.pick
  const teamAuthority = access.authority
  const liveTeamAuthority = useRef(teamAuthority)
  liveTeamAuthority.current = teamAuthority
  const mailWriteEpoch = useRef(0)
  const openedMailAuthority = useRef<string | undefined>(undefined)
  useEffect(() => {
    mailWriteEpoch.current += 1
    if (access.ready) {
      if (
        (openedMailAuthority.current !== undefined && openedMailAuthority.current !== teamAuthority) ||
        (!access.create && !access.send) ||
        (initial.kind !== 'new' && !access.read)
      ) {
        baseline.current = ''
        setCompose({ ...EMPTY_COMPOSE })
        setComposeError(!access.create && !access.send ? t('mail.noAccess') : '')
      }
      openedMailAuthority.current = teamAuthority
    }
    setSending(false)
    return () => {
      mailWriteEpoch.current += 1
    }
  }, [teamAuthority, access.ready, access.create, access.send, access.read, initial.kind, t])
  const toRef = useRef<HTMLInputElement>(null)
  const ccRef = useRef<HTMLInputElement>(null)
  const subjectRef = useRef<HTMLInputElement>(null)
  const editorRef = useRef<MailEditorHandle>(null)
  const fileRef = useRef<HTMLInputElement>(null)
  const signatureRootRef = useRef<HTMLDivElement>(null)
  const focusCc = useRef(false)

  const dirty = composeDirty(compose, baseline.current)
  const requestClose = () => {
    if (sending || confirmDiscard) return
    if (!dirty) {
      onClose()
      return
    }
    setConfirmDiscard(true)
  }
  const dismissRef = useRef(requestClose)
  dismissRef.current = requestClose

  useEffect(() => {
    onRegisterDismiss?.(() => dismissRef.current())
  }, [onRegisterDismiss])

  useEffect(() => {
    onDirtyChange?.(dirty)
  }, [dirty, onDirtyChange])

  useEffect(() => {
    if (!showCc || !focusCc.current) return
    focusCc.current = false
    ccRef.current?.focus()
  }, [showCc])

  useEffect(() => {
    if (focusField === 'body') editorRef.current?.focus(initial.kind === 'draft' ? 'end' : 'start')
    else toRef.current?.focus()
  }, [focusField, initial.kind])

  useEffect(() => {
    if (!pickerOpen) return undefined
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      setPickerOpen(false)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [pickerOpen])

  useEffect(() => {
    if (!signaturesOpen) return undefined
    const onPointer = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node) || signatureRootRef.current?.contains(target)) return
      if (target instanceof Element && target.closest('.ui-popconfirm')) return
      setSignaturesOpen(false)
    }
    window.addEventListener('mousedown', onPointer)
    return () => window.removeEventListener('mousedown', onPointer)
  }, [signaturesOpen])

  const teamEpoch = useRef(0)
  useEffect(() => {
    teamEpoch.current += 1
    setPickerOpen(false)
    setSuggestOpen(false)
    setColleagues(null)
    setTeamLoading(false)
    setTeamError('')
    setResolving(false)
    return () => {
      teamEpoch.current += 1
    }
  }, [teamAuthority])
  const loadTeam = async () => {
    if (!teamOrgId || !canPickTeam) return
    const epoch = teamEpoch.current + 1
    teamEpoch.current = epoch
    const authority = teamAuthority
    setTeamLoading(true)
    setTeamError('')
    try {
      const rows = await listMailColleagues(teamOrgId)
      if (teamEpoch.current !== epoch || liveTeamAuthority.current !== authority) return
      setColleagues(rows)
    } catch {
      if (teamEpoch.current !== epoch || liveTeamAuthority.current !== authority) return
      setColleagues(null)
      setTeamError(t('mail.compose.pick_error'))
    } finally {
      if (teamEpoch.current === epoch) setTeamLoading(false)
    }
  }

  const requestTeam = () => {
    if (!teamOrgId || !canPickTeam) return
    void loadTeam()
  }

  const takenIds = useMemo(() => {
    const ids = new Set<string>()
    for (const token of [...compose.to, ...compose.cc]) {
      if (token.userId) ids.add(token.userId)
    }
    return ids
  }, [compose.to, compose.cc])

  const suggestions = useMemo(() => {
    if (!canPickTeam || !suggestOpen || !colleagues) return []
    const draft = compose.toDraft.trim()
    if (!draft || draft.includes('@')) return []
    return filterColleagues(colleagues, draft)
      .filter((row) => !takenIds.has(row.userId))
      .slice(0, 8)
      .map((row) => ({ id: row.userId, name: row.name, detail: row.position }))
  }, [canPickTeam, suggestOpen, colleagues, compose.toDraft, takenIds])

  const pickUsers = async (userIds: string[], field: 'to' | 'cc') => {
    if (!teamOrgId || !canPickTeam || userIds.length === 0 || resolving) return
    const authority = teamAuthority
    const epoch = teamEpoch.current
    setResolving(true)
    setComposeError('')
    try {
      const resolved = await resolveMailColleagues(teamOrgId, userIds)
      if (liveTeamAuthority.current !== authority || teamEpoch.current !== epoch) return
      if (resolved.length !== userIds.length) {
        setComposeError(t('mail.compose.pick_missing'))
        return
      }
      setCompose((current) => {
        const people = resolved.map((row) => ({ address: row.address, name: row.name, userId: row.userId }))
        if (field === 'to') return { ...current, to: addNamedRecipients(current.to, people), toDraft: '' }
        return { ...current, cc: addNamedRecipients(current.cc, people), ccDraft: '' }
      })
      setSuggestOpen(false)
      setPickerOpen(false)
      setHighlight(0)
    } catch (cause) {
      if (liveTeamAuthority.current !== authority || teamEpoch.current !== epoch) return
      const message = cause instanceof Error ? cause.message.trim() : ''
      setComposeError(message ? composeFailure(message, t) : t('mail.compose.pick_error'))
    } finally {
      if (liveTeamAuthority.current === authority && teamEpoch.current === epoch) setResolving(false)
    }
  }

  const updateCompose = (recipe: (current: ComposeState) => ComposeState) => {
    setComposeError('')
    setCompose((current) => recipe(current))
  }

  const addFiles = async (list: FileList | null) => {
    if (!list) return
    let next = compose.attachments
    let problem: AttachmentProblem | null = null
    for (const file of Array.from(list)) {
      const read = await readMailAttachment(file)
      if ('error' in read) {
        problem = read.error
        break
      }
      problem = attachmentProblem(next, file.size)
      if (problem) break
      next = [...next, read.file]
    }
    setCompose((current) => ({ ...current, attachments: next }))
    setComposeError(problem ? attachmentMessage(problem, t) : '')
    if (fileRef.current) fileRef.current.value = ''
  }

  const submit = async (draft: boolean) => {
    if (!teamOrgId || (draft ? !access.create : !access.send) || sending || resolving || pickerOpen) return
    const policy = teamAuthority
    const epoch = mailWriteEpoch.current
    const to = flushTokens(compose.to, toRef.current?.value ?? compose.toDraft)
    const ccAll = flushTokens(compose.cc, ccRef.current?.value ?? compose.ccDraft)
    const cc = ccAll.filter((item) => !to.some((row) => row.value === item.value))
    const flushed: ComposeState = { ...compose, to, toDraft: '', cc: ccAll, ccDraft: '' }
    const invalid = to.some((item) => !item.valid) || cc.some((item) => !item.valid)
    const count = to.length + cc.length
    if (invalid || count > RECIPIENT_CAP || compose.body.length > BODY_CAP) {
      setCompose(flushed)
      setComposeError('')
      return
    }
    const toAddresses = to.map((item) => item.value)
    const ccAddresses = cc.map((item) => item.value)
    if (!draft && toAddresses.length + ccAddresses.length === 0) return
    setSending(true)
    setComposeError('')
    try {
      const message = await sendMail(teamOrgId, {
        to: toAddresses,
        cc: ccAddresses,
        subject: compose.subject,
        body: mailBodyText(compose.body) ? compose.body : '',
        draft,
        ...(compose.threadId ? { thread_id: compose.threadId } : {}),
        ...(compose.attachments.length > 0 ? { attachments: compose.attachments } : {})
      })
      if (mailWriteEpoch.current !== epoch || liveTeamAuthority.current !== policy) return
      if (access.remove && compose.draftId && compose.draftId !== message.id) {
        try {
          await setMailState(teamOrgId, compose.draftId, { folder: 'trash' })
        } catch {
          // The new copy is saved. The previous draft stays until the next edit.
        }
      }
      if (mailWriteEpoch.current !== epoch || liveTeamAuthority.current !== policy) return
      onSent(message, draft)
    } catch (cause) {
      if (mailWriteEpoch.current !== epoch || liveTeamAuthority.current !== policy) return
      const message = cause instanceof Error ? cause.message.trim() : ''
      setComposeError(message ? composeFailure(message, t) : t('mail.error'))
    } finally {
      if (mailWriteEpoch.current === epoch && liveTeamAuthority.current === policy) setSending(false)
    }
  }

  const advanceFrom = (field: 'to' | 'cc') => {
    if (field === 'to' && showCc) ccRef.current?.focus()
    else subjectRef.current?.focus()
  }

  const listed = listedRecipients(compose)
  const recipientNotice = listed.bad
    ? t('mail.compose.invalid')
    : listed.outside
      ? t('mail.compose.outside')
      : listed.count > RECIPIENT_CAP
        ? t('mail.compose.limit')
        : compose.body.length > BODY_CAP
          ? t('mail.compose.body_limit')
          : ''
  const canSend =
    access.send &&
    !sending &&
    !resolving &&
    !pickerOpen &&
    !listed.invalid &&
    listed.count <= RECIPIENT_CAP &&
    listed.count > 0 &&
    compose.subject.length <= SUBJECT_CAP &&
    compose.body.length <= BODY_CAP
  const draftBlocked = !access.create || sending || resolving || pickerOpen || recipientNotice !== ''
  const composeTitle =
    compose.kind === 'reply'
      ? t('mail.reply')
      : compose.kind === 'draft'
        ? t('mail.compose.continue')
        : t('mail.compose')
  const tools: ReactNode = (
    <>
      <button
        type="button"
        title={t('mail.compose.attach')}
        aria-label={t('mail.compose.attach')}
        className="inline-flex size-8 items-center justify-center rounded-lg text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => fileRef.current?.click()}>
        <Paperclip size={16} strokeWidth={1.7} />
      </button>
      <div ref={signatureRootRef} className="relative">
        <button
          type="button"
          title={t('mail.compose.signature')}
          aria-label={t('mail.compose.signature')}
          aria-pressed={signaturesOpen}
          aria-expanded={signaturesOpen}
          aria-controls={signaturesOpen ? 'mail-signature-menu' : undefined}
          className={cn(
            'inline-flex size-8 items-center justify-center rounded-lg text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10',
            signaturesOpen && 'bg-[#f2f3f5] text-[#171717]'
          )}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setSignaturesOpen((open) => !open)}>
          <PenLine size={16} strokeWidth={1.7} />
        </button>
        {signaturesOpen ? (
          <SignaturePanel
            onClose={() => setSignaturesOpen(false)}
            onInsert={(text) => {
              editorRef.current?.appendSignature(signatureHtml(text))
              setSignaturesOpen(false)
            }}
          />
        ) : null}
      </div>
    </>
  )

  const activeSuggestion = suggestions.length === 0 ? 0 : Math.min(highlight, suggestions.length - 1)
  const openTeam = (field: 'to' | 'cc') => {
    if (!canPickTeam) return
    setPickerField(field)
    setSuggestOpen(false)
    setPickerOpen(true)
    requestTeam()
  }

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <form
        className={cn('flex min-h-0 flex-1 flex-col', pickerOpen && 'pointer-events-none invisible')}
        aria-hidden={pickerOpen || undefined}
        onSubmit={(event) => {
          event.preventDefault()
          void submit(false)
        }}
        onKeyDown={(event) => {
          if (pickerOpen) return
          const mod = event.metaKey || event.ctrlKey
          const target = event.target
          const inOverlay =
            target instanceof HTMLElement &&
            Boolean(target.closest('[data-ui="mail.signatures"], [data-ui="mail.link"]'))
          if (mod && !event.altKey && event.key === 'Enter' && !inOverlay) {
            event.preventDefault()
            void submit(false)
            return
          }
          if (mod && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 's') {
            event.preventDefault()
            if (!inOverlay) void submit(true)
            return
          }
          if (mod && event.shiftKey && !event.altKey && event.key.toLowerCase() === 'c' && !inOverlay) {
            event.preventDefault()
            if (showCc) ccRef.current?.focus()
            else {
              focusCc.current = true
              setShowCc(true)
            }
            return
          }
          if (event.key === 'Enter' && target instanceof HTMLInputElement) event.preventDefault()
        }}>
        {showTitle ? (
          <div className="flex shrink-0 flex-col gap-1 border-b border-[#efefef] px-6 py-4 pr-14">
            <DialogTitle className="truncate text-[15px] leading-5 font-medium text-[#171717]">
              {composeTitle}
            </DialogTitle>
            <p className="m-0 truncate text-[12px] leading-4 text-[#8a8a8a]" title={address}>
              {t('mail.compose.from')} {address}
            </p>
            <DialogDescription className="sr-only">{t('mail.compose')}</DialogDescription>
          </div>
        ) : (
          <p
            className="m-0 truncate border-b border-[#efefef] py-2 pr-14 pl-6 text-[12px] leading-4 text-[#8a8a8a]"
            title={address}>
            {t('mail.compose.from')} {address}
          </p>
        )}
        <div className="flex shrink-0 flex-col gap-2 px-6 py-3">
          <div className="flex min-w-0 items-start gap-2">
            <div className="flex min-h-9 min-w-0 flex-1 items-start rounded-[8px] bg-[#f5f5f5] focus-within:ring-2 focus-within:ring-black/10">
              <span className="flex h-9 w-16 shrink-0 items-center pl-2.5 text-[13px] text-[#8a8a8a]">
                {t('mail.compose.to')}
              </span>
              <RecipientField
                id="mail-compose-to"
                label={t('mail.compose.to')}
                tokens={compose.to}
                draft={compose.toDraft}
                inputRef={toRef}
                invalid={compose.to.some((item) => !item.valid)}
                describedBy={recipientNotice ? 'mail-compose-notice' : undefined}
                placeholder={t('mail.compose.to_placeholder')}
                suggestions={suggestions}
                highlight={activeSuggestion}
                onHighlight={(index) => {
                  if (index < 0) {
                    setSuggestOpen(false)
                    return
                  }
                  setHighlight(index)
                }}
                onPick={(id) => void pickUsers([id], 'to')}
                onFocus={requestTeam}
                onChange={(tokens, draft) => {
                  setHighlight(0)
                  setSuggestOpen(canPickTeam && draft.trim().length > 0 && !draft.includes('@'))
                  if (canPickTeam && draft.trim() && !draft.includes('@') && !colleagues && !teamLoading) requestTeam()
                  updateCompose((current) => ({ ...current, to: tokens, toDraft: draft }))
                }}
                onAdvance={() => advanceFrom('to')}
              />
              {canPickTeam ? (
                <button
                  type="button"
                  aria-label={t('mail.compose.pick')}
                  title={t('mail.compose.pick')}
                  className="mt-1.5 mr-1.5 grid size-6 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
                  onClick={() => openTeam('to')}>
                  <Users size={14} strokeWidth={1.7} />
                </button>
              ) : null}
            </div>
            {showCc ? null : (
              <button
                type="button"
                title={`${t('mail.compose.cc')} ${chordLabel('Mod+Shift+C')}`}
                aria-keyshortcuts={chordKeys('Mod+Shift+C')}
                className="h-9 shrink-0 rounded-lg px-2 text-[13px] text-[#8a8a8a] hover:text-[#171717]"
                onClick={() => {
                  focusCc.current = true
                  setShowCc(true)
                }}>
                {t('mail.compose.cc')}
              </button>
            )}
          </div>
          {showCc ? (
            <div className="flex min-h-9 min-w-0 items-start rounded-[8px] bg-[#f5f5f5] focus-within:ring-2 focus-within:ring-black/10">
              <span className="flex h-9 w-16 shrink-0 items-center pl-2.5 text-[13px] text-[#8a8a8a]">
                {t('mail.compose.cc')}
              </span>
              <RecipientField
                id="mail-compose-cc"
                label={t('mail.compose.cc')}
                tokens={compose.cc}
                draft={compose.ccDraft}
                inputRef={ccRef}
                invalid={compose.cc.some((item) => !item.valid)}
                describedBy={recipientNotice ? 'mail-compose-notice' : undefined}
                placeholder={t('mail.compose.cc_placeholder')}
                onFocus={requestTeam}
                onChange={(tokens, draft) => updateCompose((current) => ({ ...current, cc: tokens, ccDraft: draft }))}
                onAdvance={() => advanceFrom('cc')}
              />
              {canPickTeam ? (
                <button
                  type="button"
                  aria-label={`${t('mail.compose.cc')} ${t('mail.compose.pick')}`}
                  title={t('mail.compose.pick')}
                  className="mt-1.5 mr-1.5 grid size-6 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
                  onClick={() => openTeam('cc')}>
                  <Users size={14} strokeWidth={1.7} />
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="flex h-9 min-w-0 items-center rounded-[8px] bg-[#f5f5f5] focus-within:ring-2 focus-within:ring-black/10">
            <label
              htmlFor="mail-compose-subject"
              className="flex h-9 w-16 shrink-0 items-center pl-2.5 text-[13px] text-[#8a8a8a]">
              {t('mail.compose.subject')}
            </label>
            <input
              id="mail-compose-subject"
              ref={subjectRef}
              aria-label={t('mail.compose.subject')}
              placeholder={t('mail.compose.subject_placeholder')}
              value={compose.subject}
              maxLength={SUBJECT_CAP}
              className="h-9 min-w-0 flex-1 border-0 bg-transparent px-1 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
              onChange={(event) => {
                const subject = event.target.value.slice(0, SUBJECT_CAP)
                updateCompose((current) => ({ ...current, subject }))
              }}
              onKeyDown={(event) => {
                if (event.key !== 'Enter' || event.metaKey || event.ctrlKey) return
                event.preventDefault()
                editorRef.current?.focus(compose.kind === 'draft' ? 'end' : 'start')
              }}
            />
            {compose.subject.length >= 160 ? (
              <span className="shrink-0 pr-2.5 text-[12px] text-[#8a8a8a]">
                {compose.subject.length}/{SUBJECT_CAP}
              </span>
            ) : null}
          </div>
        </div>
        {recipientNotice || composeError ? (
          <p
            id="mail-compose-notice"
            role="alert"
            className={cn(
              'm-0 px-6 pb-3 text-[12px] leading-4',
              listed.outside && !listed.bad && !composeError ? 'text-[#3f3f3f]' : 'text-[#b42318]'
            )}>
            {recipientNotice || composeError}
          </p>
        ) : null}
        <MailEditor
          seed={baseline.current}
          autoFocus={focusField === 'body'}
          place={compose.kind === 'draft' ? 'end' : 'start'}
          editorRef={editorRef}
          tools={tools}
          onChange={(body) => {
            setComposeError('')
            setCompose((current) => (current.body === body ? current : { ...current, body }))
          }}
        />
        <FileChips
          files={compose.attachments}
          onRemove={(index) =>
            updateCompose((current) => ({
              ...current,
              attachments: current.attachments.filter((_, item) => item !== index)
            }))
          }
        />
        <input
          ref={fileRef}
          type="file"
          multiple
          tabIndex={-1}
          className="sr-only"
          onChange={(event) => void addFiles(event.target.files)}
        />
        <div className="flex shrink-0 items-center gap-3 border-t border-[#efefef] px-6 py-4">
          <button
            type="button"
            disabled={draftBlocked}
            aria-keyshortcuts={chordKeys('Mod+S')}
            className={quietButtonClassName(36)}
            onClick={() => void submit(true)}>
            {t('mail.compose.save')}
            <span aria-hidden className="font-mono text-[12px] text-[#8a8a8a]">
              {chordLabel('Mod+S')}
            </span>
          </button>
          {compose.body.length >= 90_000 ? (
            <span className="text-[12px] text-[#8a8a8a]">
              {compose.body.length}/{BODY_CAP}
            </span>
          ) : null}
          <button
            type="submit"
            disabled={!canSend}
            aria-keyshortcuts={chordKeys('Mod+Enter')}
            className={cn(filledButtonClassName(36), 'ml-auto font-medium')}>
            {t('mail.compose.send')}
            <span aria-hidden className="font-mono text-[12px] text-white/70">
              {commandEnterLabel()}
            </span>
          </button>
        </div>
      </form>
      <MailTeamPanel
        open={Boolean(pickerOpen && canPickTeam && teamOrgId)}
        rows={colleagues}
        loading={teamLoading}
        loadError={teamError}
        taken={takenIds}
        busy={resolving}
        onRetry={() => void loadTeam()}
        onCancel={() => setPickerOpen(false)}
        onConfirm={(userIds) => void pickUsers(userIds, pickerField)}
      />
      <ConfirmDialog
        open={confirmDiscard}
        onOpenChange={setConfirmDiscard}
        title={t('mail.compose.discard_title')}
        description={t('mail.compose.discard_body')}
        confirmText={t('mail.compose.discard')}
        cancelText={t('mail.compose.cancel')}
        destructive
        overlayClassName="z-[140]"
        contentClassName="z-[140] bg-white text-[#171717]"
        onConfirm={onClose}
      />
    </div>
  )
}
