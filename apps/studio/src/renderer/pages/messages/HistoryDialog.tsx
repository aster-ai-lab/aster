import { Calendar, Dialog, DialogContent, DialogTitle, SearchInput } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import type { ImStore } from '@renderer/im/imStore'
import { cn } from '@renderer/utils/style'

import {
  dayKey,
  filterHistory,
  highlightParts,
  type HistoryFilter,
  type HistoryRange,
  type HistoryRecord
} from './history'
import { PersonAvatar } from './messageAvatars'

const FILTERS: readonly HistoryFilter[] = ['all', 'file', 'image', 'link']
const FIRST_FILTER: HistoryFilter = 'all'

function parseDay(value: string): Date | undefined {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) return undefined
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const date = new Date(year, month - 1, day)
  return dayKey(date.getTime()) === value ? date : undefined
}

function ResultThumb({ store, id, name }: { store: ImStore; id: string; name: string }) {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    store.attachmentUrl(id).then(
      (next) => {
        if (live) setUrl(next)
      },
      () => {
        if (live) setUrl(null)
      }
    )
    return () => {
      live = false
    }
  }, [id, store])
  if (!url) return <span className="block size-10 shrink-0 rounded-[4px] bg-[#f5f5f5]" />
  return <img alt={name} src={url} className="size-10 shrink-0 rounded-[4px] object-cover" />
}

