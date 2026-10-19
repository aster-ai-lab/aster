import { SearchInput, Tooltip } from '@a3s-lab/ui'
import { Flag, Hash, Lightbulb, PawPrint, Plane, Smile, Trophy, User, Utensils, type LucideIcon } from 'lucide-react'
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent
} from 'react'
import { useTranslation } from 'react-i18next'

import { useRecentEmojis } from '@renderer/components/EmojiPicker/useRecentEmojis'
import { useRestingDialog } from '@renderer/components/restingDialog'
import Scrollbar from '@renderer/components/Scrollbar'
import { cn } from '@renderer/utils/style'

import { MESSAGE_EMOJI_GROUPS, MESSAGE_EMOJI_GROUP_LABEL, type MessageEmojiGroup } from './messageEmojiGroups'

const COLUMNS = 8
const RECENT_LIMIT = 24
const EMOJI_TOOLTIP_DELAY = 300

const GROUP_ICON: Record<MessageEmojiGroup, LucideIcon> = {
  smileys: Smile,
  people: User,
  animals: PawPrint,
  food: Utensils,
  travel: Plane,
  activities: Trophy,
  objects: Lightbulb,
  symbols: Hash,
  flags: Flag
}

type CatalogModule = typeof import('./messageEmojiCatalog')

let catalogModule: CatalogModule | null = null
let catalogPromise: Promise<CatalogModule> | null = null

const loadCatalog = (): Promise<CatalogModule> => {
  if (!catalogPromise) {
    catalogPromise = import('./messageEmojiCatalog')
      .then((mod) => {
        catalogModule = mod
        return mod
      })
      .catch((error: unknown) => {
        catalogPromise = null
        throw error
      })
  }
  return catalogPromise
}

const keepFieldFocus = (event: ReactMouseEvent) => {
  const target = event.target
  if (target instanceof Element && target.closest('input')) return
  event.preventDefault()
}

function EmojiButton({ emoji, label, onPick }: { emoji: string; label: string; onPick: (emoji: string) => void }) {
  return (
    <Tooltip content={label} placement="top" delay={EMOJI_TOOLTIP_DELAY} sideOffset={6} asChild>
      <button
        type="button"
        data-emoji=""
        onClick={() => onPick(emoji)}
        className="flex size-10 items-center justify-center rounded-[8px] text-[22px] leading-none outline-none hover:bg-[#f7f7f7] focus-visible:bg-[#f2f3f5] focus-visible:ring-2 focus-visible:ring-black/10">
        {emoji}
      </button>
    </Tooltip>
  )
}

