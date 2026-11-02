import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import * as React from 'react'

import arcBadge from '../arc/badge/badge.module.css'

import { cn } from '../../lib/utils'

/** Legacy class recipe, kept for callers that style custom plates; the Badge itself now wears Arc styles. */
const badgeVariants = cva(
  'inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-[6px] border px-2 py-0.5 text-xs font-medium whitespace-nowrap transition-[color,box-shadow] focus-visible:border-border aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&>svg]:pointer-events-none [&>svg]:size-3',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-[#f5f5f5] text-[#171717] dark:bg-[#2c2c2c] dark:text-[#f5f5f5] [a&]:hover:bg-[#ececec] dark:[a&]:hover:bg-[#343434]',
        secondary: 'border-transparent bg-secondary text-secondary-foreground [a&]:hover:bg-secondary/90',
        primary:
          'border-transparent bg-neutral-900 text-white dark:bg-neutral-100 dark:text-neutral-900 [a&]:hover:bg-neutral-800 dark:[a&]:hover:bg-neutral-200',
        success: 'border-transparent bg-success-subtle text-success-subtle-foreground',
        warn: 'border-transparent bg-warning-subtle text-warning-subtle-foreground',
        destructive:
          'border-transparent bg-[red]/10 text-destructive focus-visible:border-destructive dark:bg-destructive/60 [a&]:hover:bg-destructive/90',
        outline: 'text-foreground [a&]:hover:bg-accent [a&]:hover:text-accent-foreground'
      }
    },
    defaultVariants: {
      variant: 'default'
    }
  }
)

/** Feedback tone names shared with the console kit. `neutral` maps to the default plate. */
export type BadgeTone = 'neutral' | 'primary' | 'success' | 'warn' | 'danger'

const toneToVariant: Record<BadgeTone, NonNullable<VariantProps<typeof badgeVariants>['variant']>> = {
  neutral: 'default',
  primary: 'primary',
  success: 'success',
  warn: 'warn',
  danger: 'destructive'
}

/** Map the console-kit variants onto Arc's tinted badge tones. */
const variantToArc: Record<NonNullable<VariantProps<typeof badgeVariants>['variant']>, string> = {
  default: arcBadge.neutral,
  secondary: arcBadge.neutral,
  primary: arcBadge.accent,
  success: arcBadge.success,
  warn: arcBadge.warning,
  destructive: arcBadge.danger,
  outline: arcBadge.outline
}

function Badge({
  className,
  variant,
  tone,
  asChild = false,
  ...props
}: React.ComponentProps<'span'> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean; tone?: BadgeTone }) {
  const Comp = asChild ? Slot : 'span'
  const resolvedVariant = tone ? toneToVariant[tone] : variant
  const arcTone = variantToArc[resolvedVariant ?? 'default']
  const isOutline = resolvedVariant === 'outline'

  return (
    <Comp
      data-slot="badge"
      data-variant={resolvedVariant ?? 'default'}
      data-tone={tone}
      className={cn(
        'arc',
        arcBadge.badge,
        arcBadge.sm,
        arcTone,
        isOutline && arcBadge.neutral,
        '[&_svg]:size-3 [&_svg]:shrink-0',
        className
      )}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
