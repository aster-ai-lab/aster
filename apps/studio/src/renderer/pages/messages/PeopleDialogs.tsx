import { Button, Checkbox, Dialog, DialogContent, DialogTitle, Input, SearchInput } from '@a3s-lab/ui'

import type { ImDepartment, ImPerson } from '@renderer/im/imApi'

type DialogPerson = ImPerson & { phone?: string; avatar?: string }
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BUILTIN_TOBOSS_ID } from '@renderer/im/imStore'
import { departmentTree } from '@renderer/im/imView'
import { cn } from '@renderer/utils/style'

import { PersonAvatar } from './messageAvatars'
import { ToBossMark } from './ToBossMark'

function matches(person: DialogPerson, needle: string): boolean {
  if (!needle) return true
  return `${person.displayName} ${person.email} ${person.position ?? ''} ${person.phone ?? ''}`
    .toLocaleLowerCase()
    .includes(needle)
}

function childIds(departments: readonly ImDepartment[]): Map<string, string[]> {
  const known = new Set(departments.map((row) => row.id))
  const map = new Map<string, string[]>()
  for (const row of departments) {
    if (!row.parentId || !known.has(row.parentId)) continue
    const list = map.get(row.parentId) ?? []
    list.push(row.id)
    map.set(row.parentId, list)
  }
  return map
}

function subtreeIds(root: string, children: Map<string, string[]>): Set<string> {
  const ids = new Set<string>()
  const walk = (id: string) => {
    if (ids.has(id)) return
    ids.add(id)
    for (const child of children.get(id) ?? []) walk(child)
  }
  walk(root)
  return ids
}

