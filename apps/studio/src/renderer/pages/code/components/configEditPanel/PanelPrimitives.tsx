import type { FC } from 'react'

import { Button } from '@a3s-lab/ui'
import { cn } from '@renderer/utils/style'

/** "Advanced Settings" toggle (ghost button with a leading icon). */
export const AdvancedSettingsButton: FC<React.ComponentPropsWithoutRef<typeof Button>> = ({
  type = 'button',
  variant = 'ghost',
  size = 'sm',
  className,
  ...props
}) => (
  <Button
    type={type}
    variant={variant}
    size={size}
    className={cn(
      'h-8 w-fit gap-1.5 bg-transparent px-0 text-foreground opacity-70 shadow-none hover:bg-transparent hover:text-foreground hover:opacity-100 active:bg-transparent active:opacity-100',
      className
    )}
    {...props}
  />
)
