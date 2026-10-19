import { ConsolePagination, SearchSelect } from '@a3s-lab/ui'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'

export const COLLECTION_PAGE_SIZES = [10, 20, 50, 100] as const
const PAGE_SIZE_OPTIONS = COLLECTION_PAGE_SIZES.map((size) => ({ value: String(size), label: String(size) }))

/** Shared by complete, bounded collections and server offset pages. Never use a partial feed's count as its total. */
export function CollectionPagination({
  page,
  pageSize,
  total,
  onPageChange,
  onPageSizeChange,
  busy = false
}: {
  page: number
  pageSize: number
  total: number
  onPageChange: (page: number) => void
  onPageSizeChange?: (pageSize: number) => void
  busy?: boolean
}) {
  const { t } = useTranslation()
  const sizeId = useId()
  return (
    <div
      data-ui="collection.pagination"
      className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-3 border-t border-[#efefef] pt-3">
      {onPageSizeChange ? (
        <div className="flex shrink-0 items-center gap-2 text-[12px] whitespace-nowrap text-[#3f3f3f]">
          <span aria-hidden>{t('collection.rowsPerPage')}</span>
          <SearchSelect
            id={sizeId}
            size="sm"
            searchable={false}
            allowClear={false}
            disabled={busy}
            aria-label={t('collection.rowsPerPage')}
            value={String(pageSize)}
            options={PAGE_SIZE_OPTIONS}
            onChange={(next) => onPageSizeChange(Number(next))}
            className="w-[76px] min-w-0 [&_.ui-search-select\_\_trigger]:min-w-0"
          />
        </div>
      ) : null}
      <ConsolePagination
        className="min-w-0 flex-[1_1_300px] justify-between"
        page={page}
        pageSize={pageSize}
        total={total}
        busy={busy}
        onPageChange={onPageChange}
        labels={{
          navigation: t('collection.pagination'),
          pages: t('collection.pageNumbers'),
          previous: t('common.previous'),
          next: t('common.next'),
          page: (value) => t('collection.page', { page: value }),
          range: (start, end, count) =>
            count === 0 ? t('collection.emptyRange') : t('collection.range', { start, end, total: count })
        }}
      />
    </div>
  )
}
