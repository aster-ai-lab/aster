import * as React from 'react'

import { cn } from '../../lib/utils'
import { SelectionHighlight } from './selection-highlight'

export interface SegmentedControlOption<TValue extends string = string> {
  value: TValue
  label: React.ReactNode
  disabled?: boolean
}

export interface SegmentedControlProps<TValue extends string = string> extends Omit<
  React.ComponentPropsWithoutRef<'div'>,
  'defaultValue' | 'onChange'
> {
  options: readonly SegmentedControlOption<TValue>[]
  value?: TValue
  defaultValue?: TValue
  onValueChange?: (value: TValue) => void
  disabled?: boolean
  size?: 'sm' | 'default'
}

function SegmentedControl<TValue extends string = string>({
  options,
  value,
  defaultValue,
  onValueChange,
  disabled = false,
  size = 'default',
  className,
  ...props
}: SegmentedControlProps<TValue>) {
  const [internalValue, setInternalValue] = React.useState<TValue | undefined>(defaultValue ?? options[0]?.value)
  const highlightId = React.useId()
  const buttons = React.useRef(new Map<TValue, HTMLButtonElement>())
  const selectedValue = value ?? internalValue
  const enabledOptions = options.filter((option) => !option.disabled)
  const focusValue = enabledOptions.find((option) => option.value === selectedValue)?.value ?? enabledOptions[0]?.value

  const handleSelect = (option: SegmentedControlOption<TValue>) => {
    if (disabled || option.disabled || option.value === selectedValue) {
      return
    }

    if (value === undefined) {
      setInternalValue(option.value)
    }
    onValueChange?.(option.value)
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, option: SegmentedControlOption<TValue>) => {
    if (disabled || option.disabled || !enabledOptions.length) return
    const index = enabledOptions.findIndex((item) => item.value === option.value)
    const rtl = getComputedStyle(event.currentTarget).direction === 'rtl'
    let next: number
    switch (event.key) {
      case 'Home':
        next = 0
        break
      case 'End':
        next = enabledOptions.length - 1
        break
      case 'ArrowRight':
        next = index + (rtl ? -1 : 1)
        break
      case 'ArrowLeft':
        next = index + (rtl ? 1 : -1)
        break
      case 'ArrowDown':
        next = index + 1
        break
      case 'ArrowUp':
        next = index - 1
        break
      default:
        return
    }
    event.preventDefault()
    const target = enabledOptions[(next + enabledOptions.length) % enabledOptions.length]
    buttons.current.get(target.value)?.focus()
    handleSelect(target)
  }

  return (
    <div
      role="radiogroup"
      data-slot="segmented-control"
      data-size={size}
      aria-disabled={disabled}
      className={cn(
        'relative isolate inline-flex items-center rounded-[8px] border-0 bg-[#f5f5f5] p-1 dark:bg-[#2c2c2c]',
        disabled && 'pointer-events-none opacity-50',
        className
      )}
      {...props}>
      {options.map((option) => {
        const selected = option.value === selectedValue

        return (
          <button
            key={option.value}
            ref={(element) => {
              if (element) buttons.current.set(option.value, element)
              else buttons.current.delete(option.value)
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={disabled || option.disabled}
            tabIndex={option.value === focusValue ? 0 : -1}
            onClick={() => handleSelect(option)}
            onKeyDown={(event) => handleKeyDown(event, option)}
            className={cn(
              'relative inline-flex shrink-0 items-center justify-center rounded-[6px] font-medium whitespace-nowrap outline-none motion-safe:transition-colors motion-safe:duration-150',
              'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:pointer-events-none disabled:opacity-50',
              size === 'sm' ? 'h-[22px] gap-1.5 px-3 text-[13px]' : 'h-[22px] gap-2 px-3 text-[13px]',
              selected
                ? 'text-[#171717] dark:text-[#f5f5f5]'
                : 'text-[#6f6f6f] hover:text-[#171717] dark:text-[#9a9a9a] dark:hover:text-[#f5f5f5]'
            )}>
            {selected && (
              <SelectionHighlight
                data-slot="segmented-control-indicator"
                layoutId={highlightId}
                className="bg-white dark:bg-[#3a3a3a]"
                style={{ borderRadius: 6, boxShadow: '0 1px 2px rgba(0, 0, 0, 0.06)' }}
              />
            )}
            <span className="relative z-10 inline-flex items-center gap-[inherit]">{option.label}</span>
          </button>
        )
      })}
    </div>
  )
}

export { SegmentedControl }
