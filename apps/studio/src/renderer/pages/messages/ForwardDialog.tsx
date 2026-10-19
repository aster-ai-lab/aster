import { Button, Dialog, DialogContent, DialogTitle, SearchInput } from '@a3s-lab/ui'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import type { ImDepartment } from '@renderer/im/imApi'
import { departmentTree } from '@renderer/im/imView'
import { cn } from '@renderer/utils/style'

import { GroupAvatar, PersonAvatar, type GroupFace } from './messageAvatars'
import { ToBotMark } from './ToBotMark'

export type ForwardChat = {
  id: string
  title: string
  tobot: boolean
  kind: 'direct' | 'group' | 'transfer'
  name: string
  phone?: string
  avatar?: string
  colorKey?: string
  faces?: GroupFace[]
}

export type ForwardPerson = {
  userId: string
  displayName: string
  email: string
  position: string | null
  departmentId: string
  phone?: string
  avatar?: string
}

export type ForwardChoice = { kind: 'chat'; id: string; tobot: boolean } | { kind: 'person'; userId: string }

const RECENT = ''

function sameChoice(left: ForwardChoice | null, right: ForwardChoice): boolean {
  if (!left || left.kind !== right.kind) return false
  if (left.kind === 'chat' && right.kind === 'chat') return left.id === right.id
  if (left.kind === 'person' && right.kind === 'person') return left.userId === right.userId
  return false
}

function personMatches(person: ForwardPerson, needle: string): boolean {
  if (!needle) return true
  return `${person.displayName} ${person.email} ${person.position ?? ''}`.toLocaleLowerCase().includes(needle)
}

function ChatFace({ chat }: { chat: ForwardChat }) {
  if (chat.kind === 'transfer') return <ToBotMark size={36} />
  if (chat.kind === 'group') {
    return <GroupAvatar faces={chat.faces?.length ? chat.faces : [{ kind: 'person', name: chat.title }]} />
  }
  return <PersonAvatar name={chat.name} phone={chat.phone} avatar={chat.avatar} colorKey={chat.colorKey} />
}

