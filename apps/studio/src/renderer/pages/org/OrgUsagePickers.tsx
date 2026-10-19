import { Popover, PopoverContent, PopoverTrigger } from '@a3s-lab/ui'
import { Check, ChevronDown, ChevronRight, Search } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import type { OrgDepartment, OrgMemberRow } from './orgApi'

type DeptNode = { id: string; name: string; children: DeptNode[] }

type VisibleDept = { node: DeptNode; depth: number; hasChildren: boolean; open: boolean }

const popoverClassName =
  'w-64 rounded-[8px] border border-[#e5e5e5] bg-white p-0 shadow-[0_12px_32px_rgba(0,0,0,0.12)] dark:border-[#3a3a3a] dark:bg-[#1c1c1c]'

const triggerClassName =
  'inline-flex h-9 w-52 min-w-0 items-center gap-2 rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-left text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10 dark:bg-[#2c2c2c] dark:text-[#f5f5f5]'

function memberPhoto(avatar: string | undefined): string {
  const value = avatar?.trim() ?? ''
  return value.startsWith('data:image/') && value.includes(';base64,') ? value : ''
}

function MemberFace({ name, avatar }: { name: string; avatar?: string }) {
  const photo = memberPhoto(avatar)
  const initial = [...name.trim()][0] ?? ''
  return (
    <span
      data-ui="org.usage.avatar"
      role="img"
      aria-label={name.trim() || undefined}
      className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-[8px] bg-[#f2f3f5] text-[12px] font-medium text-[#3f3f3f]">
      {photo ? <img src={photo} alt="" draggable={false} className="size-full object-cover" /> : initial}
    </span>
  )
}

function buildDepartmentTree(rows: readonly OrgDepartment[]): DeptNode[] {
  const children = new Map<string, OrgDepartment[]>()
  for (const row of rows) {
    if (!row.parent_id) continue
    const list = children.get(row.parent_id) ?? []
    list.push(row)
    children.set(row.parent_id, list)
  }
  const byName = (left: OrgDepartment, right: OrgDepartment) =>
    left.sort_order - right.sort_order || left.name.localeCompare(right.name, 'zh')
  for (const list of children.values()) list.sort(byName)
  const toNode = (row: OrgDepartment): DeptNode => ({
    id: row.id,
    name: row.name,
    children: (children.get(row.id) ?? []).map(toNode)
  })
  return rows
    .filter((row) => !row.parent_id)
    .sort(byName)
    .map(toNode)
}

function nameHit(name: string, query: string): boolean {
  return name.toLowerCase().includes(query)
}

function subtreeHit(node: DeptNode, query: string): boolean {
  if (nameHit(node.name, query)) return true
  return node.children.some((child) => subtreeHit(child, query))
}

function visibleDepartments(
  nodes: readonly DeptNode[],
  query: string,
  expanded: ReadonlySet<string>,
  parentMatched: boolean,
  depth: number,
  into: VisibleDept[]
): void {
  for (const node of nodes) {
    const selfHit = query.length === 0 || nameHit(node.name, query)
    const childHit = query.length > 0 && node.children.some((child) => subtreeHit(child, query))
    if (query.length > 0 && !selfHit && !childHit && !parentMatched) continue
    const hasChildren = node.children.length > 0
    const open = hasChildren && (query.length > 0 ? selfHit || childHit || parentMatched : expanded.has(node.id))
    into.push({ node, depth, hasChildren, open })
    if (open) {
      visibleDepartments(
        node.children,
        query,
        expanded,
        query.length > 0 && (selfHit || parentMatched),
        depth + 1,
        into
      )
    }
  }
}

function ancestorIds(rows: readonly OrgDepartment[], id: string): string[] {
  const byId = new Map(rows.map((row) => [row.id, row]))
  const ids: string[] = []
  const seen = new Set<string>()
  let current = byId.get(id)
  while (current?.parent_id && !seen.has(current.parent_id)) {
    seen.add(current.parent_id)
    ids.push(current.parent_id)
    current = byId.get(current.parent_id)
  }
  return ids
}

function UsagePicker({
  label,
  open,
  onOpenChange,
  trigger,
  searchLabel,
  query,
  onQueryChange,
  onSearchKeyDown,
  children
}: {
  label: string
  open: boolean
  onOpenChange: (open: boolean) => void
  trigger: ReactNode
  searchLabel: string
  query: string
  onQueryChange: (value: string) => void
  onSearchKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void
  children: ReactNode
}) {
  const listId = useId()
  const searchRef = useRef<HTMLInputElement>(null)
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          role="combobox"
          aria-label={label}
          aria-expanded={open}
          aria-controls={listId}
          aria-autocomplete="list"
          data-ui="org.usage.subject"
          className={triggerClassName}>
          {trigger}
          <ChevronDown
            aria-hidden
            strokeWidth={1.7}
            className={cn('ml-auto size-4 shrink-0 text-[#8a8a8a] transition-transform', open && 'rotate-180')}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className={popoverClassName}
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          searchRef.current?.focus()
        }}>
        <div className="p-1.5">
          <label className="flex h-[30px] items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5 dark:bg-[#2c2c2c]">
            <Search aria-hidden strokeWidth={1.7} className="size-4 shrink-0 text-[#8a8a8a]" />
            <input
              ref={searchRef}
              value={query}
              aria-label={searchLabel}
              placeholder={searchLabel}
              onChange={(event) => onQueryChange(event.target.value)}
              onKeyDown={onSearchKeyDown}
              className="h-[30px] min-w-0 flex-1 border-0 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] dark:text-[#f5f5f5]"
            />
          </label>
        </div>
        <div id={listId} className="max-h-60 overflow-y-auto px-1.5 pb-1.5">
          {children}
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** 可搜索的部门树。展开不改变筛选，点名称才选中。 */
export function DepartmentUsagePicker({
  departments,
  value,
  onChange
}: {
  departments: readonly OrgDepartment[]
  value: string
  onChange: (id: string) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [activeId, setActiveId] = useState(value)
  const focusTree = useRef(false)
  const itemRefs = useRef(new Map<string, HTMLDivElement>())
  const tree = useMemo(() => buildDepartmentTree(departments), [departments])
  const needle = query.trim().toLowerCase()
  const visible = useMemo(() => {
    const into: VisibleDept[] = []
    visibleDepartments(tree, needle, expanded, false, 0, into)
    return into
  }, [expanded, needle, tree])
  const selected = departments.find((row) => row.id === value)

  useEffect(() => {
    if (visible.some((item) => item.node.id === activeId)) return
    setActiveId(visible[0]?.node.id ?? '')
  }, [activeId, visible])

  useEffect(() => {
    if (!focusTree.current) return
    focusTree.current = false
    itemRefs.current.get(activeId)?.focus()
  }, [activeId, visible])

  const revealSelection = () => {
    setExpanded((current) => {
      const next = new Set(current)
      for (const id of ancestorIds(departments, value)) next.add(id)
      if (departments.some((row) => row.parent_id === value)) next.add(value)
      return next
    })
    setActiveId(value)
  }

  const moveActive = (id: string) => {
    focusTree.current = true
    setActiveId(id)
  }

  const toggle = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const choose = (id: string) => {
    onChange(id)
    setOpen(false)
    setQuery('')
  }

  const onItemKeyDown = (event: KeyboardEvent<HTMLDivElement>, index: number) => {
    const item = visible[index]
    if (!item) return
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      const next = visible[index + 1]
      if (next) moveActive(next.node.id)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      const previous = visible[index - 1]
      if (previous) moveActive(previous.node.id)
      return
    }
    if (event.key === 'ArrowRight' && item.hasChildren) {
      event.preventDefault()
      if (!item.open) toggle(item.node.id)
      else if (visible[index + 1]?.depth === item.depth + 1) moveActive(visible[index + 1].node.id)
      return
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      if (item.open) toggle(item.node.id)
      else {
        const parent = [...visible.slice(0, index)].reverse().find((row) => row.depth < item.depth)
        if (parent) moveActive(parent.node.id)
      }
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      choose(item.node.id)
    }
  }

  return (
    <UsagePicker
      label={t('org.usage.pickDepartment')}
      open={open}
      searchLabel={t('org.usage.searchDepartment')}
      query={query}
      onQueryChange={setQuery}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          setQuery('')
          revealSelection()
        }
      }}
      onSearchKeyDown={(event) => {
        if (event.key === 'ArrowDown' && visible[0]) {
          event.preventDefault()
          moveActive(visible[0].node.id)
        }
      }}
      trigger={<span className="min-w-0 flex-1 truncate">{selected?.name ?? ''}</span>}>
      {visible.length === 0 ? (
        <p className="m-0 py-6 text-center text-[13px] text-[#8a8a8a]">{t('org.usage.noDepartment')}</p>
      ) : (
        <div
          role="tree"
          aria-label={t('org.usage.pickDepartment')}
          className="grid gap-0.5"
          data-ui="org.usage.department-tree">
          {visible.map((item, index) => {
            const selectedRow = item.node.id === value
            return (
              <div
                key={item.node.id}
                ref={(node) => {
                  if (node) itemRefs.current.set(item.node.id, node)
                  else itemRefs.current.delete(item.node.id)
                }}
                role="treeitem"
                tabIndex={item.node.id === activeId ? 0 : -1}
                aria-label={item.node.name}
                aria-selected={selectedRow}
                aria-expanded={item.hasChildren ? item.open : undefined}
                style={{ paddingLeft: 10 + item.depth * 16 }}
                className={cn(
                  'flex h-9 cursor-pointer items-center gap-1 rounded-[8px] pr-2.5 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                  selectedRow
                    ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                    : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                )}
                onClick={() => choose(item.node.id)}
                onKeyDown={(event) => onItemKeyDown(event, index)}>
                {item.hasChildren ? (
                  <button
                    type="button"
                    tabIndex={-1}
                    aria-label={t(item.open ? 'org.usage.collapse' : 'org.usage.expand', { name: item.node.name })}
                    className="grid size-6 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] hover:bg-[#f2f3f5]"
                    onClick={(event) => {
                      event.stopPropagation()
                      toggle(item.node.id)
                    }}>
                    <ChevronRight
                      aria-hidden
                      strokeWidth={1.7}
                      className={cn('size-4 transition-transform', item.open && 'rotate-90')}
                    />
                  </button>
                ) : (
                  <span aria-hidden className="size-6 shrink-0" />
                )}
                <span className="min-w-0 flex-1 truncate">{item.node.name}</span>
                {selectedRow ? <Check aria-hidden strokeWidth={1.7} className="size-4 shrink-0" /> : null}
              </div>
            )
          })}
        </div>
      )}
    </UsagePicker>
  )
}

