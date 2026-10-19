import { Avatar, AvatarFallback, AvatarImage, EmojiIcon } from '@a3s-lab/ui'
import { cn } from '@a3s-lab/ui/lib/utils'
import type { ComponentPropsWithoutRef, CSSProperties, ReactNode } from 'react'

import { readAgentAvatar } from '@renderer/components/Avatar/agentAvatarValue'
import { useCatalogIconSrc } from '@renderer/hooks/useCatalogIconSrc'

export const MESSAGE_AVATAR_SIZE = 30
export const MESSAGE_AVATAR_CONTAINER_CLASS =
  'message-avatar flex size-[30px] shrink-0 items-center justify-center overflow-hidden rounded-full p-0'
export const MESSAGE_AVATAR_INNER_CLASS = 'size-full rounded-full p-0'
export const MESSAGE_AVATAR_IMAGE_CLASS = 'size-full object-cover p-0'
export const MESSAGE_AVATAR_FALLBACK_CLASS = 'size-full rounded-full p-0'
export const MESSAGE_MODEL_AVATAR_ICON_CLASS = 'size-full'

export const MessageAvatarFrame = ({ className, ...props }: ComponentPropsWithoutRef<'div'>) => (
  <div className={cn(MESSAGE_AVATAR_CONTAINER_CLASS, className)} {...props} />
)

interface MessageAvatarProps extends ComponentPropsWithoutRef<'div'> {
  avatar?: string
  fallback?: ReactNode
  fallbackAvatarStyle?: CSSProperties
}

const MessageAvatar = ({
  avatar = '',
  fallback,
  fallbackAvatarStyle,
  className,
  onClick,
  ...props
}: MessageAvatarProps) => {
  const clickable = !!onClick
  const value = readAgentAvatar(avatar)
  const catalogSrc = useCatalogIconSrc(value?.kind === 'catalog-icon' ? value.path : undefined)
  const src = value?.kind === 'image' ? value.src : catalogSrc

  return (
    <MessageAvatarFrame className={cn(clickable && 'cursor-pointer', className)} onClick={onClick} {...props}>
      {value?.kind === 'glyph' ? (
        <EmojiIcon emoji={value.glyph} size={MESSAGE_AVATAR_SIZE} />
      ) : (
        <Avatar className={MESSAGE_AVATAR_INNER_CLASS} style={!src ? fallbackAvatarStyle : undefined}>
          {src && <AvatarImage className={MESSAGE_AVATAR_IMAGE_CLASS} src={src} />}
          {fallback !== undefined && (
            <AvatarFallback className={MESSAGE_AVATAR_FALLBACK_CLASS}>{fallback}</AvatarFallback>
          )}
        </Avatar>
      )}
    </MessageAvatarFrame>
  )
}

export default MessageAvatar
