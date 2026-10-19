import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { EmptyState } from './EmptyState';
import { Input } from '../primitives/input';
import { Skeleton } from '../primitives/skeleton';
import { Tree, type TreeNode, type TreeProps } from './Tree';

export type TreeSearchPanelProps = {
  nodes: TreeNode[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
  query: string;
  onQueryChange: (query: string) => void;
  label?: string;
  searchPlaceholder?: string;
  emptyText?: string;
  /** Localized label for the clear-filters control when search has no hits. */
  clearFiltersLabel?: string;
  loading?: boolean;
  className?: string;
  treeProps?: Omit<
    TreeProps,
    'nodes' | 'selectedId' | 'onSelect' | 'aria-label'
  >;
  empty?: ReactNode;
};

function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  const q = query.trim().toLowerCase();
  if (!q) return nodes;
  const walk = (list: TreeNode[]): TreeNode[] => {
    const out: TreeNode[] = [];
    for (const node of list) {
      const kids = node.children ? walk(node.children) : [];
      if (node.label.toLowerCase().includes(q) || kids.length > 0) {
        out.push({ ...node, children: kids });
      }
    }
    return out;
  };
  return walk(nodes);
}

function SearchGlyph() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16.5 16.5 21 21" />
    </svg>
  );
}

/** Search field + Tree + loading/empty — directory left pane. */
export function TreeSearchPanel({
  nodes,
  selectedId,
  onSelect,
  query,
  onQueryChange,
  label = 'Tree',
  searchPlaceholder = 'Search…',
  emptyText = 'No matching items',
  clearFiltersLabel,
  loading = false,
  className = '',
  treeProps,
  empty,
}: TreeSearchPanelProps) {
  const filtered = filterTree(nodes, query);

  return (
    <div className={cn('ui-tree-search-panel', className)}>
      <div className="ui-tree-search-panel__toolbar">
        {label ? (
          <p className="ui-tree-search-panel__label">{label}</p>
        ) : null}
        <div className="ui-tree-search-panel__search">
          <span className="ui-tree-search-panel__search-icon" aria-hidden>
            <SearchGlyph />
          </span>
          <Input
            size="sm"
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder={searchPlaceholder}
            aria-label={searchPlaceholder}
            className="!pl-8 !pr-7"
            allowClear
          />
        </div>
      </div>
      {loading ? (
        <div className="ui-tree-search-panel__skeleton" aria-busy>
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="h-4 w-4/5" />
        </div>
      ) : filtered.length === 0 ? (
        (empty ?? (
          <EmptyState
            className="!border-0 !bg-transparent !px-2 !py-6"
            variant={query.trim() ? 'no-match' : 'no-data'}
            title={emptyText}
            clearFiltersLabel={clearFiltersLabel}
            onClearFilters={
              query.trim() ? () => onQueryChange('') : undefined
            }
          />
        ))
      ) : (
        <Tree
          {...treeProps}
          nodes={filtered}
          selectedId={selectedId}
          onSelect={onSelect}
          aria-label={label}
        />
      )}
    </div>
  );
}
