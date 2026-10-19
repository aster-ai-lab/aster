import { Avatar, AvatarFallback } from '../../../primitives/avatar'
import { cn } from '../../../../lib/utils'

import { type IconAvatarProps } from '../../types'
import { ComfyuiLight } from './light'

export function ComfyuiAvatar({ size = 32, shape = 'circle', className }: Omit<IconAvatarProps, 'icon'>) {
  return (
    <Avatar
      className={cn('overflow-hidden', shape === 'circle' ? 'rounded-full' : 'rounded-[20%]', className)}
      style={{ width: size, height: size }}>
      <AvatarFallback className="text-foreground">
        <ComfyuiLight style={{ width: size, height: size }} />
      </AvatarFallback>
    </Avatar>
  )
}