function memberMatches(row: OrgMemberRow, query: string): boolean {
  if (!query) return true
  return [row.name, row.phone ?? '', row.email].some((value) => value.toLowerCase().includes(query))
}

/** 可搜索的成员列表。每一行带成员头像。 */
export function MemberUsagePicker({
  members,
  value,
  onChange
}: {
  members: readonly OrgMemberRow[]
  value: string
  onChange: (id: string) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [activeId, setActiveId] = useState(value)
  const focusList = useRef(false)
  const itemRefs = useRef(new Map<string, HTMLButtonElement>())
  const sorted = useMemo(() => [...members].sort((left, right) => left.name.localeCompare(right.name, 'zh')), [members])
  const needle = query.trim().toLowerCase()
  const visible = useMemo(() => sorted.filter((row) => memberMatches(row, needle)), [needle, sorted])
  const selected = sorted.find((row) => row.id === value)

  useEffect(() => {
    if (visible.some((row) => row.id === activeId)) return
    setActiveId(visible[0]?.id ?? '')
  }, [activeId, visible])

  useEffect(() => {
    if (!focusList.current) return
    focusList.current = false
    itemRefs.current.get(activeId)?.focus()
  }, [activeId, visible])

  const moveActive = (id: string) => {
    focusList.current = true
    setActiveId(id)
  }

  const choose = (id: string) => {
    onChange(id)
    setOpen(false)
    setQuery('')
  }

  return (
    <UsagePicker
      label={t('org.usage.pickMember')}
      open={open}
      searchLabel={t('org.usage.searchMember')}
      query={query}
      onQueryChange={setQuery}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          setQuery('')
          setActiveId(value)
        }
      }}
      onSearchKeyDown={(event) => {
        if (event.key === 'ArrowDown' && visible[0]) {
          event.preventDefault()
          moveActive(visible[0].id)
        }
      }}
      trigger={
        <span className="flex min-w-0 flex-1 items-center gap-2.5">
          {selected ? <MemberFace name={selected.name} avatar={selected.avatar} /> : null}
          <span className="min-w-0 flex-1 truncate">{selected?.name ?? ''}</span>
        </span>
      }>
      {visible.length === 0 ? (
        <p className="m-0 py-6 text-center text-[13px] text-[#8a8a8a]">{t('org.usage.noMember')}</p>
      ) : (
        <div
          role="listbox"
          aria-label={t('org.usage.pickMember')}
          className="grid gap-0.5"
          data-ui="org.usage.member-list">
          {visible.map((row, index) => {
            const selectedRow = row.id === value
            return (
              <button
                key={row.id}
                ref={(node) => {
                  if (node) itemRefs.current.set(row.id, node)
                  else itemRefs.current.delete(row.id)
                }}
                type="button"
                role="option"
                tabIndex={row.id === activeId ? 0 : -1}
                aria-selected={selectedRow}
                className={cn(
                  'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                  selectedRow
                    ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                    : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                )}
                onClick={() => choose(row.id)}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowDown') {
                    event.preventDefault()
                    const next = visible[index + 1]
                    if (next) moveActive(next.id)
                  } else if (event.key === 'ArrowUp') {
                    event.preventDefault()
                    const previous = visible[index - 1]
                    if (previous) moveActive(previous.id)
                  } else if (event.key === 'Enter') {
                    event.preventDefault()
                    choose(row.id)
                  }
                }}>
                <MemberFace name={row.name} avatar={row.avatar} />
                <span className="min-w-0 flex-1 truncate">{row.name}</span>
                {row.phone ? (
                  <span className="shrink-0 text-[12px] font-normal text-[#8a8a8a]">{row.phone}</span>
                ) : null}
                {selectedRow ? <Check aria-hidden strokeWidth={1.7} className="size-4 shrink-0" /> : null}
              </button>
            )
          })}
        </div>
      )}
    </UsagePicker>
  )
}
