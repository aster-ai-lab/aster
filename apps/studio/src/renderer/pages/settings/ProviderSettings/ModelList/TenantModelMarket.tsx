import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SearchInput } from '@a3s-lab/ui'
import { loadTenantModelMarket, type TenantMarketModel } from '@renderer/data/tenantModelMarket'
import { cn } from '@renderer/utils/style'

const ALL = '__all__'

/** Tenant model market. The models are already provided; this view only searches and filters them. */
export default function TenantModelMarket() {
  const { t } = useTranslation()
  const [models, setModels] = useState<TenantMarketModel[] | null>(null)
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState(ALL)
  useEffect(() => {
    let live = true
    void loadTenantModelMarket().then((next) => {
      if (live) setModels(next)
    })
    return () => {
      live = false
    }
  }, [])
  const categories = useMemo(() => marketCategories(models ?? []), [models])
  const visible = useMemo(() => filterMarketModels(models ?? [], query, category), [models, query, category])

  return (
    <section data-ui="settings.model-market" className="flex flex-col gap-4" aria-labelledby="setting-provider-model-market">
      <h2 id="setting-provider-model-market" className="m-0 text-[18px] leading-tight font-semibold text-[#1a1a1a]">
        {t('settings.models.market_title')}
      </h2>
      <SearchInput
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onClear={() => setQuery('')}
        clearLabel={t('common.clear')}
        placeholder={t('settings.models.market_search')}
        containerClassName="h-[30px] w-full border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
      />
      <div className="flex flex-wrap items-center gap-0.5" role="radiogroup" aria-label={t('settings.models.market_title')}>
        <MarketChip label={t('settings.models.market_all')} selected={category === ALL} onSelect={() => setCategory(ALL)} />
        {categories.map((item) => (
          <MarketChip key={item} label={item} selected={category === item} onSelect={() => setCategory(item)} />
        ))}
      </div>
      {models === null ? null : visible.length === 0 ? (
        <p className="m-0 py-10 text-center text-[13px] text-[#8a8a8a]">{t('settings.models.market_empty')}</p>
      ) : (
        <ul className="m-0 grid list-none grid-cols-1 gap-3 p-0 sm:grid-cols-2">
          {visible.map((model) => (
            <li key={model.id} data-ui="settings.model-market.card" className="rounded-[14px] border border-[#ececec] bg-white p-4">
              <div className="truncate text-[14px] font-medium text-[#1a1a1a]" title={model.name}>
                {model.name}
              </div>
              {model.summary ? <p className="m-0 mt-1 text-[13px] leading-5 text-[#8a8a8a]">{model.summary}</p> : null}
              <div className="mt-3 flex flex-wrap gap-2 text-[12px] text-[#6f6f6f]">
                <span>{model.category}</span>
                {model.tags.map((tag) => (
                  <span key={tag}>{tag}</span>
                ))}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function MarketChip({ label, selected, onSelect }: { label: string; selected: boolean; onSelect: () => void }) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      className={cn(
        'h-9 shrink-0 rounded-[8px] px-2.5 text-[13px] whitespace-nowrap outline-none',
        'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
      )}>
      {label}
    </button>
  )
}

export function marketCategories(models: readonly TenantMarketModel[]): string[] {
  const categories: string[] = []
  for (const model of models) {
    if (!categories.includes(model.category)) categories.push(model.category)
  }
  return categories
}

export function filterMarketModels(
  models: readonly TenantMarketModel[],
  query: string,
  category: string
): TenantMarketModel[] {
  const needle = query.trim().toLowerCase()
  return models.filter((model) => {
    if (category !== ALL && model.category !== category) return false
    if (!needle) return true
    return [model.name, model.summary, model.id, model.category, ...model.tags].some((field) =>
      field.toLowerCase().includes(needle)
    )
  })
}
