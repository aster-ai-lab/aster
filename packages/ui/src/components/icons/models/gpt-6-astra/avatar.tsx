import { Avatar, AvatarFallback } from '../../../primitives/avatar'
import { cn } from '../../../../lib/utils'

import { type IconAvatarProps } from '../../types'
import { Gpt6AstraLight } from './light'

export function Gpt6AstraAvatar({ size = 32, shape = 'circle', className }: Omit<IconAvatarProps, 'icon'>) {
  return (
    <Avatar
      className={cn('overflow-hidden', shape === 'circle' ? 'rounded-full' : 'rounded-[20%]', className)}
      style={{ width: size, height: size }}>
      <AvatarFallback className="text-foreground bg-background">
        <Gpt6AstraLight style={{ width: size, height: size }} />
      </AvatarFallback>
    </Avatar>
  )
}
