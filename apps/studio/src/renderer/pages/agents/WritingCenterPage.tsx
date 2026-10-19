import { ChevronRight, ClipboardList, FileText, Home, Megaphone, NotebookPen } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SearchInput, SegmentedControl } from '@a3s-lab/ui'
import Scrollbar from '@renderer/components/Scrollbar'
import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'
import { cn } from '@renderer/utils/style'

import {
  hasMoreWriting,
  visibleWritingEntries,
  WRITING_CATEGORIES,
  type WritingCategory,
  type WritingEntry,
  type WritingShelf
} from './writingCenterData'

const VIEW_OPTIONS = [
  { value: 'templates', labelKey: 'workbench.writing.tab.templates' },
  { value: 'used', labelKey: 'workbench.writing.tab.used' }
] as const

type CenterView = (typeof VIEW_OPTIONS)[number]['value']

const CATEGORY_ICONS = {
  home: Home,
  notice: Megaphone,
  summary: ClipboardList,
  request: FileText,
  minutes: NotebookPen
} as const satisfies Record<WritingCategory, typeof Home>

const COVER_BAR = {
  notice: 'bg-[#69b9c9]',
  summary: 'bg-[#4c6fff]',
  request: 'bg-[#69b9c9]',
  minutes: 'bg-[#e07848]'
} as const satisfies Record<WritingShelf, string>

function DocumentCover({ genre, shelf }: { genre: string; shelf: WritingShelf }) {
  return (
    <div className="flex h-full items-center justify-center bg-[#f6f7f9]">
      <div className="w-[148px] overflow-hidden rounded-[6px] border border-[#ececec] bg-white shadow-[0_8px_22px_rgba(20,40,80,0.04)]">
        <div className={cn('h-1.5', COVER_BAR[shelf])} />
        <div className="px-3 pt-2 text-[12px] font-medium text-[#1a1a1a]">{genre}</div>
        <div className="space-y-1.5 px-3 pt-2 pb-3">
          <div className="h-[3px] rounded bg-[#e6e7ee]" />
          <div className="h-[3px] w-4/5 rounded bg-[#e6e7ee]" />
          <div className="h-[3px] w-3/5 rounded bg-[#e6e7ee]" />
        </div>
      </div>
    </div>
  )
}

function HeroPapers({ notice, summary, request }: { notice: string; summary: string; request: string }) {
  return (
    <div className="pointer-events-none absolute inset-y-0 right-0 hidden w-[420px] sm:block" aria-hidden>
      <div className="absolute top-5 right-[186px] w-[168px] -rotate-6 rounded-[12px] border border-white/80 bg-white/95 p-3 shadow-[0_8px_18px_rgba(40,80,140,0.08)]">
        <div className="h-1.5 w-10 rounded bg-[#69b9c9]" />
        <div className="mt-2 text-[13px] font-medium text-[#1a1a1a]">{notice}</div>
        <div className="mt-2 h-2 rounded bg-[#eef3f8]" />
        <div className="mt-1.5 h-2 w-4/5 rounded bg-[#eef3f8]" />
      </div>
      <div className="absolute top-[58px] right-8 w-[190px] rotate-2 rounded-[14px] bg-white p-3 shadow-[0_12px_28px_rgba(40,80,140,0.14)]">
        <div className="h-1.5 w-10 rounded bg-[#4c6fff]" />
        <div className="mt-2 text-[13px] font-medium text-[#1a1a1a]">{summary}</div>
        <div className="mt-1 text-[12px] text-[#8a8a8a]">{request}</div>
        <div className="mt-2 h-2 rounded bg-[#eef3f8]" />
        <div className="mt-1.5 h-2 w-3/5 rounded bg-[#eef3f8]" />
      </div>
    </div>
  )
}

