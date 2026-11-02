import { Search, X } from 'lucide-react'
import type * as React from 'react'

import type { InputProps } from '../../primitives/input'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput
} from '../../primitives/input-group'
import { cn } from '../../../lib/utils'

type SearchInputClearProps =
  | {
      /**
       * Clear handler. When provided, a clear button appears while the
       * controlled input holds a non-empty value. Clicking it invokes this
       * callback; the caller owns resetting `value`.
       */
      onClear: () => void
      /** Accessible label for the clear button. Pass an i18n string from the caller. */
      clearLabel: string
    }
  | {
      onClear?: undefined
      clearLabel?: never
    }

export type SearchInputProps = Omit<InputProps, 'type' | 'size'> &
  SearchInputClearProps & {
    /** Field height, forwarded to the underlying `InputGroup` (`default` = 36px, `sm` = 30px search). */
    size?: React.ComponentProps<typeof InputGroup>['size']
    /** Classes for the wrapping input group; merged after size variants so a height override like `h-8` wins. */
    containerClassName?: string
  }

/**
 * Search field built on `InputGroup`: a leading search icon, a text input, and
 * an optional trailing clear button. Controlled via `value` / `onChange`.
 */
function SearchInput({
  className,
  value,
  disabled,
  onClear,
  clearLabel,
  size,
  containerClassName,
  ...props
}: SearchInputProps) {
  const hasValue = value !== undefined && value !== null && String(value).length > 0
  const showClear = onClear !== undefined && clearLabel !== undefined && hasValue

  return (
    <InputGroup
      size={size}
      className={cn('has-[[data-slot=input-group-control]:focus-visible]:ring-0', containerClassName)}
      data-disabled={disabled ? 'true' : undefined}>
      <InputGroupAddon>
        <Search />
      </InputGroupAddon>
      <InputGroupInput
        type="search"
        value={value}
        disabled={disabled}
        // A WebKit `type=search` paints its own native field on top of ours:
        // white fill, a 1px outline and the platform accent ring on focus, all
        // inside the group's rounded track. The design's search field is one
        // borderless track, so the native paint is turned off here.
        className={cn(
          'appearance-none [&::-webkit-search-cancel-button]:hidden [&::-webkit-search-decoration]:hidden',
          className
        )}
        {...props}
      />
      {showClear && (
        <InputGroupAddon align="inline-end">
          <InputGroupButton type="button" size="icon-xs" aria-label={clearLabel} disabled={disabled} onClick={onClear}>
            <X className="size-3.5" />
          </InputGroupButton>
        </InputGroupAddon>
      )}
    </InputGroup>
  )
}

export { SearchInput }