function PersonCheckRow({
  person,
  on,
  depth = 0,
  onToggle
}: {
  person: DialogPerson
  on: boolean
  depth?: number
  onToggle: () => void
}) {
  return (
    <label
      data-depth={depth}
      data-ui="messages.people-person"
      className={cn(
        'flex min-h-14 cursor-pointer items-center gap-3 pr-6',
        depth === 0 && 'px-6',
        on ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
      )}
      style={depth === 0 ? undefined : { paddingLeft: 24 + depth * 16 }}>
      <Checkbox size="sm" checked={on} onCheckedChange={() => onToggle()} aria-label={person.displayName} />
      <PersonAvatar name={person.displayName} phone={person.phone} avatar={person.avatar} colorKey={person.userId} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-medium text-[#171717]">{person.displayName}</span>
        <span className="block truncate text-[12px] text-[#8a8a8a]">{person.position || person.email}</span>
      </span>
    </label>
  )
}

function DepartmentCheckRow({
  label,
  depth,
  checked,
  onToggle
}: {
  label: string
  depth: number
  checked: boolean | 'indeterminate'
  onToggle: () => void
}) {
  return (
    <label
      data-depth={depth}
      data-ui="messages.people-department"
      className="flex h-10 cursor-pointer items-center gap-3 pr-6 hover:bg-[#f7f7f7]"
      style={{ paddingLeft: 24 + depth * 16 }}>
      <Checkbox size="sm" checked={checked} onCheckedChange={() => onToggle()} aria-label={label} />
      <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{label}</span>
    </label>
  )
}

/** Picks colleagues for a new group (with a name) or for adding to an existing one. */
export function PeoplePickerDialog({
  open,
  onOpenChange,
  title,
  confirmLabel,
  people,
  departments,
  withName,
  initialPicked,
  initialEmails,
  allowEmail = false,
  parseEmail,
  emailHint,
  maxPicked,
  overlayClassName,
  contentClassName,
  bots,
  onConfirm
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  confirmLabel: string
  people: DialogPerson[]
  departments?: readonly ImDepartment[]
  withName: boolean
  initialPicked?: readonly string[]
  initialEmails?: readonly string[]
  allowEmail?: boolean
  parseEmail?: (query: string) => string
  emailHint?: (email: string) => string
  maxPicked?: number
  overlayClassName?: string
  contentClassName?: string
  /** Built-in agents offered beside people. They are not organization members. */
  bots?: readonly { id: string; label: string }[]
  onConfirm: (userIds: string[], name: string, emails?: string[]) => Promise<void>
}) {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [name, setName] = useState('')
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set())
  const [emails, setEmails] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const openRef = useRef(false)
  const needle = query.trim().toLocaleLowerCase()
  const orgs = departments ?? []
  const children = useMemo(() => childIds(orgs), [orgs])
  const tree = useMemo(() => (orgs.length > 0 && !needle ? departmentTree(orgs) : []), [orgs, needle])
  const grouped = useMemo(() => {
    const known = new Set(orgs.map((row) => row.id))
    const map = new Map<string, DialogPerson[]>()
    const loose: DialogPerson[] = []
    const sorted = [...people].sort((left, right) => left.displayName.localeCompare(right.displayName, 'zh'))
    for (const person of sorted) {
      if (!person.departmentId || !known.has(person.departmentId)) {
        loose.push(person)
        continue
      }
      const list = map.get(person.departmentId) ?? []
      list.push(person)
      map.set(person.departmentId, list)
    }
    return { map, loose }
  }, [people, orgs])
  const visible = useMemo(() => {
    if (!needle) return people
    const matched = new Set<string>()
    for (const row of orgs) {
      if (!row.name.toLocaleLowerCase().includes(needle)) continue
      for (const id of subtreeIds(row.id, children)) matched.add(id)
    }
    return people.filter((person) => matches(person, needle) || matched.has(person.departmentId))
  }, [people, needle, orgs, children])
  const botRows = (bots ?? []).filter((bot) => !needle || bot.label.toLocaleLowerCase().includes(needle))
  const inviteEmail = allowEmail && parseEmail ? parseEmail(query) : ''
  const nameError = withName && name.trim().length > 64 ? t('messages.group_name_too_long') : ''
  const chosen = picked.size + emails.length
  const ready = chosen > 0 && !nameError && !busy
  const limit = maxPicked ?? Number.POSITIVE_INFINITY

  useEffect(() => {
    if (open && !openRef.current) {
      setPicked(new Set(initialPicked ?? []))
      setEmails([...(initialEmails ?? [])])
      setQuery('')
      setName('')
      setError('')
    }
    openRef.current = open
  }, [open, initialPicked, initialEmails])

  const close = (next: boolean) => {
    if (!next) {
      setQuery('')
      setName('')
      setPicked(new Set())
      setEmails([])
      setError('')
    }
    onOpenChange(next)
  }

  const roomFor = (current: ReadonlySet<string>) => limit - current.size - emails.length

  const toggle = (userId: string) => {
    setPicked((current) => {
      const next = new Set(current)
      if (next.has(userId)) next.delete(userId)
      else if (roomFor(current) > 0) next.add(userId)
      return next
    })
  }

  const toggleDepartment = (departmentId: string) => {
    const ids = subtreeIds(departmentId, children)
    const members = people.filter((person) => ids.has(person.departmentId)).map((person) => person.userId)
    if (members.length === 0) return
    setPicked((current) => {
      const next = new Set(current)
      const allOn = members.every((id) => next.has(id))
      if (allOn) {
        for (const id of members) next.delete(id)
        return next
      }
      for (const id of members) {
        if (next.has(id)) continue
        if (limit - next.size - emails.length <= 0) break
        next.add(id)
      }
      return next
    })
  }

  const addEmail = (email: string) => {
    if (!email || emails.includes(email) || chosen >= limit) return
    setEmails((current) => [...current, email])
    setQuery('')
  }

  const confirm = async () => {
    if (!ready) return
    setBusy(true)
    setError('')
    try {
      await onConfirm([...picked], name.trim(), emails)
      close(false)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent
        size="lg"
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        data-ui="messages.people-dialog"
        overlayClassName={overlayClassName}
        className={cn(
          'flex h-[540px] flex-col gap-0 overflow-hidden rounded-[12px] border border-[#ebebeb] bg-white p-0 text-[#171717] sm:max-w-[520px]',
          contentClassName
        )}>
        <div className="shrink-0 px-6 pt-4 pr-14 pb-3">
          <DialogTitle className="m-0 text-[15px] leading-6 font-medium text-[#171717]">{title}</DialogTitle>
        </div>
        <div className="flex shrink-0 flex-col gap-3 px-6 pb-3">
          {withName ? (
            <label className="flex flex-col gap-1">
              <span className="text-[12px] leading-4 text-[#8a8a8a]">{t('messages.group_name')}</span>
              <Input
                value={name}
                maxLength={80}
                aria-invalid={nameError ? true : undefined}
                placeholder={t('messages.group_name_placeholder')}
                onChange={(event) => setName(event.target.value)}
              />
              {nameError ? <span className="text-[12px] text-[#E23B3B]">{nameError}</span> : null}
            </label>
          ) : null}
          <SearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('messages.search_people')}
            placeholder={t('messages.search_people')}
            containerClassName="h-[30px] border-0 bg-[#f5f5f5] shadow-none"
          />
        </div>
        <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto border-t border-[#efefef]">
          {botRows.map((bot) => (
            <li key={bot.id} className="border-b border-[#efefef]">
              <label
                data-ui="messages.toboss-option"
                className={cn(
                  'flex min-h-14 cursor-pointer items-center gap-3 px-6',
                  picked.has(bot.id) ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
                )}>
                <Checkbox
                  size="sm"
                  checked={picked.has(bot.id)}
                  onCheckedChange={() => toggle(bot.id)}
                  aria-label={bot.label}
                />
                {bot.id === BUILTIN_TOBOSS_ID ? <ToBossMark size={36} label={bot.label} /> : null}
                <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[#171717]">{bot.label}</span>
              </label>
            </li>
          ))}
          {inviteEmail && !emails.includes(inviteEmail) ? (
            <li className="border-b border-[#efefef]">
              <button
                type="button"
                className="flex min-h-14 w-full items-center px-6 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7]"
                onClick={() => addEmail(inviteEmail)}>
                {emailHint ? emailHint(inviteEmail) : inviteEmail}
              </button>
            </li>
          ) : null}
          {emails.map((email) => (
            <li key={email} className="border-b border-[#efefef]">
              <label className="flex min-h-14 cursor-pointer items-center gap-3 px-6">
                <Checkbox
                  size="sm"
                  checked
                  onCheckedChange={() => setEmails((current) => current.filter((item) => item !== email))}
                  aria-label={email}
                />
                <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{email}</span>
              </label>
            </li>
          ))}
          {tree.length > 0 ? (
            <>
              {tree.map((node) => {
                const members = people.filter((person) => subtreeIds(node.id, children).has(person.departmentId))
                const selected = members.filter((person) => picked.has(person.userId)).length
                const checked =
                  members.length > 0 && selected === members.length ? true : selected > 0 ? 'indeterminate' : false
                return (
                  <li key={node.id}>
                    <DepartmentCheckRow
                      label={node.name}
                      depth={node.depth}
                      checked={checked}
                      onToggle={() => toggleDepartment(node.id)}
                    />
                    {(grouped.map.get(node.id) ?? []).map((person) => (
                      <PersonCheckRow
                        key={person.userId}
                        person={person}
                        depth={node.depth + 1}
                        on={picked.has(person.userId)}
                        onToggle={() => toggle(person.userId)}
                      />
                    ))}
                  </li>
                )
              })}
              {grouped.loose.map((person) => (
                <li key={person.userId}>
                  <PersonCheckRow
                    person={person}
                    on={picked.has(person.userId)}
                    onToggle={() => toggle(person.userId)}
                  />
                </li>
              ))}
            </>
          ) : visible.length === 0 && emails.length === 0 && !inviteEmail && botRows.length === 0 ? (
            <li className="px-6 py-8 text-center text-[13px] text-[#8a8a8a]">{t('messages.no_people')}</li>
          ) : (
            visible.map((person) => (
              <li key={person.userId} className="border-b border-[#efefef] last:border-b-0">
                <PersonCheckRow person={person} on={picked.has(person.userId)} onToggle={() => toggle(person.userId)} />
              </li>
            ))
          )}
        </ul>
        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-[#f0f0f0] px-6 py-3">
          <span className={cn('min-w-0 truncate text-[12px]', error ? 'text-[#E23B3B]' : 'text-[#8a8a8a]')}>
            {error || t('messages.picked_count', { count: chosen })}
          </span>
          <div className="flex shrink-0 gap-2">
            <Button variant="outline" size="md" className="rounded-[8px]" type="button" onClick={() => close(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              variant="emphasis"
              size="md"
              className="rounded-[8px]"
              type="button"
              disabled={!ready}
              onClick={() => void confirm()}>
              {confirmLabel}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/** Organization chart: departments on the left, their people on the right; a click starts a direct chat. */
export function OrgDialog({
  open,
  onOpenChange,
  departments,
  people,
  meId,
  onStartChat
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  departments: ImDepartment[]
  people: DialogPerson[]
  meId: string | undefined
  onStartChat: (userId: string) => Promise<void>
}) {
  const { t } = useTranslation()
  const [departmentId, setDepartmentId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [busyId, setBusyId] = useState<string | null>(null)
  const [error, setError] = useState('')
  const tree = useMemo(() => departmentTree(departments), [departments])
  const counts = useMemo(() => {
    const out = new Map<string, number>()
    for (const person of people) out.set(person.departmentId, (out.get(person.departmentId) ?? 0) + 1)
    return out
  }, [people])
  const needle = query.trim().toLocaleLowerCase()
  const visible = people.filter((person) =>
    needle ? matches(person, needle) : !departmentId || person.departmentId === departmentId
  )

  const start = async (userId: string) => {
    setBusyId(userId)
    setError('')
    try {
      await onStartChat(userId)
      onOpenChange(false)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusyId(null)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        size="xl"
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        data-ui="messages.org-dialog"
        className="flex h-[560px] flex-col gap-0 overflow-hidden rounded-[12px] border border-[#ebebeb] bg-white p-0 text-[#171717] shadow-[0_16px_48px_rgba(20,40,80,0.12)] sm:max-w-[760px]">
        <div className="flex shrink-0 items-center gap-4 px-6 pt-4 pr-14 pb-3">
          <DialogTitle className="m-0 shrink-0 text-[20px] leading-tight font-bold text-[#171717]">
            {t('messages.org')}
          </DialogTitle>
          <SearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('messages.search_people')}
            placeholder={t('messages.search_people')}
            containerClassName="h-[30px] min-w-0 flex-1 border-0 bg-[#f5f5f5] shadow-none"
          />
        </div>
        <div data-ui="messages.org-body" className="flex min-h-0 flex-1 border-t border-[#f0f0f0]">
          <nav
            aria-label={t('messages.departments')}
            className="w-[220px] shrink-0 overflow-y-auto border-r border-[#f0f0f0] py-2">
            <DepartmentButton
              label={t('messages.all_people')}
              count={people.length}
              depth={0}
              active={!departmentId}
              onClick={() => setDepartmentId(null)}
            />
            {tree.map((node) => (
              <DepartmentButton
                key={node.id}
                label={node.name}
                count={counts.get(node.id) ?? 0}
                depth={node.depth}
                active={departmentId === node.id}
                onClick={() => setDepartmentId(node.id)}
              />
            ))}
          </nav>
          <div className="flex min-w-0 flex-1 flex-col">
            {error ? <p className="m-0 px-4 pt-2 text-[12px] text-[#E23B3B]">{error}</p> : null}
            <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto p-2">
              {visible.length === 0 ? (
                <li className="px-3 py-8 text-center text-[13px] text-[#8a8a8a]">{t('messages.no_people')}</li>
              ) : (
                visible.map((person) => {
                  const self = person.userId === meId
                  return (
                    <li key={person.userId}>
                      <button
                        type="button"
                        data-ui="messages.org-person"
                        disabled={self || busyId !== null}
                        onClick={() => void start(person.userId)}
                        className="flex w-full items-center gap-3 rounded-[8px] px-3 py-1.5 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 disabled:cursor-default disabled:hover:bg-transparent">
                        <PersonAvatar
                          name={person.displayName}
                          phone={person.phone}
                          avatar={person.avatar}
                          colorKey={person.userId}
                        />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-[13px] text-[#171717]">
                            {person.displayName}
                            {self ? (
                              <span className="ml-1 text-[12px] text-[#8a8a8a]">({t('messages.me')})</span>
                            ) : null}
                          </span>
                          <span className="block truncate text-[12px] text-[#8a8a8a]">
                            {[person.position, person.email].filter(Boolean).join(' · ')}
                          </span>
                        </span>
                        {!self ? (
                          <span className="shrink-0 text-[12px] text-[#5f5f5f]">
                            {busyId === person.userId ? t('messages.opening') : t('messages.start_chat')}
                          </span>
                        ) : null}
                      </button>
                    </li>
                  )
                })
              )}
            </ul>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function DepartmentButton({
  label,
  count,
  depth,
  active,
  onClick
}: {
  label: string
  count: number
  depth: number
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      aria-current={active ? 'true' : undefined}
      onClick={onClick}
      style={{ paddingLeft: 12 + depth * 14 }}
      className={cn(
        'flex h-8 w-full items-center gap-2 pr-3 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        active ? 'bg-[#f2f3f5] text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'
      )}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="shrink-0 text-[12px] text-[#8a8a8a]">{count}</span>
    </button>
  )
}
