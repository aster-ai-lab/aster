import { Dialog, DialogContent, EmptyState, PopConfirm } from '@a3s-lab/ui'
import {
  Check,
  Copy,
  FileEdit,
  Flag,
  FlagOff,
  Inbox,
  Mail,
  RefreshCw,
  Reply,
  ReplyAll,
  Send,
  ShieldAlert,
  Trash2,
  type LucideIcon
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import {
  listMail,
  loadMailbox,
  openMail,
  setMailState,
  type MailFolder,
  type MailMessage,
  type MailSummary
} from '@renderer/mail/mailApi'
import { publishMailUnread } from '@renderer/mail/mailUnread'
import { useTeamMailAccess } from '@renderer/mail/useTeamMailAccess'
import { toast } from '@renderer/services/toast'

import {
  composeSeed as toComposeSeed,
  continueDraft,
  teamMailOrgId,
  EMPTY_COMPOSE,
  replyAllDraft,
  replyDraft,
  sameRecipients,
  subjectText,
  whenLabel,
  type ComposeFocus,
  type ComposeState
} from './mailCompose'
import { MailAttachmentList, MailComposeSurface } from './MailComposeSurface'
import { mailBodyText, quoteMailBody } from './mailDocument'
import { MailMessageBody } from './MailEditor'
import { mailComposeWindowAvailable, openMailComposeWindow } from './openMailComposeWindow'

const FOLDERS: MailFolder[] = ['inbox', 'unread', 'flagged', 'drafts', 'sent', 'spam', 'trash']

const FOLDER_ICONS: Record<MailFolder, LucideIcon> = {
  inbox: Inbox,
  unread: Mail,
  flagged: Flag,
  drafts: FileEdit,
  sent: Send,
  spam: ShieldAlert,
  trash: Trash2
}

function quotedBody(message: MailMessage): string {
  return quoteMailBody(message.from_name || message.from_address, message.body)
}

function MailAction({
  icon: Icon,
  label,
  pressed,
  onClick
}: {
  icon: LucideIcon
  label: string
  pressed?: boolean
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      className="inline-flex h-8 items-center gap-1.5 rounded-lg px-2 text-[13px] text-[#3f3f3f] hover:bg-[#f2f3f5] hover:text-[#171717]"
      onClick={onClick}>
      <Icon size={15} strokeWidth={1.7} aria-hidden />
      {label}
    </button>
  )
}

export default function MailPage() {
  const { t } = useTranslation()
  const [folder, setFolder] = useState<MailFolder>('inbox')
  const [query, setQuery] = useState('')
  const [applied, setApplied] = useState('')
  const [address, setAddress] = useState('')
  const [items, setItems] = useState<MailSummary[]>([])
  const [opened, setOpened] = useState<MailMessage | null>(null)
  const [compose, setCompose] = useState<ComposeState | null>(null)
  const [focusField, setFocusField] = useState<ComposeFocus>('to')
  const [composeSeed, setComposeSeed] = useState(0)
  const [loading, setLoading] = useState(true)
  const [pageError, setPageError] = useState('')
  const [reloadKey, setReloadKey] = useState(0)
  const [copied, setCopied] = useState(false)
  const copyTimer = useRef<number | null>(null)
  const askClose = useRef<() => void>(() => undefined)
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const teamOrgId = teamMailOrgId(centerId)
  const access = useTeamMailAccess(teamOrgId)
  const canCompose = access.create || access.send
  const authority = useRef(access.authority)
  authority.current = access.authority
  const readEpoch = useRef(0)
  const writeEpoch = useRef(0)
  useEffect(() => {
    readEpoch.current += 1
    writeEpoch.current += 1
    setItems([])
    setOpened(null)
    setAddress('')
    setCompose(null)
    publishMailUnread(0)
    return () => {
      readEpoch.current += 1
      writeEpoch.current += 1
    }
  }, [access.authority])

  useEffect(() => {
    const timer = window.setTimeout(() => setApplied(query.trim()), 200)
    return () => window.clearTimeout(timer)
  }, [query])

  useEffect(
    () => () => {
      if (copyTimer.current !== null) window.clearTimeout(copyTimer.current)
    },
    []
  )

  const copyAddress = async () => {
    if (!address) return
    try {
      await navigator.clipboard.writeText(address)
    } catch {
      const node = document.createElement('textarea')
      node.value = address
      node.setAttribute('readonly', '')
      document.body.appendChild(node)
      node.select()
      document.execCommand('copy')
      node.remove()
    }
    setCopied(true)
    toast.success({ title: t('message.copy.success'), appearance: 'pill', key: 'mail.copy-address' })
    if (copyTimer.current !== null) window.clearTimeout(copyTimer.current)
    copyTimer.current = window.setTimeout(() => setCopied(false), 1600)
  }

  const load = useCallback(async () => {
    const epoch = ++readEpoch.current
    const policy = access.authority
    if (!teamOrgId || !access.read) {
      setLoading(false)
      return
    }
    setLoading(true)
    setPageError('')
    try {
      const [mailbox, page] = await Promise.all([loadMailbox(teamOrgId), listMail(teamOrgId, folder, applied)])
      if (epoch !== readEpoch.current || authority.current !== policy) return
      setAddress(mailbox.address)
      setItems(page.items)
      publishMailUnread(page.unread)
      setOpened((current) => (current && page.items.some((item) => item.id === current.id) ? current : null))
    } catch (cause) {
      if (epoch !== readEpoch.current || authority.current !== policy) return
      setItems([])
      setOpened(null)
      setAddress('')
      publishMailUnread(0)
      setPageError(cause instanceof Error && cause.message ? cause.message : t('mail.error'))
    } finally {
      if (epoch === readEpoch.current && authority.current === policy) setLoading(false)
    }
  }, [applied, folder, t, teamOrgId, access.read, access.authority])

  useEffect(() => {
    void load()
    return () => {
      readEpoch.current += 1
    }
  }, [load, reloadKey])

  useEffect(() => {
    const refresh = () => setReloadKey((value) => value + 1)
    window.addEventListener('a3s:mail-changed', refresh)
    return () => window.removeEventListener('a3s:mail-changed', refresh)
  }, [])

  const showDialog = (next: ComposeState, field: ComposeFocus) => {
    setFocusField(field)
    setComposeSeed((value) => value + 1)
    setCompose(next)
  }

  const openCompose = (next: ComposeState, field: ComposeFocus) => {
    if (!canCompose || !teamOrgId) return
    if (next.kind !== 'new' && !access.read) return
    const policy = access.authority
    const title =
      next.kind === 'reply' ? t('mail.reply') : next.kind === 'draft' ? t('mail.compose.continue') : t('mail.compose')
    if (!mailComposeWindowAvailable()) {
      showDialog(next, field)
      return
    }
    void openMailComposeWindow(toComposeSeed(next, field, address, title, teamOrgId ?? '')).then((openedWindow) => {
      if (!openedWindow && authority.current === policy) showDialog(next, field)
    })
  }

  const closeCompose = () => {
    setCompose(null)
  }

  const chooseFolder = (next: MailFolder) => {
    setFolder(next)
    setOpened(null)
  }

  const openItem = async (id: string) => {
    if (!teamOrgId || !access.read) return
    const epoch = ++readEpoch.current
    const policy = access.authority
    setPageError('')
    try {
      const message = await openMail(teamOrgId, id)
      if (readEpoch.current !== epoch || authority.current !== policy) return
      setOpened(message)
      setItems((current) => current.map((item) => (item.id === id ? { ...item, unread: message.unread } : item)))
      if (folder === 'inbox' || folder === 'unread') {
        const page = await listMail(teamOrgId, folder, applied, 1)
        if (readEpoch.current === epoch && authority.current === policy) publishMailUnread(page.unread)
      }
    } catch (cause) {
      if (readEpoch.current !== epoch || authority.current !== policy) return
      setPageError(cause instanceof Error && cause.message ? cause.message : t('mail.error'))
    }
  }

  const patch = async (patchBody: { unread?: boolean; flagged?: boolean; folder?: MailFolder }) => {
    if (!opened || !teamOrgId) return
    if (
      patchBody.folder === 'trash'
        ? !access.remove || opened.can_delete !== true
        : !access.update || opened.can_update !== true
    )
      return
    const epoch = ++writeEpoch.current
    const policy = access.authority
    setPageError('')
    try {
      const message = await setMailState(teamOrgId, opened.id, patchBody)
      if (writeEpoch.current !== epoch || authority.current !== policy) return
      setOpened(message)
      setReloadKey((value) => value + 1)
    } catch (cause) {
      if (writeEpoch.current !== epoch || authority.current !== policy) return
      setPageError(cause instanceof Error && cause.message ? cause.message : t('mail.error'))
    }
  }

  const quote = opened ? quotedBody(opened) : ''
  const plainReply = opened ? replyDraft(opened, quote) : null
  const allReply = opened ? replyAllDraft(opened, address, quote) : null
  const offerReplyAll = plainReply !== null && allReply !== null && !sameRecipients(plainReply, allReply)
  const emptyLabel = applied ? t('mail.no_match') : t('mail.empty')
  const canUpdateOpened = access.update && opened?.can_update === true
  const canDeleteOpened = access.remove && opened?.can_delete === true

  return (
    <div data-ui="mail.page" className="flex h-full min-h-0 w-full flex-1 overflow-hidden bg-white text-[#171717]">
      <nav
        data-ui="mail.folders"
        aria-label={t('mail.title')}
        className="flex w-[260px] shrink-0 flex-col border-r border-[#efefef] px-2 py-3">
        <button
          type="button"
          disabled={!canCompose}
          title={canCompose ? undefined : t('mail.noAccess')}
          className="mx-1 h-9 rounded-lg bg-[#171717] text-[13px] font-medium text-white hover:bg-black disabled:cursor-not-allowed disabled:opacity-40"
          onClick={() => openCompose(EMPTY_COMPOSE, 'to')}>
          {t('mail.compose')}
        </button>
        <div className="px-2.5 pt-3 pb-2">
          <h1 className="m-0 text-[16px] font-medium">{t('mail.title')}</h1>
          <div className="mt-1 flex min-w-0 items-center gap-1">
            <p className="m-0 min-w-0 flex-1 truncate text-[12px] text-[#8a8a8a] select-text" title={address}>
              {address}
            </p>
            {address ? (
              <button
                type="button"
                data-ui="mail.copy-address"
                aria-label={copied ? t('mail.copied') : t('mail.copy')}
                className="grid size-6 shrink-0 place-items-center rounded-md text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]"
                onClick={() => void copyAddress()}>
                {copied ? <Check size={14} strokeWidth={1.7} /> : <Copy size={14} strokeWidth={1.7} />}
              </button>
            ) : null}
          </div>
        </div>
        <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
          {FOLDERS.map((id) => {
            const Icon = FOLDER_ICONS[id]
            const selected = folder === id
            return (
              <button
                key={id}
                type="button"
                aria-current={selected ? 'page' : undefined}
                className={`flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] ${selected ? 'bg-[#f2f3f5] font-medium' : 'hover:bg-[#f7f7f7]'}`}
                onClick={() => chooseFolder(id)}>
                <Icon size={16} strokeWidth={1.7} />
                <span className="min-w-0 flex-1 truncate">{t(`mail.folder.${id}`)}</span>
              </button>
            )
          })}
        </div>
      </nav>
      <section className="flex w-[360px] shrink-0 flex-col border-r border-[#efefef]">
        <div className="flex items-center gap-2 border-b border-[#efefef] px-3 py-2">
          <input
            data-ui="mail.search"
            value={query}
            aria-label={t('mail.search')}
            placeholder={t('mail.search')}
            className="h-[30px] min-w-0 flex-1 rounded-lg border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
            onChange={(event) => setQuery(event.target.value)}
          />
          <button
            type="button"
            aria-label={t('mail.refresh')}
            className="grid size-[30px] place-items-center rounded-lg text-[#171717] hover:bg-[#f7f7f7]"
            onClick={() => setReloadKey((value) => value + 1)}>
            <RefreshCw size={16} strokeWidth={1.7} />
          </button>
        </div>
        <div data-ui="mail.list" className="min-h-0 flex-1 overflow-y-auto">
          {loading && items.length === 0 ? (
            <p className="m-0 px-3 py-3 text-[13px] text-[#8a8a8a]">{t('mail.loading')}</p>
          ) : null}
          {!loading && items.length === 0 ? (
            <div
              data-ui={applied ? 'mail.no-match' : 'mail.empty'}
              className="flex h-full min-h-[240px] items-center justify-center">
              <EmptyState
                compact
                title={access.read ? emptyLabel : t('mail.noAccess')}
                actionLabel={applied ? t('mail.clear_search') : undefined}
                onAction={applied ? () => setQuery('') : undefined}
              />
            </div>
          ) : null}
          {(access.read ? items : []).map((item) => {
            const selected = opened?.id === item.id
            const snippet = mailBodyText(item.preview)
            return (
              <button
                key={item.id}
                type="button"
                aria-pressed={selected}
                className={`flex w-full flex-col gap-0.5 border-b border-[#efefef] px-3 py-2 text-left hover:bg-[#f7f7f7] ${selected ? 'bg-[#f2f3f5]' : ''}`}
                onClick={() => void openItem(item.id)}>
                <span className="flex items-center gap-2">
                  <span className={`min-w-0 flex-1 truncate text-[13px] ${item.unread ? 'font-medium' : ''}`}>
                    {item.from_name || item.from_address}
                  </span>
                  <span className="shrink-0 text-[12px] text-[#8a8a8a]">{whenLabel(item.created_at)}</span>
                </span>
                <span className="flex items-center gap-2">
                  <span
                    className={`min-w-0 flex-1 truncate text-[13px] ${item.unread ? 'font-medium' : 'text-[#8a8a8a]'}`}>
                    {subjectText(item.subject, t('mail.no_subject'))}
                  </span>
                  {item.flagged ? <Flag size={14} strokeWidth={1.7} className="shrink-0" /> : null}
                </span>
                {snippet ? <span className="truncate text-[12px] leading-4 text-[#8a8a8a]">{snippet}</span> : null}
              </button>
            )
          })}
        </div>
      </section>
      <section className="flex min-h-0 min-w-0 flex-1 flex-col">
        {pageError ? (
          <p role="alert" className="m-0 border-b border-[#efefef] px-4 py-2 text-[13px] text-[#171717]">
            {pageError}{' '}
            <button type="button" className="underline" onClick={() => setReloadKey((value) => value + 1)}>
              {t('mail.retry')}
            </button>
          </p>
        ) : null}
        {opened && access.read ? (
          <article data-ui="mail.reader" className="flex min-h-0 flex-1 flex-col">
            <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-[#efefef] px-4 py-3">
              <h2 className="m-0 min-w-0 flex-1 truncate text-[16px] font-medium">
                {subjectText(opened.subject, t('mail.no_subject'))}
              </h2>
              <div data-ui="mail.actions" className="flex flex-wrap items-center justify-end gap-0.5">
                {opened.folder === 'drafts' && canCompose ? (
                  <MailAction
                    icon={FileEdit}
                    label={t('mail.compose.continue')}
                    onClick={() => openCompose(continueDraft(opened), opened.body.trim() ? 'body' : 'to')}
                  />
                ) : access.send && opened.folder !== 'drafts' ? (
                  <MailAction
                    icon={Reply}
                    label={t('mail.reply')}
                    onClick={() => plainReply && openCompose(plainReply, 'body')}
                  />
                ) : null}
                {access.send && opened.folder !== 'drafts' && offerReplyAll && allReply ? (
                  <MailAction
                    icon={ReplyAll}
                    label={t('mail.reply_all')}
                    onClick={() => openCompose(allReply, 'body')}
                  />
                ) : null}
                {canUpdateOpened ? (
                  <MailAction
                    icon={opened.flagged ? FlagOff : Flag}
                    label={opened.flagged ? t('mail.action.unflag') : t('mail.action.flag')}
                    pressed={opened.flagged}
                    onClick={() => void patch({ flagged: !opened.flagged })}
                  />
                ) : null}
                {!canUpdateOpened || opened.unread ? null : (
                  <MailAction
                    icon={Mail}
                    label={t('mail.action.unread')}
                    onClick={() => void patch({ unread: true })}
                  />
                )}
                <span className="mx-1 h-4 w-px bg-[#efefef]" aria-hidden />
                {!canUpdateOpened ? null : opened.folder === 'spam' || opened.folder === 'trash' ? (
                  <MailAction
                    icon={Inbox}
                    label={t('mail.action.restore')}
                    onClick={() => void patch({ folder: 'inbox' })}
                  />
                ) : (
                  <MailAction
                    icon={ShieldAlert}
                    label={t('mail.action.spam')}
                    onClick={() => void patch({ folder: 'spam' })}
                  />
                )}
                {!canDeleteOpened || opened.folder === 'trash' ? null : (
                  <PopConfirm
                    tone="danger"
                    title={t('mail.action.delete_confirm')}
                    description={t('mail.action.delete_hint')}
                    confirmLabel={t('mail.action.delete')}
                    cancelLabel={t('common.cancel')}
                    onConfirm={() => patch({ folder: 'trash' })}>
                    <MailAction icon={Trash2} label={t('mail.action.delete')} />
                  </PopConfirm>
                )}
              </div>
            </header>
            <p className="m-0 px-4 pt-3 text-[13px] text-[#8a8a8a]">
              {t('mail.from')} {opened.from_name ? `${opened.from_name} ` : ''}
              {opened.from_address}
            </p>
            {opened.to.length > 0 ? (
              <p className="m-0 px-4 text-[13px] text-[#8a8a8a]">
                {t('mail.compose.to')} {opened.to.join(', ')}
              </p>
            ) : null}
            {opened.cc.length > 0 ? (
              <p className="m-0 px-4 text-[13px] text-[#8a8a8a]">
                {t('mail.compose.cc')} {opened.cc.join(', ')}
              </p>
            ) : null}
            <MailAttachmentList files={opened.attachments} />
            <MailMessageBody body={opened.body} />
          </article>
        ) : (
          <div data-ui="mail.reader-empty" className="flex min-h-0 flex-1 flex-col">
            {loading ? (
              <p className="m-0 px-6 py-16 text-center text-[13px] text-[#8a8a8a]">{t('mail.loading')}</p>
            ) : (
              <EmptyState title={t('mail.select')} description={canCompose ? t('mail.select_hint') : undefined} />
            )}
          </div>
        )}
      </section>
      <Dialog
        open={compose !== null}
        onOpenChange={(next) => {
          if (!next) askClose.current()
        }}>
        {/* The tab layer sits under the icon rail. Fixed would center on the viewport and slide underneath it. */}
        <DialogContent
          data-ui="mail.compose"
          closeLabel={t('common.close')}
          size="xl"
          overlayClassName="absolute z-[120]"
          className="absolute z-[120] flex max-h-[min(640px,calc(100%-2rem))] flex-col gap-0 overflow-hidden bg-white p-0 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]"
          onEscapeKeyDown={(event) => {
            const target = event.target
            if (target instanceof HTMLElement && target.closest('[data-ui="mail.link"], [data-ui="mail.signatures"]'))
              event.preventDefault()
          }}
          onOpenAutoFocus={(event) => {
            event.preventDefault()
          }}>
          {compose ? (
            <MailComposeSurface
              key={composeSeed}
              address={address}
              initial={compose}
              focusField={focusField}
              showTitle
              teamOrgId={teamOrgId}
              onRegisterDismiss={(dismiss) => {
                askClose.current = dismiss
              }}
              onClose={closeCompose}
              onSent={(message, draft) => {
                closeCompose()
                setOpened(message)
                setFolder(draft ? 'drafts' : 'sent')
                setReloadKey((value) => value + 1)
              }}
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </div>
  )
}
