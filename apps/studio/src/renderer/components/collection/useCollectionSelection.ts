import { useEffect, useMemo, useRef, useState } from 'react'

/** Current-page selection is never restored into a different query or authority scope. */
export function useCollectionSelection(rows: readonly { id: string }[], viewKey: string) {
  const [state, setState] = useState<{ key: string; ids: string[] }>({ key: viewKey, ids: [] })
  const headerRef = useRef<HTMLButtonElement>(null)
  const visible = useMemo(() => new Set(rows.map((row) => row.id)), [rows])
  const ids = useMemo(
    () => (state.key === viewKey ? state.ids.filter((id) => visible.has(id)) : []),
    [state, viewKey, visible]
  )
  useEffect(() => {
    if (state.key !== viewKey || ids.length !== state.ids.length) setState({ key: viewKey, ids })
  }, [state, viewKey, ids])
  return {
    ids,
    headerRef,
    all: rows.length > 0 && ids.length === rows.length,
    toggle: (id: string, checked: boolean) => {
      if (!visible.has(id)) return
      setState({ key: viewKey, ids: checked ? [...new Set([...ids, id])] : ids.filter((value) => value !== id) })
    },
    togglePage: (checked: boolean) => setState({ key: viewKey, ids: checked ? [...visible] : [] }),
    clear: () => {
      setState({ key: viewKey, ids: [] })
      headerRef.current?.focus()
    }
  }
}

export type CollectionRowSelection = ReturnType<typeof useCollectionSelection>
