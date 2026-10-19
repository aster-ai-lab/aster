import { EmojiIcon } from '@a3s-lab/ui'
import { DefaultAvatar } from '@renderer/components/DefaultAvatar'
import { isEmoji } from '@renderer/utils/naming'

import type { SidebarUser } from './types'

export function DefaultLogo({ title }: { title: string }) {
  return (
    <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-secondary text-sm font-medium text-foreground">
      {title ? title.slice(0, 1).toUpperCase() : ''}
    </div>
  )
}

/** Returns true if the string is NOT a URL — i.e., should be rendered as text (emoji or initial). */
function isTextAvatar(str?: string): boolean {
  if (
    !str ||
    str.startsWith('data:') ||
    str.startsWith('http') ||
    str.startsWith('/') ||
    str.startsWith('blob:') ||
    str.startsWith('file:')
  ) {
    return false
  }
  return true
}

export function UserAvatar({
  user,
  className,
  ring = true
}: {
  user: SidebarUser
  className?: string
  ring?: boolean
}) {
  const isEmojiAvatar = user.avatar ? isEmoji(user.avatar) : false
  const customAvatar = Boolean((user.avatar && !isTextAvatar(user.avatar)) || isEmojiAvatar)

  return (
    <div
      className={`${customAvatar ? 'overflow-hidden rounded-full' : ''} ${customAvatar && ring ? 'ring-1 ring-border' : ''} ${className ?? ''}`}>
      {user.avatar && !isTextAvatar(user.avatar) ? (
        <img src={user.avatar} alt={user.name} className="h-full w-full object-cover" />
      ) : isEmojiAvatar ? (
        <EmojiIcon emoji={user.avatar!} fluid />
      ) : (
        <DefaultAvatar label={user.name} className="h-full w-full" />
      )}
    </div>
  )
}
