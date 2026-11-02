import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { Loader } from 'lucide-react'
import * as React from 'react'
import { Link } from 'react-router-dom'

import { Button as ArcButton } from '../arc/button/button'

import { cn } from '../../lib/utils'

const buttonVariants = cva(
  cn(
    'inline-flex items-center justify-center gap-2 whitespace-nowrap',
    'rounded-md font-normal motion-safe:transition-[background-color,color,border-color,box-shadow,opacity] motion-safe:duration-150 active:opacity-80',
    'disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground',
    "shrink-0 outline-none aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&_.lucide:not(.lucide-custom)]:text-current [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
    'data-[busy=true]:cursor-progress data-[busy=true]:opacity-100',
    'shadow-none'
  ),
  {
    variants: {
      variant: {
        default:
          'bg-neutral-900 text-white hover:bg-neutral-800 focus-visible:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 dark:focus-visible:bg-neutral-200',
        /** Console-kit aliases of the filled default action. */
        primary:
          'bg-neutral-900 text-white hover:bg-neutral-800 focus-visible:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 dark:focus-visible:bg-neutral-200',
        accent:
          'bg-neutral-900 text-white hover:bg-neutral-800 focus-visible:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 dark:focus-visible:bg-neutral-200',
        destructive: 'bg-destructive text-white hover:bg-destructive-hover focus-visible:bg-destructive-hover',
        outline:
          'border border-[#e5e5e5] bg-white text-[#171717] shadow-none hover:bg-[#fafafa] focus-visible:border-[#e5e5e5] focus-visible:bg-[#fafafa] dark:border-[#2f2f33] dark:bg-[#171717] dark:text-[#f5f5f5] dark:hover:bg-[#232326] dark:focus-visible:border-[#2f2f33] dark:focus-visible:bg-[#232326]',
        secondary:
          'bg-[#f5f5f5] text-[#171717] shadow-none hover:bg-[#ececec] focus-visible:bg-[#ececec] dark:bg-[#2c2c2c] dark:text-[#f5f5f5] dark:hover:bg-[#343434] dark:focus-visible:bg-[#343434]',
        /** Dialog primary action style: same color hierarchy as default, with the 8px control radius. */
        emphasis:
          'bg-neutral-900 text-white shadow-none hover:bg-neutral-800 focus-visible:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 dark:focus-visible:bg-neutral-200',
        ghost:
          'text-[#171717] shadow-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:bg-[#f5f5f5] focus-visible:text-[#171717] dark:text-[#f5f5f5] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5] dark:focus-visible:bg-[#2c2c2c] dark:focus-visible:text-[#f5f5f5]',
        link: 'text-neutral-900 underline-offset-4 hover:text-neutral-700 hover:underline focus-visible:text-neutral-700 focus-visible:underline dark:text-neutral-100 dark:hover:text-neutral-300 dark:focus-visible:text-neutral-300'
      },
      size: {
        default: 'min-h-7.5 gap-1.5 px-2.5 text-[13px]',
        /** 36px confirm height with 13px label (DESIGN.md §6 filled button). */
        md: 'min-h-9 gap-2 px-3.5 text-[13px]',
        /** 30px quiet height, 13px label (DESIGN.md §6). */
        sm: 'min-h-7.5 gap-1.5 px-2.5 text-[13px]',
        lg: 'min-h-9 px-4 text-sm',
        icon: 'size-9',
        'icon-sm': 'size-7',
        'icon-lg': 'size-10',
        /** Navbar / toolbar icon button: 30px box, 18px icon (8px radius from base). */
        'icon-navbar': 'size-[30px] [&_svg]:!size-[18px]'
      }
    },
    defaultVariants: {
      variant: 'default',
      size: 'default'
    }
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
  if (arcVariant && arcSize && !asChild && to === undefined && href === undefined && loadingIcon === undefined && loadingIconClassName === undefined) {
    return (
      <ArcButton
        data-slot="button"
        data-variant={variant ?? 'default'}
        className={className}
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
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      data-busy={loading || undefined}
      {...(to !== undefined ? { to } : {})}
      {...(href !== undefined ? { href } : {})}
      {...props}
      onClick={(event: React.MouseEvent<HTMLButtonElement>) => {
        if (!asChild && to === undefined && href === undefined && (props['aria-disabled'] === true || props['aria-disabled'] === 'true')) {
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
              'inline-flex items-center justify-center gap-[inherit] motion-safe:transition-opacity motion-safe:duration-150',
              loading && 'opacity-0'
            )}>
            {children}
          </span>
          {loading && (
            <span
              aria-hidden="true"
              data-slot="button-pending"
              className="pointer-events-none absolute inset-0 inline-flex items-center justify-center motion-safe:animate-in motion-safe:fade-in motion-safe:duration-150">
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