export function WritingCenterPage({
  used,
  onDraft
}: {
  used: ReadonlySet<string>
  onDraft: (entry: WritingEntry) => void
}) {
  const { t } = useTranslation()
  const [view, setView] = useState<CenterView>('templates')
  const [category, setCategory] = useState<WritingCategory>('home')
  const [expanded, setExpanded] = useState(false)
  const [query, setQuery] = useState('')
  const scrollRef = useRef<HTMLDivElement>(null)
  const collectionRef = useRef<HTMLElement>(null)
  const skipTopReset = useRef(false)
  const [collectionRequest, setCollectionRequest] = useState(0)

  const searching = query.trim().length > 0
  const needle = query.trim().toLocaleLowerCase()
  const shelf = visibleWritingEntries(category, searching || expanded || view === 'used')
  const scoped = view === 'used' ? shelf.filter((entry) => used.has(entry.id)) : shelf
  const entries = searching
    ? scoped.filter((entry) => `${t(entry.titleKey)} ${t(entry.descriptionKey)}`.toLocaleLowerCase().includes(needle))
    : scoped
  const showHero = view === 'templates' && category === 'home' && !searching
  const heading = category === 'home' ? t('workbench.writing.collection.title') : t(`workbench.writing.category.${category}`)

  useEffect(() => {
    if (skipTopReset.current) {
      skipTopReset.current = false
      return
    }
    scrollRef.current?.scrollTo?.({ top: 0 })
  }, [category])

  useEffect(() => {
    if (collectionRequest === 0) return
    collectionRef.current?.scrollIntoView?.({ block: 'start' })
  }, [collectionRequest])

  const showCollection = () => {
    skipTopReset.current = true
    setCategory('home')
    setExpanded(false)
    setCollectionRequest((current) => current + 1)
  }

  return (
    <div data-ui="writing.center" className="flex h-full min-h-0 flex-col bg-white">
      <div className="shrink-0 bg-white">
        <div className="flex flex-col gap-4 px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
                {t('workbench.writing.title')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">
                <SearchInput
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onClear={() => setQuery('')}
                  clearLabel={t('common.clear')}
                  placeholder={t('workbench.writing.search')}
                  aria-label={t('workbench.writing.search')}
                  containerClassName="h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]"
                />
                <SegmentedControl
                  aria-label={t('workbench.writing.title')}
                  size="sm"
                  className="h-[30px]"
                  value={view}
                  options={VIEW_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
                  onValueChange={setView}
                />
              </div>
            </div>
            <p className="m-0 text-[15px] text-muted-foreground">{t('workbench.writing.subtitle')}</p>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <nav
          aria-label={t('workbench.writing.nav')}
          className={cn(
            'flex flex-col gap-0.5 bg-white px-3 pt-2 pb-4',
            pageSidebarWidthClassName,
            pageSidebarEdgeClassName
          )}>
          {WRITING_CATEGORIES.map((id) => {
            const Icon = CATEGORY_ICONS[id]
            const selected = id === category
            return (
              <button
                key={id}
                type="button"
                aria-current={selected ? 'page' : undefined}
                onClick={() => {
                  skipTopReset.current = false
                  setCategory(id)
                  if (id === 'home') setExpanded(false)
                }}
                className={cn(
                  'flex h-9 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
                  'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                  selected
                    ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                    : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                )}>
                <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                <span className="truncate">{t(`workbench.writing.category.${id}`)}</span>
              </button>
            )
          })}
        </nav>

        <Scrollbar ref={scrollRef} className="min-h-0 min-w-0 flex-1">
          <div className="px-6 pt-4 pb-6">
            {showHero ? (
              <section className="relative min-h-[168px] overflow-hidden rounded-[16px] bg-[radial-gradient(80%_130%_at_100%_0%,#c5dcff_0%,#e4f0ff_42%,#f6f9ff_100%)] px-8 py-6">
                <div className="relative z-10 max-w-[340px]">
                  <p className="m-0 text-[22px] leading-snug font-semibold text-[#1a1a1a]">
                    {t('workbench.writing.hero.title')}
                  </p>
                  <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">{t('workbench.writing.hero.subtitle')}</p>
                  <button
                    type="button"
                    onClick={showCollection}
                    className="mt-5 h-8 rounded-[8px] border border-[#e6e6e6] bg-white px-3.5 text-[13px] text-[#171717] shadow-[0_1px_2px_rgba(20,40,80,0.04)] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                    {t('workbench.writing.hero.action')}
                  </button>
                </div>
                <HeroPapers
                  notice={t('workbench.writing.category.notice')}
                  summary={t('workbench.writing.category.summary')}
                  request={t('workbench.writing.category.request')}
                />
              </section>
            ) : null}

            <section ref={collectionRef} className={cn(showHero ? 'mt-7' : 'mt-1')}>
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <h2 className="m-0 text-[18px] font-semibold text-[#1a1a1a]">{heading}</h2>
                  {showHero ? (
                    <p className="m-0 mt-1 text-[13px] text-[#8a8a8a]">{t('workbench.writing.collection.subtitle')}</p>
                  ) : null}
                </div>
                {!searching && view === 'templates' && hasMoreWriting(category, expanded) ? (
                  <button
                    type="button"
                    onClick={() => setExpanded(true)}
                    className="mt-0.5 flex shrink-0 items-center gap-0.5 text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
                    {t('workbench.writing.more')}
                    <ChevronRight className="size-3.5 text-current" />
                  </button>
                ) : null}
              </div>

              {entries.length === 0 ? (
                <p className="m-0 py-16 text-center text-sm text-muted-foreground">{t('workbench.writing.empty')}</p>
              ) : (
                <div className="mt-4 grid grid-cols-2 gap-4 min-[1080px]:grid-cols-4">
                  {entries.map((entry) => {
                    const drafted = used.has(entry.id)
                    return (
                      <article
                        key={entry.id}
                        data-ui={`writing.card.${entry.id}`}
                        className="flex min-w-0 flex-col overflow-hidden rounded-[14px] border border-[#ececec] bg-white shadow-[0_8px_22px_rgba(20,40,80,0.04)]">
                        <div className="h-[132px]">
                          <DocumentCover genre={t(`workbench.writing.category.${entry.category}`)} shelf={entry.category} />
                        </div>
                        <div className="flex flex-1 flex-col px-4 pt-3 pb-3.5">
                          <h3 className="m-0 truncate text-[14px] font-medium text-[#1a1a1a]">{t(entry.titleKey)}</h3>
                          <p className="m-0 mt-1.5 line-clamp-2 min-h-10 text-[12px] leading-5 text-[#8b8b8b]">
                            {t(entry.descriptionKey)}
                          </p>
                          <div className="mt-3 flex items-center justify-between gap-2">
                            <span className="truncate text-[13px] text-[#171717]">
                              {t(`workbench.writing.category.${entry.category}`)}
                            </span>
                            <button
                              type="button"
                              data-ui="writing.draft"
                              aria-pressed={drafted}
                              onClick={() => onDraft(entry)}
                              className="h-[30px] shrink-0 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                              {drafted ? t('workbench.writing.drafted') : t('workbench.writing.draft')}
                            </button>
                          </div>
                        </div>
                      </article>
                    )
                  })}
                </div>
              )}
            </section>
          </div>
        </Scrollbar>
      </div>
    </div>
  )
}
