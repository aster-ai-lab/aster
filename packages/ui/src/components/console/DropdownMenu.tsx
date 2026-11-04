import { type ReactNode } from 'react'
import { cn } from '../../lib/cn'
import {
  DropdownMenu as PrimitiveDropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../primitives/dropdown-menu'

export type DropdownMenuItem = {
  id: string
  label: string
  icon?: ReactNode
  tone?: 'default' | 'danger'
  disabled?: boolean
  /** Visual separator before this item (org settings groups). */
  separator?: boolean
  onSelect?: () => void
}

export type DropdownMenuProps = {
  trigger: ReactNode
  items: DropdownMenuItem[]
  align?: 'start' | 'end'
  className?: string
  'aria-label'?: string
}

/**
 * Anchored item menu on the shared Arc/primitives DropdownMenu.
 * Prefer the primitives compound API for new Studio surfaces.
 */
export function DropdownMenu({
  trigger,
  items,
  align = 'end',
  className = '',
  'aria-label': ariaLabel = 'More actions',
}: DropdownMenuProps) {
  return (
    <PrimitiveDropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={ariaLabel}
          className={cn('inline-flex', className)}
          onClick={(event) => event.stopPropagation()}
        >
          {trigger}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align={align} sideOffset={4} className="min-w-[10rem]">
        {items.map((item) => (
          <div key={item.id}>
            {item.separator ? <DropdownMenuSeparator /> : null}
            <DropdownMenuItem
              disabled={item.disabled}
              variant={item.tone === 'danger' ? 'destructive' : 'default'}
              onSelect={(event) => {
                event.stopPropagation()
                item.onSelect?.()
              }}
            >
              {item.icon ? (
                <span className="inline-flex shrink-0 opacity-80">{item.icon}</span>
              ) : null}
              {item.label}
            </DropdownMenuItem>
          </div>
        ))}
      </DropdownMenuContent>
    </PrimitiveDropdownMenu>
  )
}
