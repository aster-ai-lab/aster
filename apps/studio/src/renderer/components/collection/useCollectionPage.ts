import { useEffect, useState } from 'react'

/** For complete bounded arrays only. Remote datasets must page at their source. */
export function useCollectionPage<T>(
  items: readonly T[] | undefined,
  scope: string,
  initialPageSize = 20,
  viewKey = scope
) {
  const [state, setState] = useState({ scope, viewKey, page: 1, pageSize: initialPageSize })
  const current =
    state.scope !== scope
      ? { scope, viewKey, page: 1, pageSize: initialPageSize }
      : state.viewKey !== viewKey
        ? { ...state, viewKey, page: 1 }
        : state
  const pageCount = Math.max(1, Math.ceil((items?.length ?? 0) / current.pageSize))
  const page = Math.min(current.page, pageCount)
  useEffect(() => {
    if (state.scope !== scope) setState({ scope, viewKey, page: 1, pageSize: initialPageSize })
    else if (state.viewKey !== viewKey) setState({ ...state, viewKey, page: 1 })
    else if (items && state.page > pageCount) setState({ ...state, page: pageCount })
  }, [scope, viewKey, state, pageCount, items, initialPageSize])
  return {
    page,
    pageSize: current.pageSize,
    total: items?.length ?? 0,
    visible: items?.slice((page - 1) * current.pageSize, page * current.pageSize) ?? [],
    onPageChange: (next: number) => setState({ ...current, page: Math.max(1, Math.min(next, pageCount)) }),
    onPageSizeChange: (pageSize: number) => setState({ scope, viewKey, page: 1, pageSize })
  }
}
