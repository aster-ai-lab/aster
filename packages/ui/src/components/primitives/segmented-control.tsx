import * as React from 'react'

import { cn } from '../../lib/utils'

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
  const selectedValue = value ?? internalValue

  const handleSelect = (option: SegmentedControlOption<TValue>) => {
    if (disabled || option.disabled || option.value === selectedValue) {
      return
    }

    if (value === undefined) {
      setInternalValue(option.value)
    }
    onValueChange?.(option.value)
  }

  return (
    <div
      role="radiogroup"
      data-slot="segmented-control"
      data-size={size}
      aria-disabled={disabled}
      className={cn(
        'inline-flex items-center rounded-[8px] border-0 bg-[#f5f5f5] p-1 dark:bg-[#2c2c2c]',
        disabled && 'pointer-events-none opacity-50',
        className
      )}
      {...props}>
      {options.map((option) => {
        const selected = option.value === selectedValue

        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={disabled || option.disabled}
            onClick={() => handleSelect(option)}
            className={cn(
              'inline-flex shrink-0 items-center justify-center rounded-[6px] font-medium whitespace-nowrap transition-[background-color,color,box-shadow] outline-none',
              'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:pointer-events-none disabled:opacity-50',
              size === 'sm' ? 'h-[22px] gap-1.5 px-3 text-[13px]' : 'h-[22px] gap-2 px-3 text-[13px]',
              selected
                ? 'bg-white text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.06)] dark:bg-[#3a3a3a] dark:text-[#f5f5f5]'
                : 'text-[#6f6f6f] hover:text-[#171717] dark:text-[#9a9a9a] dark:hover:text-[#f5f5f5]'
            )}>
            {option.label}
          </button>
        )
      })}
    </div>
  )
}

export { SegmentedControl }
