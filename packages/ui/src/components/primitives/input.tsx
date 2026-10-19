import * as React from 'react'

import { cn } from '../../lib/utils'

type InputSize = 'sm' | 'md'

interface InputProps extends Omit<React.ComponentProps<'input'>, 'size'> {
  /** Control height: 30px search field (`sm`) or 36px field (`md`, default). */
  size?: InputSize
  /** Show a clear affordance when the field has a value; fires onChange with an empty value. */
  allowClear?: boolean
  /** Extra callback after the built-in clear affordance empties the field. */
  onClear?: () => void
}

function Input({ className, type, size = 'md', allowClear, onClear, value, onChange, ...props }: InputProps) {
  const hasValue = value !== undefined && value !== null && String(value) !== ''

  const input = (
    <input
      type={type}
      value={value}
      onChange={onChange}
      data-slot="input"
      data-size={size}
      className={cn(
        'w-full min-w-0 rounded-[8px] border border-transparent bg-[#f5f5f5] py-1 text-[13px] text-[#171717] shadow-none transition-[color,box-shadow] outline-none selection:bg-foreground selection:text-background file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-[13px] file:font-medium file:text-[#171717] placeholder:text-[#8a8a8a] disabled:pointer-events-none disabled:cursor-not-allowed dark:bg-[#2c2c2c] dark:text-[#f5f5f5]',
        size === 'sm' ? 'h-[30px] px-2.5' : 'h-9 px-3',
        allowClear && hasValue && 'pr-7',
        'focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
        'disabled:opacity-50',
        'aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40',
        className
      )}
      {...props}
    />
  )

  if (!allowClear) {
    return input
  }

  return (
    <span className="relative block w-full" data-slot="input-wrapper">
      {input}
      {hasValue && (
        <button
          type="button"
          aria-label="Clear"
          className="absolute right-1.5 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-md text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]"
          onClick={() => {
            onChange?.({ target: { value: '' } } as React.ChangeEvent<HTMLInputElement>)
            onClear?.()
          }}
        >
          <svg viewBox="0 0 14 14" className="size-3" fill="none" stroke="currentColor" strokeWidth="1.7">
            <path d="M3 3l8 8M11 3l-8 8" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </span>
  )
}

export { Input, type InputProps, type InputSize }
