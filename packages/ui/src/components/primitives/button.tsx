import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { Loader } from 'lucide-react'
import * as React from 'react'
import { Link } from 'react-router-dom'

import { cn } from '../../lib/utils'

const buttonVariants = cva(
  cn(
    'inline-flex items-center justify-center gap-2 whitespace-nowrap',
    'rounded-md font-normal transition-all',
    'disabled:pointer-events-none disabled:opacity-40',
    "shrink-0 outline-none aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&_.lucide:not(.lucide-custom)]:text-current [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
    'data-[busy=true]:cursor-progress data-[busy=true]:opacity-40',
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
  loading = false,
  loadingIcon,
  loadingIconClassName,
  disabled,
  children,
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
  const defaultLoadingIcon = <Loader className={cn('animate-spin', loadingIconClassName)} size={getSpinnerSize()} />

  // Use custom icon or default icon
  const spinnerElement = loadingIcon ?? defaultLoadingIcon

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
      {...props}>
      {/* asChild mode does not support loading because Slot requires a single child element */}
      {asChild ? (
        children
      ) : (
        <>
          {loading && spinnerElement}
          {children}
        </>
      )}
    </Comp>
  )
}

type ButtonVariant = NonNullable<VariantProps<typeof buttonVariants>['variant']>

export { Button, type ButtonProps, type ButtonVariant, buttonVariants }