export function MessageEmojiPanel({ open, onPick }: { open: boolean; onPick: (emoji: string) => void }) {
  const presence = useRestingDialog(open)
  const { t, i18n } = useTranslation()
  const { recent, pushRecent } = useRecentEmojis()
  const [catalog, setCatalog] = useState<CatalogModule | null>(catalogModule)
  const [failed, setFailed] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState<MessageEmojiGroup>('smileys')
  const listRef = useRef<HTMLDivElement>(null)
  const trimmed = query.trim()

  useEffect(() => {
    if (!open) return
    setQuery('')
    setActive('smileys')
  }, [open])

  useEffect(() => {
    if (!open || catalogModule) return
    let live = true
    void loadCatalog()
      .then((mod) => {
        if (live) setCatalog(mod)
      })
      .catch(() => {
        if (live) setFailed(true)
      })
    return () => {
      live = false
    }
  }, [open])

  const results = useMemo(() => (catalog && trimmed ? catalog.searchMessageEmojis(trimmed) : null), [catalog, trimmed])

  const recentShown = useMemo(() => [...new Set(recent.filter((item) => item.trim()))].slice(0, RECENT_LIMIT), [recent])

  const pick = (emoji: string) => {
    pushRecent(emoji)
    onPick(emoji)
  }

  const labelFor = (emoji: string) => (catalog ? catalog.messageEmojiLabel(emoji, i18n.language) : emoji)

  const onSearchKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault()
      if (results && results.length > 0) pick(results[0].emoji)
      return
    }
    if (event.key !== 'ArrowDown') return
    const first = listRef.current?.querySelector<HTMLButtonElement>('button[data-emoji]')
    if (!first) return
    event.preventDefault()
    first.focus()
  }

  const onListKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (
      event.key !== 'ArrowLeft' &&
      event.key !== 'ArrowRight' &&
      event.key !== 'ArrowUp' &&
      event.key !== 'ArrowDown'
    ) {
      return
    }
    const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button[data-emoji]')]
    const index = buttons.findIndex((button) => button === document.activeElement)
    if (index < 0) return
    const delta =
      event.key === 'ArrowLeft' ? -1 : event.key === 'ArrowRight' ? 1 : event.key === 'ArrowUp' ? -COLUMNS : COLUMNS
    const next = buttons[index + delta]
    if (!next) return
    event.preventDefault()
    next.focus()
  }

  const showCategory = (group: MessageEmojiGroup) => {
    setActive(group)
    setQuery('')
    const list = listRef.current
    if (list) list.scrollTop = 0
  }

  const activeItems = catalog?.messageEmojisByGroup[active] ?? []

  if (!presence.present) return null

  return (
    <div
      data-ui="messages.emoji-panel"
      role="dialog"
      aria-label={t('messages.action.emoji')}
      aria-busy={!catalog && !failed}
      data-state={presence.state}
      onMouseDown={keepFieldFocus}
      className={`a3s-popup-rise absolute bottom-9 left-0 z-30 flex w-[352px] flex-col overflow-hidden rounded-[12px] border border-[#e5e5e5] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]${
        presence.state === 'closed' ? ' pointer-events-none' : ''
      }`}>
      <div className="p-2 pb-1.5">
        <SearchInput
          autoFocus
          size="sm"
          value={query}
          placeholder={t('emoji_picker.search')}
          aria-label={t('emoji_picker.search')}
          clearLabel={t('common.clear')}
          onClear={() => setQuery('')}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={onSearchKeyDown}
          containerClassName="h-[30px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
          className="text-[13px] placeholder:text-[#8a8a8a]"
        />
      </div>
      <Scrollbar
        ref={listRef}
        onKeyDown={onListKeyDown}
        className="h-[320px] max-h-[min(320px,calc(100vh-9rem))] overscroll-contain px-1.5">
        {!catalog && !failed ? (
          <p className="m-0 px-2 py-8 text-center text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
        ) : null}
        {failed ? (
          <p className="m-0 px-2 py-8 text-center text-[13px] text-[#8a8a8a]">{t('emoji_picker.empty')}</p>
        ) : null}
        {results ? (
          results.length > 0 ? (
            <div className="grid grid-cols-8 pb-1.5">
              {results.map((item) => (
                <EmojiButton key={item.emoji} emoji={item.emoji} label={labelFor(item.emoji)} onPick={pick} />
              ))}
            </div>
          ) : (
            <p className="m-0 px-2 py-8 text-center text-[13px] text-[#8a8a8a]">{t('emoji_picker.empty')}</p>
          )
        ) : null}
        {catalog && !results ? (
          <>
            {active === 'smileys' && recentShown.length > 0 ? (
              <section>
                <div className="sticky top-0 z-10 bg-white px-1.5 py-1 text-[12px] leading-4 text-[#8a8a8a]">
                  {t('emoji_picker.categories.recent')}
                </div>
                <div className="grid grid-cols-8">
                  {recentShown.map((emoji) => (
                    <EmojiButton key={`recent-${emoji}`} emoji={emoji} label={labelFor(emoji)} onPick={pick} />
                  ))}
                </div>
              </section>
            ) : null}
            <section>
              <div className="sticky top-0 z-10 bg-white px-1.5 py-1 text-[12px] leading-4 text-[#8a8a8a]">
                {t(MESSAGE_EMOJI_GROUP_LABEL[active])}
              </div>
              <div className="grid grid-cols-8 pb-1.5">
                {activeItems.map((item) => (
                  <EmojiButton key={item.emoji} emoji={item.emoji} label={labelFor(item.emoji)} onPick={pick} />
                ))}
              </div>
            </section>
          </>
        ) : null}
      </Scrollbar>
      <div className="flex items-center justify-between border-t border-[#efefef] px-1.5 py-1">
        {MESSAGE_EMOJI_GROUPS.map((group) => {
          const Icon = GROUP_ICON[group]
          const pressed = !trimmed && active === group
          return (
            <button
              key={group}
              type="button"
              aria-pressed={pressed}
              aria-label={t(MESSAGE_EMOJI_GROUP_LABEL[group])}
              onClick={() => showCategory(group)}
              className={cn(
                'flex size-8 items-center justify-center rounded-[8px] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10',
                pressed ? 'bg-[#f2f3f5] text-[#171717]' : 'text-[#8a8a8a] hover:text-[#171717]'
              )}>
              <Icon size={16} strokeWidth={1.7} aria-hidden />
            </button>
          )
        })}
      </div>
    </div>
  )
}
