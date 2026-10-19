import { Avatar, AvatarFallback } from '../../../primitives/avatar'
import { cn } from '../../../../lib/utils'

import { type IconAvatarProps } from '../../types'
import { SerplyLight } from './light'

export function SerplyAvatar({ size = 32, shape = 'circle', className }: Omit<IconAvatarProps, 'icon'>) {
  return (
    <Avatar
      className={cn('overflow-hidden', shape === 'circle' ? 'rounded-full' : 'rounded-[20%]', className)}
      style={{ width: size, height: size }}>
      <AvatarFallback className="text-foreground bg-background">
        <SerplyLight style={{ width: size, height: size }} />
      </AvatarFallback>
    </Avatar>
  )
}
