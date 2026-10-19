import { Check } from 'lucide-react'
import { useEffect, useId, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import type { MailColleague } from '@renderer/mail/mailApi'
import { cn } from '@renderer/utils/style'

import { PersonAvatar } from '../messages/messageAvatars'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName } from '../org/orgUi'

const fieldClassName = cn(orgFieldClassName, 'outline-none focus-visible:ring-2 focus-visible:ring-black/10')

export function filterColleagues(rows: readonly MailColleague[], query: string): MailColleague[] {
  const needle = query.trim().toLocaleLowerCase()
  if (!needle) return [...rows]
  return rows.filter((row) => `${row.name} ${row.position}`.toLocaleLowerCase().includes(needle))
}

/** In-window directory. It stays inside the compose surface so it does not dismiss the letter. */
export function MailTeamPanel({
  open,
  rows,
  loading,
  loadError,
  taken,
  busy,
  onRetry,
  onCancel,
  onConfirm
}: {
  open: boolean
  rows: MailColleague[] | null
  loading: boolean
  loadError: string
  taken: ReadonlySet<string>
  busy: boolean
  onRetry: () => void
  onCancel: () => void
  onConfirm: (userIds: string[]) => void
}) {
  const { t } = useTranslation()
  const titleId = useId()
  const searchId = useId()
  const presence = useRestingDialog(open)
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<string[]>([])
  const visible = useMemo(() => filterColleagues(rows ?? [], query), [rows, query])

  useEffect(() => {
    if (!open) return
    setQuery('')
    setSelected([])
    document.getElementById(searchId)?.focus()
  }, [open, searchId])

  const toggle = (userId: string) => {
    if (taken.has(userId) || busy) return
    setSelected((current) => (current.includes(userId) ? current.filter((id) => id !== userId) : [...current, userId]))
  }

  if (!presence.present) return null

  return (
    <section
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      data-ui="mail.team"
      data-state={presence.state}
      className={cn(
        'a3s-dialog-surface absolute inset-0 z-10 flex flex-col bg-white',
        presence.state === 'closed' && 'pointer-events-none'
      )}>
      <header className="flex h-14 shrink-0 items-center gap-2 border-b border-[#efefef] px-4">
        <button type="button" className={quietButtonClassName(36)} onClick={onCancel}>
          {t('mail.compose.cancel')}
        </button>
        <h2
          id={titleId}
          className="m-0 min-w-0 flex-1 truncate text-center text-[15px] leading-5 font-medium text-[#171717]">
          {t('mail.compose.pick_title')}
        </h2>
        <button
          type="button"
          className={filledButtonClassName(36)}
          disabled={selected.length === 0 || busy}
          onClick={() => onConfirm(selected)}>
          {t('mail.compose.pick_add')}
        </button>
      </header>
      <div className="shrink-0 px-6 py-3">
        <input
          id={searchId}
          aria-label={t('mail.compose.pick_search')}
          placeholder={t('mail.compose.pick_search')}
          value={query}
          className={cn(fieldClassName, 'placeholder:text-[#8a8a8a]')}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {loading && !rows ? (
          <p className="m-0 px-6 py-8 text-center text-[13px] text-[#8a8a8a]">{t('mail.compose.pick_loading')}</p>
        ) : null}
        {!rows && !loading && loadError ? (
          <div className="flex flex-col items-center gap-3 px-6 py-8">
            <p role="alert" className="m-0 text-center text-[13px] text-[#b42318]">
              {loadError}
            </p>
            <button type="button" className={quietButtonClassName(36)} onClick={onRetry}>
              {t('mail.retry')}
            </button>
          </div>
        ) : null}
        {!loading && !loadError && rows && visible.length === 0 ? (
          <p className="m-0 px-6 py-8 text-center text-[13px] text-[#8a8a8a]">{t('mail.compose.pick_empty')}</p>
        ) : null}
        {!loadError && rows
          ? visible.map((row) => {
              const already = taken.has(row.userId)
              const on = already || selected.includes(row.userId)
              return (
                <button
                  key={row.userId}
                  type="button"
                  aria-pressed={on}
                  disabled={already || busy}
                  className={cn(
                    'flex h-12 w-full items-center gap-3 border-b border-[#efefef] px-6 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:opacity-40',
                    on ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
                  )}
                  onClick={() => toggle(row.userId)}>
                  <PersonAvatar name={row.name} avatar={row.avatar} colorKey={row.userId} className="size-8" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-medium text-[#1a1a1a]">{row.name}</span>
                    {row.position ? (
                      <span className="block truncate text-[12px] leading-4 text-[#8a8a8a]">{row.position}</span>
                    ) : null}
                  </span>
                  {on ? <Check size={14} strokeWidth={1.7} aria-hidden className="shrink-0 text-[#171717]" /> : null}
                </button>
              )
            })
          : null}
      </div>
    </section>
  )
}
