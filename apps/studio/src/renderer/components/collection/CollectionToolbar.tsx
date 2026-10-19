import type { ReactNode } from 'react'

/** Collection controls stay next to the table, independently of the page heading. The result summary shares the control row. */
export function CollectionToolbar({
  label,
  search,
  filters,
  views,
  actions,
  summary
}: {
  label: string
  search?: ReactNode
  filters?: ReactNode
  views?: ReactNode
  actions?: ReactNode
  summary?: ReactNode
}) {
  return (
    <section aria-label={label} data-ui="collection.toolbar" className="mb-3">
      <div className="flex min-h-10 flex-wrap items-center gap-x-4 gap-y-2">
        <div className="flex max-w-full min-w-0 flex-wrap items-center gap-2">
          {search}
          {filters}
        </div>
        <div className="ml-auto flex max-w-full min-w-0 flex-wrap items-center justify-end gap-2">
          {views ? <div className="max-w-full overflow-x-auto">{views}</div> : null}
          {summary ? <div className="flex shrink-0 items-center gap-3">{summary}</div> : null}
          {actions}
        </div>
      </div>
    </section>
  )
}
