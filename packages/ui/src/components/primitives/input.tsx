import * as React from 'react'

import { cn } from '../../lib/utils'

import arcInput from '../arc/input/input.module.css'

type InputSize = 'sm' | 'md'

interface InputProps extends Omit<React.ComponentProps<'input'>, 'size'> {
  /** Control height: 30px (`sm`) or 36px (`md`, default). */
  size?: InputSize
  /** Show a clear affordance when the field has a value; fires onChange with an empty value. */
  allowClear?: boolean
  /** Extra callback after the built-in clear affordance empties the field. */
  onClear?: () => void
}

function Input({ className, type, size = 'md', allowClear, onClear, value, onChange, ref, ...props }: InputProps) {
  const inputRef = React.useRef<HTMLInputElement>(null)
  const [uncontrolledHasValue, setUncontrolledHasValue] = React.useState(
    () => props.defaultValue !== undefined && String(props.defaultValue) !== ''
  )
  const controlled = value !== undefined
  const hasValue = controlled ? value !== null && String(value) !== '' : uncontrolledHasValue
  React.useImperativeHandle(ref, () => inputRef.current!, [allowClear])

  React.useEffect(() => {
    const input = inputRef.current
    const form = input?.form
    if (!allowClear || controlled || !input || !form) return
    const handleReset = () => {
      queueMicrotask(() => {
        if (inputRef.current === input) setUncontrolledHasValue(input.value !== '')
      })
    }
    form.addEventListener('reset', handleReset)
    return () => form.removeEventListener('reset', handleReset)
  }, [allowClear, controlled])

  const input = (
    <input
      ref={inputRef}
      type={type}
      value={value}
      onChange={(event) => {
        if (allowClear && !controlled) setUncontrolledHasValue(event.currentTarget.value !== '')
        onChange?.(event)
      }}
      data-slot="input"
      data-size={size}
      className={cn('arc', arcInput.input, className)}
      data-has-clear={allowClear && hasValue ? 'true' : undefined}
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
          disabled={props.disabled || props.readOnly}
          aria-label="Clear"
          className="absolute right-1.5 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-md text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]"
          onClick={() => {
            const node = inputRef.current
            if (node && (!controlled || onChange)) {
              // Bypass React's value tracker so a native input event produces a real onChange.
              const view = node.ownerDocument.defaultView
              const setter = view && Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, 'value')?.set
              setter?.call(node, '')
              if (view && setter) node.dispatchEvent(new view.Event('input', { bubbles: true }))
            }
            onClear?.()
          }}>
          <svg viewBox="0 0 14 14" className="size-3" fill="none" stroke="currentColor" strokeWidth="1.7">
            <path d="M3 3l8 8M11 3l-8 8" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </span>
  )
}

export { Input, type InputProps, type InputSize }
