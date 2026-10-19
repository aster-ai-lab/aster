import { Button, TreeView, flattenTree, type TreeNodeAdapter } from '@a3s-lab/ui'
import { ChevronDown, ChevronRight, Play, RefreshCw } from 'lucide-react'
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import type { AixTreeProvider } from '@renderer/services/plugins'
import type { AixTreeNode } from '@shared/types/aixTree'

export function AixTreeView({ provider, title }: { provider: AixTreeProvider; title: string }) {
  const { t } = useTranslation()
  const snapshot = useSyncExternalStore(provider.subscribe, provider.getSnapshot, provider.getSnapshot)
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set())
  const [selected, setSelected] = useState<string | null>(null)
  const [focused, setFocused] = useState<string | null>(null)
  const tree = useRef<HTMLDivElement>(null)
  const { roots, adapter } = useMemo(() => {
    const children = new Map<string, AixTreeNode[]>()
    for (const node of snapshot.nodes) {
      if (node.parent === null) continue
      const siblings = children.get(node.parent) ?? []
      siblings.push(node)
      children.set(node.parent, siblings)
    }
    return {
      roots: snapshot.nodes.filter((node) => node.parent === null),
      adapter: {
        getId: (node: AixTreeNode) => node.id,
        getChildren: (node: AixTreeNode) => children.get(node.id),
        canHaveChildren: (node: AixTreeNode) => node.hasChildren
      } satisfies TreeNodeAdapter<AixTreeNode>
    }
  }, [snapshot.nodes])
  const visible = useMemo(() => flattenTree(roots, adapter, expanded), [roots, adapter, expanded])
  const focusedId = visible.some((item) => item.id === focused) ? focused : visible[0]?.id
  useEffect(() => {
    const ids = new Set(snapshot.nodes.map((node) => node.id))
    setExpanded((previous) => new Set([...previous].filter((id) => ids.has(id))))
    setSelected((previous) => (previous && ids.has(previous) ? previous : null))
    setFocused((previous) => (previous && ids.has(previous) ? previous : null))
  }, [snapshot.nodes])
  const focus = (id: string | null) => {
    const index = visible.findIndex((item) => item.id === id)
    if (index < 0) return
    setFocused(id)
    tree.current?.querySelectorAll<HTMLElement>('[role="treeitem"]')[index]?.focus()
  }
  if (snapshot.status === 'failed' || snapshot.status === 'closed') return <div role="alert">{t('common.error')}</div>
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex justify-end px-1">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t('common.refresh')}
          disabled={snapshot.busy || snapshot.status !== 'ready'}
          onClick={() => provider.request('refresh', null)}>
          <RefreshCw size={14} />
        </Button>
      </div>
      {snapshot.busy && <div role="status">{t('common.loading')}</div>}
      <div ref={tree} role="tree" aria-label={title} aria-busy={snapshot.busy} className="min-h-0 flex-1 overflow-auto">
        <TreeView
          data={roots}
          adapter={adapter}
          expandedIds={expanded}
          onExpandedChange={(next) => {
            const id = [...next, ...expanded].find((key) => next.has(key) !== expanded.has(key))
            if (id && provider.request(next.has(id) ? 'expand' : 'collapse', id)) setExpanded(next)
          }}
          selectedId={selected}
          emptyState={
            !snapshot.busy ? <span className="text-xs text-muted-foreground">{t('common.no_results')}</span> : undefined
          }
          onSelectedChange={(id) => {
            if (id && provider.request('select', id)) setSelected(id)
          }}
          renderRow={({ node, depth, isExpanded, isSelected, toggleExpanded, selectNode }) => (
            <div
              role="treeitem"
              aria-level={depth + 1}
              aria-expanded={node.hasChildren ? isExpanded : undefined}
              aria-selected={isSelected}
              aria-label={node.label}
              tabIndex={focusedId === node.id ? 0 : -1}
              onFocus={() => setFocused(node.id)}
              onKeyDown={(event) => {
                if (event.target !== event.currentTarget) return
                const index = visible.findIndex((item) => item.id === node.id)
                switch (event.key) {
                  case 'ArrowDown':
                    focus(visible[index + 1]?.id ?? null)
                    break
                  case 'ArrowUp':
                    focus(visible[index - 1]?.id ?? null)
                    break
                  case 'Home':
                    focus(visible[0]?.id ?? null)
                    break
                  case 'End':
                    focus(visible.at(-1)?.id ?? null)
                    break
                  case 'ArrowRight':
                    if (node.hasChildren && !isExpanded) toggleExpanded()
                    else if (isExpanded) focus(visible[index + 1]?.id ?? null)
                    break
                  case 'ArrowLeft':
                    if (isExpanded) toggleExpanded()
                    else focus(node.parent)
                    break
                  case 'Enter':
                    if (node.command) provider.request('invoke', node.id)
                    else selectNode()
                    break
                  case ' ':
                    selectNode()
                    break
                  default:
                    return
                }
                event.preventDefault()
              }}
              className="flex min-h-8 items-center gap-1 rounded-sm px-1 focus-visible:ring-2 focus-visible:ring-ring aria-selected:bg-accent"
              style={{ paddingInlineStart: depth * 16 + 4 }}>
              <Button
                variant="ghost"
                size="icon-sm"
                disabled={!node.hasChildren || snapshot.busy}
                tabIndex={-1}
                aria-label={`${t(isExpanded ? 'common.collapse' : 'common.expand')} ${node.label}`}
                onClick={toggleExpanded}>
                {node.hasChildren && (isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />)}
              </Button>
              <Button
                variant="ghost"
                disabled={snapshot.busy}
                tabIndex={-1}
                className="min-w-0 flex-1 justify-start"
                onClick={selectNode}>
                <span className="truncate">{node.label}</span>
                {node.description && <span className="truncate text-xs text-muted-foreground">{node.description}</span>}
              </Button>
              {node.command && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  disabled={snapshot.busy}
                  tabIndex={-1}
                  aria-label={node.command}
                  onClick={() => provider.request('invoke', node.id)}>
                  <Play size={12} />
                </Button>
              )}
            </div>
          )}
        />
      </div>
    </div>
  )
}
