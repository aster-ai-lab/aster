import { Button, SearchInput } from '@a3s-lab/ui'
import { useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { readSupplierRelations, type SupplierRelationsQuery } from '@renderer/data/platformSuppliers'

const viewLabels = {
  models: 'platform.supplierRelations.models',
  dependencies: 'platform.supplierRelations.dependencies'
} as const
const stateLabels = {
  draft: 'platform.supplierRelations.state.draft',
  published: 'platform.supplierRelations.state.published',
  suspended: 'platform.supplierRelations.state.suspended'
} as const

export function SupplierRelations(props: SupplierRelationsProps) {
  return <SupplierRelationsView key={`${props.scope}/${props.supplierId}`} {...props} />
}

type SupplierRelationsProps = {
  supplierId: string
  scope: string
  authorized: boolean
  initialKind?: SupplierRelationsQuery['kind']
  reviewKey?: string
  onReview?: (review: { revision: number; total: number } | null) => void
  onUnavailable?: () => void
}

function SupplierRelationsView({
  supplierId,
  scope,
  authorized,
  initialKind = 'models',
  reviewKey = '',
  onReview,
  onUnavailable
}: SupplierRelationsProps) {
  const { t } = useTranslation()
  const heading = useId()
  const [query, setQuery] = useState<SupplierRelationsQuery>({
    kind: initialKind,
    page: 1,
    limit: 20,
    search: '',
    model: ''
  })
  const [input, setInput] = useState('')
  const [composing, setComposing] = useState(false)
  const generation = JSON.stringify([`studio/platform/${scope}/suppliers/${supplierId}/relations`, reviewKey, query])
  const { data, error, isLoading, isValidating, mutate } = useSWR(
    authorized ? generation : null,
    () => readSupplierRelations(supplierId, query),
    { keepPreviousData: false, shouldRetryOnError: false }
  )
  const busy = isLoading || isValidating
  const reported = useRef<unknown>(null)
  useEffect(() => {
    if (composing || input === query.search) return
    const timer = setTimeout(() => setQuery((previous) => ({ ...previous, search: input, page: 1 })), 300)
    return () => clearTimeout(timer)
  }, [input, composing, query.search])
  useEffect(() => {
    if (data && !error && data.page !== query.page) setQuery((previous) => ({ ...previous, page: data.page }))
  }, [data, error, query.page])
  useEffect(() => {
    onReview?.(authorized && !error && !busy && data ? { revision: data.revision, total: data.dependencyTotal } : null)
  }, [authorized, data, error, busy, onReview])
  useEffect(() => {
    if (
      error &&
      reported.current !== error &&
      error instanceof MemberOrgError &&
      [401, 404].includes(error.status ?? 0)
    ) {
      reported.current = error
      onUnavailable?.()
    }
  }, [error, onUnavailable])
  if (!authorized) return null
  const switchKind = (kind: SupplierRelationsQuery['kind'], model = '') => {
    setInput('')
    setQuery((previous) => ({ ...previous, kind, model, search: '', page: 1 }))
  }
  const visible = error ? undefined : data
  return (
    <section
      data-ui="platform.suppliers.relations"
      aria-labelledby={heading}
      aria-busy={busy}
      className="mt-4 grid gap-3 text-[13px] text-[#3f3f3f] [&_:focus-visible]:outline-2 [&_:focus-visible]:outline-offset-2 [&_:focus-visible]:outline-[#171717]">
      <h3 id={heading} className="m-0 text-[14px] font-medium text-[#171717]">
        {t('platform.supplierRelations.title')}
      </h3>
      <div role="group" aria-label={t('platform.supplierRelations.views')} className="flex flex-wrap gap-2">
        {(['models', 'dependencies'] as const).map((kind) => (
          <Button
            key={kind}
            variant="outline"
            size="sm"
            aria-pressed={query.kind === kind}
            onClick={() => switchKind(kind)}>
            {t(viewLabels[kind])}
          </Button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput
          value={input}
          maxLength={128}
          aria-label={t('platform.supplierRelations.search')}
          placeholder={t('platform.supplierRelations.search')}
          clearLabel={t('platform.searchClear')}
          onChange={(event) => setInput(event.target.value)}
          onClear={() => {
            setInput('')
            setQuery((previous) => ({ ...previous, search: '', page: 1 }))
          }}
          onCompositionStart={() => setComposing(true)}
          onCompositionEnd={(event) => {
            setComposing(false)
            setInput(event.currentTarget.value)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !composing && !event.nativeEvent.isComposing)
              setQuery((previous) => ({ ...previous, search: input, page: 1 }))
          }}
        />
        <Button variant="outline" size="sm" disabled={busy} onClick={() => void mutate()}>
          {t('common.refresh')}
        </Button>
        {query.model ? (
          <Button variant="outline" size="sm" onClick={() => switchKind('dependencies')}>
            {t('platform.supplierRelations.clearModel', { model: query.model })}
          </Button>
        ) : null}
      </div>
      {error ? (
        <div role="alert">
          <p>{t('platform.supplierRelations.failed')}</p>
          <Button variant="outline" size="sm" disabled={busy} onClick={() => void mutate()}>
            {t('common.retry')}
          </Button>
        </div>
      ) : null}
      {!error && !visible ? <p role="status">{t('platform.suppliers.modelsLoading')}</p> : null}
      {visible ? (
        <>
          <p role="status" className="m-0">
            {t('platform.supplierRelations.dependencySummary', { count: visible.dependencyTotal })}
          </p>
          {!visible.total ? (
            <p>
              {t(
                query.search || query.model
                  ? 'platform.supplierRelations.noMatches'
                  : query.kind === 'models'
                    ? 'platform.suppliers.noModels'
                    : 'platform.supplierRelations.noDependencies'
              )}
            </p>
          ) : (
            <ul className="m-0 list-none p-0">
              {visible.items.map((item) => (
                <li key={item.id} className="flex flex-wrap items-center gap-3 border-b border-[#efefef] py-2">
                  <span className="min-w-0 flex-1 break-all text-[#171717]">
                    {item.name}
                    {query.kind === 'dependencies' && item.name !== item.id ? (
                      <span className="ml-2 text-[#3f3f3f]">{item.id}</span>
                    ) : null}
                  </span>
                  {query.kind === 'models' ? (
                    <>
                      <span>{t('platform.supplierRelations.count', { count: item.dependencyCount })}</span>
                      {item.dependencyCount ? (
                        <Button variant="outline" size="sm" onClick={() => switchKind('dependencies', item.id)}>
                          {t('platform.supplierRelations.viewDependencies', { model: item.id })}
                        </Button>
                      ) : null}
                    </>
                  ) : (
                    <>
                      <span>{item.state ? t(stateLabels[item.state]) : '—'}</span>
                      <span className="min-w-0 break-all">{item.upstreamModels.join(', ')}</span>
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}
          <CollectionPagination
            page={visible.page}
            pageSize={visible.limit}
            total={visible.total}
            busy={busy}
            onPageChange={(page) => setQuery((previous) => ({ ...previous, page }))}
            onPageSizeChange={(limit) => setQuery((previous) => ({ ...previous, limit, page: 1 }))}
          />
        </>
      ) : null}
    </section>
  )
}
