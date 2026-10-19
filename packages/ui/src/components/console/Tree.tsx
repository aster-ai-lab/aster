import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react';
import { cn } from '../../lib/cn';

/**
 * Tree — directory / outline control.
 *
 * Visual tokens follow repo root `DESIGN.md`:
 * - Inter 14px / 400–500 (body-sm · button)
 * - Ink on canvas; selected = surface-strong wash (no text-link blue)
 * - Compact row radius `{rounded.sm}` 6px; hit area ≥ 32px
 * - 4px spacing base; depth indent = 16px (`{spacing.base}`)
 */

export type TreeNode = {
  id: string;
  label: string;
  children?: TreeNode[];
  /** Optional leading mark (folder / org glyph). */
  icon?: ReactNode;
  disabled?: boolean;
};

export type TreeProps = {
  nodes: TreeNode[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
  /** Controlled expanded ids. When set with onExpandedChange, Tree is controlled. */
  expandedIds?: string[];
  onExpandedChange?: (ids: string[]) => void;
  /** Uncontrolled initial expansion (defaults to all branch ids). */
  defaultExpandedIds?: string[];
  className?: string;
  renderAction?: (node: TreeNode) => ReactNode;
  'aria-label'?: string;
};

function collectBranchIds(nodes: TreeNode[], into: Set<string>) {
  for (const n of nodes) {
    if (n.children?.length) {
      into.add(n.id);
      collectBranchIds(n.children, into);
    }
  }
}

function flattenVisible(
  nodes: TreeNode[],
  expanded: Set<string>,
  depth = 0,
  out: Array<{ node: TreeNode; depth: number }> = [],
): Array<{ node: TreeNode; depth: number }> {
  for (const node of nodes) {
    out.push({ node, depth });
    if (node.children?.length && expanded.has(node.id)) {
      flattenVisible(node.children, expanded, depth + 1, out);
    }
  }
  return out;
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      className={cn('ui-tree__chevron', open && 'is-open')}
      width="12"
      height="12"
      viewBox="0 0 12 12"
      fill="none"
      aria-hidden
    >
      <path
        d="M4.25 2.5 7.75 6l-3.5 3.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Tree({
  nodes,
  selectedId = null,
  onSelect,
  expandedIds,
  onExpandedChange,
  defaultExpandedIds,
  className = '',
  renderAction,
  'aria-label': ariaLabel = 'Tree',
}: TreeProps) {
  const treeId = useId();
  const listRef = useRef<HTMLUListElement>(null);
  const [focusId, setFocusId] = useState<string | null>(selectedId);

  const defaultSet = useMemo(() => {
    if (defaultExpandedIds) return new Set(defaultExpandedIds);
    const all = new Set<string>();
    collectBranchIds(nodes, all);
    return all;
  }, [nodes, defaultExpandedIds]);

  const [uncontrolled, setUncontrolled] = useState<Set<string>>(defaultSet);
  const controlled = expandedIds !== undefined;
  const expanded = useMemo(
    () => (controlled ? new Set(expandedIds) : uncontrolled),
    [controlled, expandedIds, uncontrolled],
  );

  useEffect(() => {
    if (!controlled) setUncontrolled(defaultSet);
  }, [controlled, defaultSet]);

  useEffect(() => {
    if (selectedId) setFocusId(selectedId);
  }, [selectedId]);

  const setExpanded = useCallback(
    (next: Set<string>) => {
      if (controlled) onExpandedChange?.([...next]);
      else setUncontrolled(next);
    },
    [controlled, onExpandedChange],
  );

  const toggle = useCallback(
    (id: string) => {
      const next = new Set(expanded);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      setExpanded(next);
    },
    [expanded, setExpanded],
  );

  const visible = useMemo(
    () => flattenVisible(nodes, expanded),
    [nodes, expanded],
  );

  const focusIndex = visible.findIndex((v) => v.node.id === focusId);

  function moveFocus(delta: number) {
    if (visible.length === 0) return;
    const start = focusIndex < 0 ? 0 : focusIndex;
    let i = start;
    for (let step = 0; step < visible.length; step += 1) {
      i = (i + delta + visible.length) % visible.length;
      if (!visible[i].node.disabled) {
        setFocusId(visible[i].node.id);
        return;
      }
    }
  }

  function onKeyDown(e: KeyboardEvent<HTMLUListElement>) {
    const current = visible.find((v) => v.node.id === focusId)?.node;
    if (!current && visible[0]) {
      setFocusId(visible[0].node.id);
      return;
    }
    if (!current) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        moveFocus(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        moveFocus(-1);
        break;
      case 'ArrowRight':
        e.preventDefault();
        if (current.children?.length) {
          if (!expanded.has(current.id)) toggle(current.id);
          else moveFocus(1);
        }
        break;
      case 'ArrowLeft':
        e.preventDefault();
        if (current.children?.length && expanded.has(current.id)) {
          toggle(current.id);
        } else {
          // jump to parent if any
          const idx = visible.findIndex((v) => v.node.id === current.id);
          const depth = visible[idx]?.depth ?? 0;
          for (let i = idx - 1; i >= 0; i -= 1) {
            if ((visible[i]?.depth ?? 0) < depth) {
              setFocusId(visible[i].node.id);
              break;
            }
          }
        }
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (!current.disabled) onSelect?.(current.id);
        break;
      case 'Home':
        e.preventDefault();
        setFocusId(visible.find((v) => !v.node.disabled)?.node.id ?? null);
        break;
      case 'End': {
        e.preventDefault();
        const last = [...visible].reverse().find((v) => !v.node.disabled);
        setFocusId(last?.node.id ?? null);
        break;
      }
      default:
        break;
    }
  }

  useEffect(() => {
    if (!focusId || !listRef.current) return;
    const el = listRef.current.querySelector<HTMLElement>(
      `[data-tree-id="${focusId.replace(/"/g, '\\"')}"]`,
    );
    el?.focus({ preventScroll: false });
  }, [focusId]);

  const renderNodes = (list: TreeNode[], depth: number) =>
    list.map((node) => {
      const hasChildren = Boolean(node.children?.length);
      const isOpen = expanded.has(node.id);
      const selected = selectedId === node.id;
      const focused = focusId === node.id;
      return (
        <li
          key={node.id}
          role="treeitem"
          aria-expanded={hasChildren ? isOpen : undefined}
          aria-selected={selected}
          aria-level={depth + 1}
          aria-disabled={node.disabled || undefined}
        >
          <div
            className={cn(
              'ui-tree__row',
              selected && 'is-selected',
              focused && 'is-focused',
              node.disabled && 'is-disabled',
            )}
            style={{ paddingLeft: `calc(0.5rem + ${depth} * 1rem)` }}
          >
            {hasChildren ? (
              <button
                type="button"
                className="ui-tree__twist"
                tabIndex={-1}
                aria-label={isOpen ? 'Collapse' : 'Expand'}
                disabled={node.disabled}
                onClick={(e) => {
                  e.stopPropagation();
                  toggle(node.id);
                  setFocusId(node.id);
                }}
              >
                <Chevron open={isOpen} />
              </button>
            ) : (
              <span className="ui-tree__twist ui-tree__twist--spacer" />
            )}
            <button
              type="button"
              className="ui-tree__label"
              data-tree-id={node.id}
              tabIndex={focused ? 0 : -1}
              disabled={node.disabled}
              onClick={() => {
                setFocusId(node.id);
                if (!node.disabled) onSelect?.(node.id);
              }}
            >
              {node.icon ? (
                <span className="ui-tree__icon" aria-hidden>
                  {node.icon}
                </span>
              ) : null}
              <span className="ui-tree__text">{node.label}</span>
            </button>
            {renderAction ? (
              <span className="ui-tree__action">{renderAction(node)}</span>
            ) : null}
          </div>
          {hasChildren && isOpen ? (
            <ul role="group">{renderNodes(node.children!, depth + 1)}</ul>
          ) : null}
        </li>
      );
    });

  return (
    <ul
      ref={listRef}
      id={treeId}
      className={cn('ui-tree', className)}
      role="tree"
      aria-label={ariaLabel}
      tabIndex={-1}
      onKeyDown={onKeyDown}
    >
      {renderNodes(nodes, 0)}
    </ul>
  );
}

/** Build a TreeNode forest from flat parent_id rows (null parent = root). */
export function buildTreeNodes<T extends { id: string; name: string; parent_id: string | null }>(
  rows: T[],
  options?: { icon?: ReactNode; sort?: (a: T, b: T) => number },
): TreeNode[] {
  const byParent = new Map<string | null, T[]>();
  for (const row of rows) {
    const key = row.parent_id;
    const list = byParent.get(key) ?? [];
    list.push(row);
    byParent.set(key, list);
  }
  const sortFn =
    options?.sort ?? ((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }));
  for (const list of byParent.values()) {
    list.sort(sortFn);
  }
  const walk = (parentId: string | null): TreeNode[] =>
    (byParent.get(parentId) ?? []).map((row) => ({
      id: row.id,
      label: row.name,
      icon: options?.icon,
      children: walk(row.id),
    }));
  return walk(null);
}
