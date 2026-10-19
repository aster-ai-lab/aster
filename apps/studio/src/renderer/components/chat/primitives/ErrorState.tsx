import type { ComponentProps, ReactNode } from 'react'

import { Alert } from '@a3s-lab/ui'
import { cn } from '@a3s-lab/ui/lib/utils'

export interface ErrorStateProps extends Omit<ComponentProps<'div'>, 'title'> {
  action?: ReactNode
  description?: ReactNode
  icon?: ReactNode
  title?: ReactNode
}

export function ErrorState({ action, className, description, icon, title, ...props }: ErrorStateProps) {
  return (
    <Alert
      data-slot="chat-error-state"
      type="error"
      showIcon
      icon={icon}
      message={title}
      description={description}
      action={action}
      className={cn('min-w-0', className)}
      {...props}
    />
  )
}
