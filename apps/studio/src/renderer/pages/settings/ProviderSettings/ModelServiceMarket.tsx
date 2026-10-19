import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SearchInput } from '@a3s-lab/ui'
import { loadTenantModelMarket, type TenantMarketModel } from '@renderer/data/tenantModelMarket'
import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'
import { cn } from '@renderer/utils/style'

import {
  EMPTY_MARKET_QUERY,
  filterServiceModels,
  marketCategories,
  marketFiltersActive,
  marketTags,
  marketVendors,
  type MarketQuery,
  type MarketView
} from './marketFilters'

/** The model-service page when the tenant publishes a catalog. Users browse available models. */
export default function ModelServiceMarket() {
  const { t } = useTranslation()
  const [models, setModels] = useState<TenantMarketModel[] | null>(null)
  const [query, setQuery] = useState<MarketQuery>(EMPTY_MARKET_QUERY)
  const [view, setView] = useState<MarketView>('cards')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let live = true
    void loadTenantModelMarket().then((next) => {
      if (live) setModels(next)
    })
    return () => {
      live = false
    }
  }, [])

  const catalog = models ?? []
  const vendors = useMemo(() => marketVendors(catalog), [catalog])
  const categories = useMemo(() => marketCategories(catalog), [catalog])
  const tags = useMemo(() => marketTags(catalog), [catalog])
  const visible = useMemo(() => filterServiceModels(catalog, query), [catalog, query])
  const selected = catalog.find((model) => model.id === selectedId) ?? null
  const filtersOn = marketFiltersActive(query)

  const setFilter = (patch: Partial<MarketQuery>) => {
    setSelectedId(null)
    setQuery((current) => ({ ...current, ...patch }))
  }

  return (
    <div data-ui="settings.model-service" className="flex h-full min-h-0 w-full min-w-0 bg-white text-[#171717]">
      <aside
        className={cn('flex flex-col gap-4 overflow-y-auto bg-white px-3 pt-4 pb-6', pageSidebarWidthClassName, pageSidebarEdgeClassName)}
        aria-label={t('settings.models.market_filter')}>
        <div className="flex items-center justify-between px-2.5">
          <h2 className="m-0 text-[12px] text-[#8a8a8a]">{t('settings.models.market_filter')}</h2>
          {filtersOn ? (
            <button type="button" className="text-[13px] text-[#8a8a8a] hover:text-[#171717]" onClick={() => setFilter({ vendor: '', category: '', tag: '' })}>
              {t('settings.models.market_reset')}
            </button>
          ) : null}
        </div>
        <FilterGroup
          title={t('settings.models.market_vendor')}
          allLabel={t('settings.models.market_all')}
          value={query.vendor}
          options={vendors}
          onChange={(vendor) => setFilter({ vendor })}
        />
        <FilterGroup
          title={t('settings.models.market_type')}
          allLabel={t('settings.models.market_all')}
          value={query.category}
          options={categories}
          onChange={(category) => setFilter({ category })}
        />
        <FilterGroup
          title={t('settings.models.market_tags')}
          allLabel={t('settings.models.market_all')}
          value={query.tag}
          options={tags}
          onChange={(tag) => setFilter({ tag })}
        />
      </aside>
      <div className="min-w-0 flex-1 overflow-y-auto px-6 pt-4 pb-8">
        {selected ? (
          <ModelDetail
            model={selected}
            copied={copied}
            onBack={() => {
              setCopied(false)
              setSelectedId(null)
            }}
            onCopy={() => {
              void navigator.clipboard?.writeText(selected.id).then(() => setCopied(true))
            }}
          />
        ) : (
          <>
            <h1 className="m-0 text-[24px] leading-tight font-bold text-foreground">{t('settings.models.market_title')}</h1>
            <p className="m-0 mt-1 text-[15px] text-[#8a8a8a]">
              {t('settings.models.market_count', { count: catalog.length })}
            </p>
            <SearchInput
              value={query.query}
              onChange={(event) => setFilter({ query: event.target.value })}
              onClear={() => setFilter({ query: '' })}
              clearLabel={t('common.clear')}
              placeholder={t('settings.models.market_search')}
              containerClassName="mt-4 h-[30px] w-full border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
            />
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <span className="text-[13px] text-[#8a8a8a]">
                {t('settings.models.market_shown', { shown: visible.length, total: catalog.length })}
              </span>
              <button
                type="button"
                className="h-[30px] rounded-[8px] px-2.5 text-[13px] text-[#3f3f3f] hover:bg-[#f7f7f7]"
                onClick={() => setQuery((current) => ({ ...current, sort: current.sort === 'name-asc' ? 'name-desc' : 'name-asc' }))}>
                {t('settings.models.market_sort_name')}
                {query.sort === 'name-desc' ? ' ↓' : ' ↑'}
              </button>
              <div className="ml-auto flex items-center gap-0.5">
                <ViewChip label={t('settings.models.market_view_cards')} selected={view === 'cards'} onSelect={() => setView('cards')} />
                <ViewChip label={t('settings.models.market_view_list')} selected={view === 'list'} onSelect={() => setView('list')} />
              </div>
            </div>
            {models !== null && visible.length === 0 ? (
              <p className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">
                {catalog.length === 0 ? t('settings.models.market_empty') : t('settings.models.market_empty_filtered')}
              </p>
            ) : view === 'cards' ? (
              <ul className="m-0 mt-4 grid list-none grid-cols-1 gap-3 p-0 xl:grid-cols-2">
                {visible.map((model) => (
                  <li key={model.id}>
                    <ModelCard model={model} onOpen={() => setSelectedId(model.id)} />
                  </li>
                ))}
              </ul>
            ) : (
              <ul className="m-0 mt-4 list-none p-0">
                {visible.map((model) => (
                  <li key={model.id} className="border-b border-[#f0f0f0]">
                    <button
                      type="button"
                      data-ui="settings.model-market.row"
                      className="flex w-full items-center gap-4 px-1 py-3 text-left hover:bg-[#f7f7f7]"
                      onClick={() => setSelectedId(model.id)}>
                      <span className="min-w-0 flex-1 truncate text-[14px] font-medium text-[#1a1a1a]">{model.name}</span>
                      <span className="w-24 shrink-0 truncate text-[13px] text-[#8a8a8a]">{model.vendor}</span>
                      <span className="w-16 shrink-0 text-[13px] text-[#8a8a8a]">{model.category}</span>
                      <span className="hidden w-28 shrink-0 truncate text-[13px] text-[#8a8a8a] sm:block">{model.tags.join(' ')}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>
    </div>
  )
}

function FilterGroup({
  title,
  allLabel,
  value,
  options,
  onChange
}: {
  title: string
  allLabel: string
  value: string
  options: string[]
  onChange: (value: string) => void
}) {
  if (options.length === 0) return null
  return (
    <div>
      <div className="px-2.5 pb-1 text-[12px] text-[#8a8a8a]">{title}</div>
      <div role="radiogroup" aria-label={title} className="flex flex-col">
        <FilterRow label={allLabel} selected={value === ''} onSelect={() => onChange('')} />
        {options.map((option) => (
          <FilterRow key={option} label={option} selected={value === option} onSelect={() => onChange(option)} />
        ))}
      </div>
    </div>
  )
}

function FilterRow({ label, selected, onSelect }: { label: string; selected: boolean; onSelect: () => void }) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      className={cn(
        'h-9 rounded-[8px] px-2.5 text-left text-[13px] outline-none',
        'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'
      )}>
      {label}
    </button>
  )
}

function ViewChip({ label, selected, onSelect }: { label: string; selected: boolean; onSelect: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      className={cn(
        'h-[30px] rounded-[8px] px-2.5 text-[13px]',
        selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#6f6f6f] hover:bg-[#f7f7f7]'
      )}>
      {label}
    </button>
  )
}

function ModelCard({ model, onOpen }: { model: TenantMarketModel; onOpen: () => void }) {
  const { t } = useTranslation()
  return (
    <article data-ui="settings.model-market.card" className="flex h-full flex-col rounded-[14px] border border-[#ececec] bg-white p-4">
      <h3 className="m-0 truncate text-[14px] font-medium text-[#1a1a1a]" title={model.name}>
        {model.name}
      </h3>
      {model.vendor ? <p className="m-0 mt-1 text-[12px] text-[#8a8a8a]">{model.vendor}</p> : null}
      {model.summary ? <p className="m-0 mt-2 line-clamp-2 text-[13px] leading-5 text-[#8a8a8a]">{model.summary}</p> : null}
      <p className="m-0 mt-3 text-[12px] text-[#6f6f6f]">{[model.category, model.endpoint, ...model.tags].filter(Boolean).join(' · ')}</p>
      <button type="button" className="mt-3 self-start text-[13px] text-[#8a8a8a] hover:text-[#171717]" onClick={onOpen}>
        {t('settings.models.market_details')}
      </button>
    </article>
  )
}

function ModelDetail({
  model,
  copied,
  onBack,
  onCopy
}: {
  model: TenantMarketModel
  copied: boolean
  onBack: () => void
  onCopy: () => void
}) {
  const { t } = useTranslation()
  return (
    <section data-ui="settings.model-market.detail" aria-labelledby="settings-model-detail-title">
      <button type="button" className="text-[13px] text-[#8a8a8a] hover:text-[#171717]" onClick={onBack}>
        {t('settings.models.market_back')}
      </button>
      <h1 id="settings-model-detail-title" className="m-0 mt-3 text-[24px] leading-tight font-bold text-foreground">
        {model.name}
      </h1>
      {model.vendor ? <p className="m-0 mt-1 text-[15px] text-[#8a8a8a]">{model.vendor}</p> : null}
      {model.summary ? <p className="m-0 mt-4 max-w-[640px] text-[14px] leading-6 text-[#3f3f3f]">{model.summary}</p> : null}
      <dl className="mt-6 grid max-w-[640px] grid-cols-[96px_minmax(0,1fr)] gap-y-3 text-[13px]">
        <dt className="text-[#8a8a8a]">ID</dt>
        <dd className="m-0 flex items-center gap-3">
          <code className="font-mono text-[#1a1a1a]">{model.id}</code>
          <button type="button" className="text-[#8a8a8a] hover:text-[#171717]" onClick={onCopy}>
            {copied ? t('settings.models.market_copied') : t('settings.models.market_copy_id')}
          </button>
        </dd>
        <dt className="text-[#8a8a8a]">{t('settings.models.market_type')}</dt>
        <dd className="m-0 text-[#1a1a1a]">{model.category}</dd>
        {model.endpoint ? (
          <>
            <dt className="text-[#8a8a8a]">{t('settings.models.market_endpoint')}</dt>
            <dd className="m-0 text-[#1a1a1a]">{model.endpoint}</dd>
          </>
        ) : null}
        {model.tags.length > 0 ? (
          <>
            <dt className="text-[#8a8a8a]">{t('settings.models.market_tags')}</dt>
            <dd className="m-0 text-[#1a1a1a]">{model.tags.join('、')}</dd>
          </>
        ) : null}
      </dl>
    </section>
  )
}