function NavButton({
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
      aria-label={label}
      aria-current={active ? 'true' : undefined}
      onClick={onClick}
      style={{ paddingLeft: 12 + depth * 14 }}
      className={cn(
        'flex h-8 w-full items-center gap-2 pr-3 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        active ? 'bg-[#f2f3f5] text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'
      )}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="shrink-0 text-[12px] text-[#8a8a8a]" aria-hidden>
        {count}
      </span>
    </button>
  )
}

/** Recent chats, plus colleagues filtered by department. One choice, then send. */
export function ForwardDialog({
  open,
  mode,
  chats,
  people,
  departments,
  onOpenChange,
  onConfirm
}: {
  open: boolean
  mode: 'each' | 'merge'
  chats: readonly ForwardChat[]
  people: readonly ForwardPerson[]
  departments: readonly ImDepartment[]
  onOpenChange: (open: boolean) => void
  onConfirm: (choice: ForwardChoice) => Promise<boolean>
}) {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [departmentId, setDepartmentId] = useState(RECENT)
  const [choice, setChoice] = useState<ForwardChoice | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const tree = useMemo(() => departmentTree(departments), [departments])
  const counts = useMemo(() => {
    const out = new Map<string, number>()
    for (const person of people) out.set(person.departmentId, (out.get(person.departmentId) ?? 0) + 1)
    return out
  }, [people])
  const needle = query.trim().toLocaleLowerCase()
  const searching = needle.length > 0
  const visibleChats = chats.filter((chat) => !needle || chat.title.toLocaleLowerCase().includes(needle))
  const visiblePeople = people.filter((person) =>
    searching ? personMatches(person, needle) : departmentId !== RECENT && person.departmentId === departmentId
  )
  const showChats = searching || departmentId === RECENT
  const showPeople = searching || departmentId !== RECENT

  useEffect(() => {
    if (open) return
    setQuery('')
    setDepartmentId(RECENT)
    setChoice(null)
    setBusy(false)
    setError('')
  }, [open])

  const confirm = async () => {
    if (!choice || busy) return
    setBusy(true)
    setError('')
    try {
      const sent = await onConfirm(choice)
      if (!sent) setError(t('messages.error.send_failed'))
    } finally {
      setBusy(false)
    }
  }

  const empty = (!showChats || visibleChats.length === 0) && (!showPeople || visiblePeople.length === 0)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        size="xl"
        closeLabel={t('common.close')}
        aria-describedby={undefined}
        data-ui="messages.forward-dialog"
        className="flex h-[560px] flex-col gap-0 overflow-hidden rounded-[12px] border border-[#ebebeb] bg-white p-0 text-[#171717] shadow-[0_16px_48px_rgba(20,40,80,0.12)] sm:max-w-[760px]">
        <div className="flex shrink-0 items-center gap-4 px-6 pt-4 pr-14 pb-3">
          <DialogTitle className="m-0 shrink-0 text-[20px] leading-tight font-bold text-[#171717]">
            {t(mode === 'merge' ? 'messages.forward_merge' : 'messages.forward_each')}
          </DialogTitle>
          <SearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('messages.forward_search')}
            placeholder={t('messages.forward_search')}
            containerClassName="h-[30px] min-w-0 flex-1 border-0 bg-[#f5f5f5] shadow-none"
          />
        </div>
        <div className="flex min-h-0 flex-1 border-t border-[#f0f0f0]">
          <nav
            aria-label={t('messages.departments')}
            className="w-[220px] shrink-0 overflow-y-auto border-r border-[#f0f0f0] py-2">
            <NavButton
              label={t('messages.forward_recent')}
              count={chats.length}
              depth={0}
              active={!searching && departmentId === RECENT}
              onClick={() => setDepartmentId(RECENT)}
            />
            {tree.map((node) => (
              <NavButton
                key={node.id}
                label={node.name}
                count={counts.get(node.id) ?? 0}
                depth={node.depth}
                active={!searching && departmentId === node.id}
                onClick={() => setDepartmentId(node.id)}
              />
            ))}
          </nav>
          <div className="flex min-w-0 flex-1 flex-col">
            <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto p-2">
              {empty ? (
                <li className="px-3 py-8 text-center text-[13px] text-[#8a8a8a]">
                  {t(searching || departmentId !== RECENT ? 'messages.no_people' : 'messages.forward_none')}
                </li>
              ) : null}
              {showChats && visibleChats.length > 0 ? (
                <>
                  {searching ? (
                    <li className="px-3 pt-1 pb-1 text-[12px] text-[#8a8a8a]">{t('messages.forward_recent')}</li>
                  ) : null}
                  {visibleChats.map((chat) => {
                    const next: ForwardChoice = { kind: 'chat', id: chat.id, tobot: chat.tobot }
                    const on = sameChoice(choice, next)
                    return (
                      <li key={chat.id}>
                        <button
                          type="button"
                          aria-label={chat.title}
                          aria-pressed={on}
                          data-ui="messages.forward-target"
                          onClick={() => setChoice(next)}
                          className={cn(
                            'flex w-full items-center gap-3 rounded-[8px] px-3 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                            on ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
                          )}>
                          <ChatFace chat={chat} />
                          <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{chat.title}</span>
                        </button>
                      </li>
                    )
                  })}
                </>
              ) : null}
              {showPeople && visiblePeople.length > 0 ? (
                <>
                  {searching ? (
                    <li className="px-3 pt-2 pb-1 text-[12px] text-[#8a8a8a]">{t('messages.forward_contacts')}</li>
                  ) : null}
                  {visiblePeople.map((person) => {
                    const next: ForwardChoice = { kind: 'person', userId: person.userId }
                    const on = sameChoice(choice, next)
                    return (
                      <li key={person.userId}>
                        <button
                          type="button"
                          aria-label={person.displayName}
                          aria-pressed={on}
                          data-ui="messages.forward-target"
                          onClick={() => setChoice(next)}
                          className={cn(
                            'flex w-full items-center gap-3 rounded-[8px] px-3 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                            on ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
                          )}>
                          <PersonAvatar
                            name={person.displayName}
                            phone={person.phone}
                            avatar={person.avatar}
                            colorKey={person.userId}
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-[13px] text-[#171717]">{person.displayName}</span>
                            <span className="block truncate text-[12px] text-[#8a8a8a]">
                              {[person.position, person.email].filter(Boolean).join(' · ')}
                            </span>
                          </span>
                        </button>
                      </li>
                    )
                  })}
                </>
              ) : null}
            </ul>
          </div>
        </div>
        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-[#f0f0f0] px-6 py-3">
          <span className={cn('min-w-0 truncate text-[12px]', error ? 'text-[#E23B3B]' : 'text-[#8a8a8a]')}>
            {error}
          </span>
          <div className="flex shrink-0 gap-2">
            <Button variant="outline" size="md" className="rounded-[8px]" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              variant="emphasis"
              size="md"
              className="rounded-[8px]"
              disabled={!choice || busy}
              onClick={() => void confirm()}>
              {t('messages.forward_send')}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
