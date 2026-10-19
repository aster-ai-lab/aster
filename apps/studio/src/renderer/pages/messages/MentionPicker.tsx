import { Users } from 'lucide-react'
import { useEffect, useRef } from 'react'

import { BUILTIN_TOBOSS_ID } from '@renderer/im/imStore'
import { cn } from '@renderer/utils/style'

import { PersonAvatar } from './messageAvatars'
import { ToBossMark } from './ToBossMark'
import { ToBotMark } from './ToBotMark'

export type MentionOption = {
  id: string
  kind: 'all' | 'member' | 'bot'
  label: string
  online?: boolean
  phone?: string
  avatar?: string
}

/** The member list that opens above the composer while an @ query is being typed. */
export function MentionPicker({
  label,
  empty,
  options,
  activeIndex,
  onActive,
  onChoose
}: {
  label: string
  empty: string
  options: readonly MentionOption[]
  activeIndex: number
  onActive: (index: number) => void
  onChoose: (option: MentionOption) => void
}) {
  const activeRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex, options])

  return (
    <div
      id="messages-mention-picker"
      role="listbox"
      aria-label={label}
      data-ui="messages.mention-picker"
      className="absolute right-0 bottom-full left-0 z-30 mb-1 max-h-60 overflow-y-auto rounded-[10px] border border-[#ebebeb] bg-white p-1 shadow-[0_8px_22px_rgba(20,40,80,0.08)]">
      {options.length === 0 ? (
        <p className="m-0 px-2 py-2 text-[13px] text-[#8a8a8a]">{empty}</p>
      ) : (
        options.map((option, index) => {
          const active = index === activeIndex
          return (
            <button
              key={option.id}
              ref={active ? activeRef : undefined}
              id={`messages-mention-${option.id}`}
              type="button"
              role="option"
              aria-selected={active}
              data-ui="messages.mention-option"
              data-mention-id={option.id}
              onMouseEnter={() => onActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onChoose(option)}
              className={cn(
                'flex h-9 w-full items-center gap-2 rounded-[6px] px-1.5 text-left text-[13px] text-[#171717] outline-none',
                active ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]'
              )}>
              {option.kind === 'all' ? (
                <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-[#f5f5f5] text-[#3f3f3f]">
                  <Users className="size-4" strokeWidth={1.7} />
                </span>
              ) : option.kind === 'bot' && option.id === BUILTIN_TOBOSS_ID ? (
                <ToBossMark size={28} label={option.label} />
              ) : option.kind === 'bot' ? (
                <ToBotMark size={28} label={option.label} />
              ) : (
                <PersonAvatar
                  name={option.label}
                  phone={option.phone}
                  avatar={option.avatar}
                  colorKey={option.id}
                  size={28}
                  online={option.online}
                />
              )}
              <span className="min-w-0 truncate">{option.label}</span>
            </button>
          )
        })
      )}
    </div>
  )
}