/** In-conversation history. All lists every record; the other tabs keep one kind. */
export function HistoryDialog({
  open,
  onOpenChange,
  name,
  records,
  formatTime,
  loading,
  store,
  onPick
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  name: string
  records: readonly HistoryRecord[]
  formatTime: (at: number) => string
  loading: boolean
  store: ImStore
  onPick: (id: string) => void
}) {
  const { t, i18n } = useTranslation()
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<HistoryFilter>(FIRST_FILTER)
  const [range, setRange] = useState<HistoryRange | null>(null)
  const [draft, setDraft] = useState<{ from?: Date; to?: Date }>({})
  const [dateOpen, setDateOpen] = useState(false)
  const datePresence = useRestingDialog(dateOpen)
  const results = filterHistory(records, query, filter, range)
  const filtered = Boolean(query.trim()) || filter !== 'all' || range !== null
  const filterLabel: Record<(typeof FILTERS)[number], string> = {
    all: t('messages.history.all'),
    file: t('messages.history.file'),
    image: t('messages.history.image'),
    link: t('messages.history.link')
  }

  const clearFilters = () => {
    setQuery('')
    setFilter(FIRST_FILTER)
    setRange(null)
    setDraft({})
    setDateOpen(false)
  }

  useEffect(() => {
    if (open) return
    setQuery('')
    setFilter(FIRST_FILTER)
    setRange(null)
    setDraft({})
    setDateOpen(false)
  }, [open])

  const openRange = () => {
    setDraft({
      from: range ? parseDay(range.start) : undefined,
      to: range ? parseDay(range.end) : undefined
    })
    setDateOpen(true)
  }

  const chooseKind = (item: HistoryFilter) => {
    setDateOpen(false)
    if (filter !== item) setQuery('')
    setFilter(item)
  }

  const chooseDate = () => {
    if (dateOpen) {
      setDateOpen(false)
      return
    }
    setQuery('')
    openRange()
  }

  const applyRange = () => {
    if (!draft.from) return
    const start = dayKey(draft.from.getTime())
    const end = dayKey((draft.to ?? draft.from).getTime())
    setRange(start <= end ? { start, end } : { start: end, end: start })
    setDateOpen(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        size="xl"
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        className="flex h-[560px] flex-col gap-0 bg-white p-0 sm:max-w-[720px]">
        <div className="border-b border-[#f0f0f0] px-5 pt-4 pr-12 pb-3">
          <DialogTitle className="m-0 truncate text-[15px] font-medium text-[#171717]">
            {t('messages.history.title', { name })}
          </DialogTitle>
          <SearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('messages.history.search')}
            placeholder={t('messages.history.search')}
            containerClassName="mt-3 h-[32px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
          />
          <div className="mt-3 flex items-center gap-4">
            {FILTERS.map((item) => (
              <button
                key={item}
                type="button"
                aria-pressed={filter === item}
                className={cn(
                  'h-7 border-b-2 text-[13px] outline-none',
                  filter === item
                    ? 'border-[#171717] font-medium text-[#171717]'
                    : 'border-transparent text-[#3f3f3f] hover:text-[#171717]'
                )}
                onClick={() => chooseKind(item)}>
                {filterLabel[item]}
              </button>
            ))}
            <button
              type="button"
              aria-expanded={dateOpen}
              aria-pressed={range !== null || dateOpen}
              className={cn(
                'h-7 border-b-2 text-[13px] outline-none',
                range || dateOpen
                  ? 'border-[#171717] font-medium text-[#171717]'
                  : 'border-transparent text-[#3f3f3f] hover:text-[#171717]'
              )}
              onClick={chooseDate}>
              {t('messages.history.date')}
            </button>
            {filtered && results.length > 0 ? (
              <button
                type="button"
                data-ui="messages.history-clear"
                className="ml-auto h-7 shrink-0 text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717]"
                onClick={clearFilters}>
                {t('messages.history.clear')}
              </button>
            ) : null}
          </div>
        </div>
        {/* presentation keeps row buttons activatable. WKWebView drops AXPress inside an overflow group. */}
        <div
          data-ui="messages.history"
          role="presentation"
          className="relative min-h-0 flex-1 overflow-y-auto px-3 py-2">
          {datePresence.present ? (
            <div className={cn('absolute inset-0 z-10', datePresence.state === 'closed' && 'pointer-events-none')}>
              <button
                type="button"
                aria-hidden
                tabIndex={-1}
                className="absolute inset-0 cursor-default"
                onClick={() => setDateOpen(false)}
              />
              <div
                role="dialog"
                aria-label={t('messages.history.range')}
                data-ui="messages.history-range"
                data-state={datePresence.state}
                className="a3s-dialog-surface absolute top-2 left-1/2 z-10 w-[320px] -translate-x-1/2 rounded-[12px] border border-[#ebebeb] bg-white p-3 shadow-[0_8px_22px_rgba(20,40,80,0.12)]">
                <span className="sr-only">{t('messages.history.range')}</span>
                <Calendar
                  mode="range"
                  weekStartsOn={1}
                  captionLayout="label"
                  navLayout="around"
                  lang={i18n.language}
                  showOutsideDays
                  defaultMonth={draft.from ?? new Date()}
                  selected={draft.from ? { from: draft.from, to: draft.to } : undefined}
                  onSelect={(next) => setDraft(next ?? {})}
                  formatters={{
                    formatCaption: (month) =>
                      t('messages.history.caption', { year: month.getFullYear(), month: month.getMonth() + 1 }),
                    formatWeekdayName: (weekday) => weekday.toLocaleDateString(i18n.language, { weekday: 'narrow' })
                  }}
                  labels={{
                    labelPrevious: () => t('messages.history.prev'),
                    labelNext: () => t('messages.history.next'),
                    labelDayButton: (date) => dayKey(date.getTime())
                  }}
                  className="w-full p-0"
                />
                <div className="mt-2 flex items-center justify-end gap-2">
                  {range ? (
                    <button
                      type="button"
                      className="mr-auto h-8 rounded-[8px] px-2 text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717]"
                      onClick={() => {
                        setRange(null)
                        setDateOpen(false)
                      }}>
                      {t('messages.history.range_clear')}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="h-8 rounded-[8px] border border-[#e6e6e6] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#f7f7f7]"
                    onClick={() => setDateOpen(false)}>
                    {t('common.cancel')}
                  </button>
                  <button
                    type="button"
                    aria-label={t('common.confirm')}
                    className={cn(
                      'h-8 rounded-[8px] px-3 text-[13px] outline-none',
                      draft.from ? 'bg-[#171717] text-white' : 'bg-[#f0f0f0] text-[#8a8a8a]'
                    )}
                    onClick={() => {
                      if (!draft.from) return
                      applyRange()
                    }}>
                    {t('common.confirm')}
                  </button>
                </div>
              </div>
            </div>
          ) : null}
          {range ? (
            <button
              type="button"
              aria-label={t('messages.history.range_clear')}
              className="mb-1 px-2 text-left text-[12px] text-[#8a8a8a] outline-none hover:text-[#171717]"
              onClick={() => setRange(null)}>
              {range.start === range.end ? range.start : `${range.start} – ${range.end}`}
            </button>
          ) : null}
          {results.length === 0 ? (
            <div className="flex flex-col items-center px-2 py-10">
              <p className="m-0 text-center text-[13px] text-[#8a8a8a]">
                {loading
                  ? t('messages.state.loading')
                  : filtered
                    ? t('messages.history.no_match')
                    : t('messages.history.empty')}
              </p>
              {filtered ? (
                <button
                  type="button"
                  data-ui="messages.history-clear"
                  className="mt-3 h-8 rounded-[8px] border border-[#e6e6e6] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#f7f7f7]"
                  onClick={clearFilters}>
                  {t('messages.history.clear')}
                </button>
              ) : null}
            </div>
          ) : (
            <ul className="m-0 flex list-none flex-col p-0">
              {results.map((record) => (
                <li key={record.key}>
                  <button
                    type="button"
                    data-ui="messages.history-result"
                    className="flex h-14 w-full items-center gap-3 overflow-hidden rounded-[8px] px-2 text-left outline-none hover:bg-[#f7f7f7]"
                    onClick={() => onPick(record.id)}>
                    <PersonAvatar
                      name={record.senderName}
                      phone={record.phone}
                      avatar={record.avatar ?? undefined}
                      colorKey={record.colorKey}
                      size={36}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] text-[#171717]">{record.senderName}</span>
                      {record.thumb ? (
                        <ResultThumb store={store} id={record.id} name={record.text} />
                      ) : (
                        <span className="block truncate text-[13px] text-[#3f3f3f]">
                          {highlightParts(record.text, query).map((part, index) =>
                            part.hit ? (
                              <mark key={index} className="bg-transparent font-medium text-[#4c6fff]">
                                {part.text}
                              </mark>
                            ) : (
                              <span key={index}>{part.text}</span>
                            )
                          )}
                        </span>
                      )}
                    </span>
                    {record.at ? (
                      <span className="shrink-0 text-[12px] text-[#8a8a8a]">{formatTime(record.at)}</span>
                    ) : null}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
