import type { ComponentType } from 'react'

import { cn } from '../../../lib/utils'
import { Button } from '../../primitives/button'

/**
 * @deprecated Presets no longer affect rendering — every preset renders the
 * same unified empty illustration, so all modules read as one product. Kept
 * for call-site compatibility only; existing `preset` usages are slated for
 * removal in a follow-up.
 */
export type EmptyStatePreset =
  | 'no-model'
  | 'no-assistant'
  | 'no-agent'
  | 'no-knowledge'
  | 'no-file'
  | 'no-note'
  | 'no-miniapp'
  | 'no-code-tool'
  | 'no-resource'
  | 'no-translate'
  | 'no-result'
  | 'no-topic'
  | 'no-session'

type EmptyStateIcon = ComponentType<{ size?: number; className?: string; strokeWidth?: number }>

/** One line-drawing family for an empty collection, knowledge and search. */
export type EmptyStateIllustration = 'inbox' | 'book' | 'search'

function EmptyIllustration({ variant, compact }: { variant: EmptyStateIllustration; compact: boolean }) {
  return (
    <svg
      width={compact ? 64 : 112}
      height={compact ? 48 : 84}
      viewBox="0 0 128 96"
      className="mb-5 max-w-full shrink-0 text-foreground"
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round">
      {variant === 'book' ? (
        <>
          <path
            opacity=".55"
            d="M64 24C48 14 32 14 16 18v58c16-4 32-4 48 6 16-10 32-10 48-6V18c-16-4-32-4-48 6Zm0 0v58"
          />
          <path opacity=".2" d="m28 31 22 4m-22 9 22 4m-22 9 22 4m28-26 22-4m-22 17 22-4m-22 17 22-4" />
        </>
      ) : (
        <>
          <rect x="15" y="10" width="96" height="74" rx="12" opacity=".55" />
          <path opacity=".2" d="M30 29h65M30 45h65M30 61h42" />
          {variant === 'search' ? (
            <>
              <circle cx="91" cy="62" r="19" className="fill-background" stroke="none" />
              <circle cx="91" cy="62" r="19" opacity=".75" />
              <path d="m105 76 16 16" opacity=".75" />
              <path d="M81 59a11 11 0 0 1 7-7" opacity=".3" />
            </>
          ) : (
            <>
              <path className="fill-background" d="M49 75h30v18H49z" stroke="none" />
              <path d="M54 82h20M64 72v20" opacity=".55" />
            </>
          )}
        </>
      )}
    </svg>
  )
}

export interface EmptyStateProps {
  /** @deprecated No-op — the unified illustration renders regardless. Slated for removal. */
  preset?: EmptyStatePreset
  /** Which unified illustration to render (ignored when `icon` is set). */
  illustration?: EmptyStateIllustration
  icon?: EmptyStateIcon
  title?: string
  description?: string
  actionLabel?: string
  onAction?: () => void
  secondaryLabel?: string
  onSecondary?: () => void
  compact?: boolean
  className?: string
}

export function EmptyState({
  illustration = 'inbox',
  icon: IconOverride,
  title,
  description,
  actionLabel,
  onAction,
  secondaryLabel,
  onSecondary,
  compact = false,
  className
}: EmptyStateProps) {
  const Icon = IconOverride
  const hasActions = Boolean((actionLabel && onAction) || (secondaryLabel && onSecondary))

  return (
    <div
      data-slot="empty-state"
      className={cn(
        'aster-row-enter flex min-w-0 flex-col items-center justify-center text-center [overflow-wrap:anywhere]',
        compact ? 'px-4 py-8' : 'flex-1 px-6 py-10',
        className
      )}>
      {Icon ? (
        <Icon aria-hidden="true" size={compact ? 28 : 40} strokeWidth={1.7} className="mb-5 text-muted-foreground" />
      ) : (
        <EmptyIllustration variant={illustration} compact={compact} />
      )}
      {title && (
        <h3
          className={cn(
            'm-0 max-w-lg text-balance font-medium text-foreground',
            compact ? 'text-sm leading-5' : 'text-lg leading-6',
            description ? 'mb-2' : hasActions ? (compact ? 'mb-3' : 'mb-5') : ''
          )}>
          {title}
        </h3>
      )}
      {description && (
        <p
          className={cn(
            'm-0 text-pretty text-foreground/70',
            compact ? 'mb-4 max-w-xs text-xs leading-5' : 'mb-5 max-w-md text-sm leading-6'
          )}>
          {description}
        </p>
      )}
      {hasActions && (
        <div className="flex max-w-full flex-wrap items-center justify-center gap-2">
          {actionLabel && onAction && (
            <Button variant="default" size="sm" className="max-w-full whitespace-normal" onClick={onAction}>
              {actionLabel}
            </Button>
          )}
          {secondaryLabel && onSecondary && (
            <Button variant="ghost" size="sm" onClick={onSecondary}>
              {secondaryLabel}
            </Button>
          )}
        </div>
      )}
    </div>
  )
}
