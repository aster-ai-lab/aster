import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { Loader } from 'lucide-react'
import * as React from 'react'
import { Link } from 'react-router-dom'

import { cn } from '../../lib/utils'
import { ArcButton } from './arc-button'

import arcButton from '../arc/button/button.module.css'

const buttonVariants = cva(
  cn(
    'arc',
    arcButton.button,
    'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md font-normal shrink-0 outline-none',
    'active:opacity-80',
    'disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground',
    "aria-invalid:border-destructive aria-invalid:ring-destructive/20 [&_.lucide:not(.lucide-custom)]:text-current [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
    'data-[busy=true]:cursor-progress data-[busy=true]:opacity-100'
  ),
  {
    variants: {
      variant: {
        default: arcButton.primary,
        primary: arcButton.primary,
        accent: arcButton.primary,
        emphasis: arcButton.primary,
        destructive: arcButton.danger,
        outline: arcButton.secondary,
        secondary: cn(arcButton.secondary, arcButton.tonal),
        ghost: arcButton.ghost,
        link: 'border-transparent bg-transparent text-foreground underline-offset-4 hover:underline focus-visible:underline'
      },
      size: {
        default: arcButton.sm,
        sm: arcButton.sm,
        md: arcButton.md,
        lg: arcButton.lg,
        icon: arcButton.icon,
        'icon-sm': arcButton['icon-sm'],
        'icon-lg': arcButton['icon-lg'],
        'icon-navbar': arcButton['icon-navbar']
      }
    },
    defaultVariants: { variant: 'default', size: 'default' }
  }
)

type ButtonProps = React.ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
    loading?: boolean
    loadingIcon?: React.ReactNode
    loadingIconClassName?: string
    /** Render as a react-router Link (console router links). */
    to?: string
    /** Render as a plain anchor. */
    href?: string
  }

function Button({
  className,
  variant,
  size,
  asChild = false,
  loading,
  loadingIcon,
  loadingIconClassName,
  disabled,
  children,
  onClick,
  to,
  href,
  ...props
}: ButtonProps) {
  const Comp = (asChild ? Slot : to !== undefined ? Link : href !== undefined ? 'a' : 'button') as React.ElementType

  // Determine spinner size based on button size
  const getSpinnerSize = () => {
    if (size === 'icon-sm') return 13
    if (size === 'sm') return 14
    if (size === 'icon-navbar') return 18
    if (size === 'lg' || size === 'icon-lg') return 18
    return 16
  }

  // Default loading icon
  const defaultLoadingIcon = (
    <Loader className={cn('motion-safe:animate-spin', loadingIconClassName)} size={getSpinnerSize()} />
  )

  // Use custom icon or default icon
  const spinnerElement = loadingIcon ?? defaultLoadingIcon

  const arcVariant = arcVariants[variant ?? 'default']
  const arcSize = arcSizes[size ?? 'default']
  if (
    arcVariant &&
    arcSize &&
    !asChild &&
    to === undefined &&
    href === undefined &&
    loadingIcon === undefined &&
    loadingIconClassName === undefined
  ) {
    return (
      <ArcButton
        data-slot="button"
        data-variant={variant ?? 'default'}
        className={buttonVariants({ variant, size, className })}
        variant={arcVariant}
        size={arcSize}
        loading={loading ?? false}
        disabled={disabled}
        onClick={onClick}
        {...props}>
        {children}
      </ArcButton>
    )
  }

  return (
    <Comp
      data-slot="button"
      data-variant={variant ?? 'default'}
      className={cn(buttonVariants({ variant, size, className }))}
      disabled={to === undefined && href === undefined ? disabled || loading : undefined}
      aria-disabled={disabled || loading ? true : props['aria-disabled']}
      aria-busy={loading || undefined}
      data-busy={loading || undefined}
      {...(to !== undefined ? { to } : {})}
      {...(href !== undefined ? { href } : {})}
      {...props}
      onClickCapture={(event: React.MouseEvent<HTMLButtonElement>) => {
        if (disabled || loading || props['aria-disabled'] === true || props['aria-disabled'] === 'true') {
          event.preventDefault()
          event.stopPropagation()
          return
        }
        props.onClickCapture?.(event)
      }}
      onClick={(event: React.MouseEvent<HTMLButtonElement>) => {
        if (disabled || loading || props['aria-disabled'] === true || props['aria-disabled'] === 'true') {
          event.preventDefault()
          event.stopPropagation()
          return
        }
        onClick?.(event)
      }}>
      {/* asChild mode does not support loading because Slot requires a single child element */}
      {asChild ? (
        children
      ) : loading !== undefined ? (
        <span className="relative inline-flex items-center justify-center gap-[inherit]">
          <span
            data-slot="button-label"
            className={cn(
              'inline-flex items-center justify-center gap-[inherit] motion-safe:transition-opacity motion-safe:duration-[160ms]',
              loading && 'opacity-0'
            )}>
            {children}
          </span>
          {loading && (
            <span
              aria-hidden="true"
              data-slot="button-pending"
              className="pointer-events-none absolute inset-0 inline-flex items-center justify-center motion-safe:animate-in motion-safe:fade-in motion-safe:duration-[160ms]">
              {spinnerElement}
            </span>
          )}
        </span>
      ) : (
        <>{children}</>
      )}
    </Comp>
  )
}

type ButtonVariant = NonNullable<VariantProps<typeof buttonVariants>['variant']>
type ButtonSize = NonNullable<VariantProps<typeof buttonVariants>['size']>

/** Map the existing API onto Arc's variants and shared size scale. */
const arcVariants: Partial<Record<ButtonVariant, 'primary' | 'secondary' | 'ghost' | 'danger'>> = {
  default: 'primary',
  primary: 'primary',
  accent: 'primary',
  emphasis: 'primary',
  secondary: 'secondary',
  outline: 'secondary',
  ghost: 'ghost',
  destructive: 'danger'
}
const arcSizes: Partial<Record<ButtonSize, 'sm' | 'md' | 'lg' | 'icon' | 'icon-sm' | 'icon-lg' | 'icon-navbar'>> = {
  default: 'sm',
  sm: 'sm',
  md: 'md',
  lg: 'lg',
  icon: 'icon',
  'icon-sm': 'icon-sm',
  'icon-lg': 'icon-lg',
  'icon-navbar': 'icon-navbar'
}

export { Button, type ButtonProps, type ButtonVariant, buttonVariants }
