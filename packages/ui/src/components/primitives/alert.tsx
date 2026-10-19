import { cva, type VariantProps } from 'class-variance-authority'
import { CheckCircle2, Info, TriangleAlert, XCircle } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'

const alertVariants = cva(
  cn(
    'relative flex w-full items-start gap-3 rounded-md border px-3 py-2.5 text-sm leading-5 shadow-xs',
    '[&_svg]:pointer-events-none [&_svg]:shrink-0'
  ),
  {
    variants: {
      type: {
        info: 'border-info-border bg-info-subtle text-info-subtle-foreground',
        success: 'border-success-border bg-success-subtle text-success-subtle-foreground',
        warning: 'border-warning-border bg-warning-subtle text-warning-subtle-foreground',
        error: 'border-error-border bg-error-subtle text-error-subtle-foreground'
      }
    },
    defaultVariants: {
      type: 'info'
    }
  }
)

const alertIconVariants = cva('', {
  variants: {
    type: {
      info: 'text-info',
      success: 'text-success',
      warning: 'text-warning',
      error: 'text-error'
    }
  },
  defaultVariants: {
    type: 'info'
  }
})

const alertIconContainerVariants = cva('mt-0.5 flex shrink-0 items-center', {
  variants: {
    type: {
      info: 'text-info [&_.lucide:not(.lucide-custom)]:!text-info',
      success: 'text-success [&_.lucide:not(.lucide-custom)]:!text-success',
      warning: 'text-warning [&_.lucide:not(.lucide-custom)]:!text-warning',
      error: 'text-error [&_.lucide:not(.lucide-custom)]:!text-error'
    }
  },
  defaultVariants: {
    type: 'info'
  }
})

const alertIcons = {
  info: Info,
  success: CheckCircle2,
  warning: TriangleAlert,
  error: XCircle
} satisfies Record<NonNullable<AlertProps['type']>, React.ComponentType<{ className?: string; size?: number }>>

/** Console-kit tone names; `warn`/`danger` map onto the warning/error variants. */
export type AlertTone = 'info' | 'success' | 'warn' | 'danger'

const toneToType: Record<AlertTone, NonNullable<VariantProps<typeof alertVariants>['type']>> = {
  info: 'info',
  success: 'success',
  warn: 'warning',
  danger: 'error'
}

type AlertProps = Omit<React.ComponentProps<'div'>, 'title'> &
  VariantProps<typeof alertVariants> & {
    message?: React.ReactNode
    /** Console-kit alias of `message`. */
    title?: React.ReactNode
    description?: React.ReactNode
    action?: React.ReactNode
    icon?: React.ReactNode
    showIcon?: boolean
    /** Console-kit alias of `type`. */
    tone?: AlertTone
  }

function Alert({
  className,
  type,
  tone,
  message,
  title,
  description,
  action,
  icon,
  showIcon = false,
  children,
  role,
  ref,
  ...props
}: AlertProps) {
  const resolvedType = (tone ? toneToType[tone] : type) ?? 'info'
  const heading = message ?? title
  const Icon = alertIcons[resolvedType]
  const alertRole = role ?? (resolvedType === 'error' ? 'alert' : 'status')

  return (
    <div
      ref={ref}
      role={alertRole}
      data-slot="alert"
      data-type={resolvedType}
      className={cn(alertVariants({ type: resolvedType }), className)}
      {...props}>
      {showIcon && (
        <span data-slot="alert-icon" data-type={resolvedType} className={alertIconContainerVariants({ type: resolvedType })}>
          {icon ?? <Icon size={16} className={cn('lucide-custom', alertIconVariants({ type: resolvedType }))} />}
        </span>
      )}
      <div data-slot="alert-content" className="min-w-0 flex-1">
        {children ?? (
          <>
            {heading && (
              <div data-slot="alert-message" className="font-medium">
                {heading}
              </div>
            )}
            {description && (
              <div data-slot="alert-description" className="mt-1 text-xs leading-5 opacity-90">
                {description}
              </div>
            )}
          </>
        )}
      </div>
      {action && (
        <div data-slot="alert-action" className="ml-2 flex shrink-0 items-center">
          {action}
        </div>
      )}
    </div>
  )
}

Alert.displayName = 'Alert'

export { Alert, alertVariants }
export type { AlertProps }
