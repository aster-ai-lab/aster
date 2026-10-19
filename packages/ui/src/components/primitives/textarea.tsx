import { composeEventHandlers } from '@radix-ui/primitive'
import { useCallbackRef } from '@radix-ui/react-use-callback-ref'
import { useControllableState } from '@radix-ui/react-use-controllable-state'
import { cva } from 'class-variance-authority'
import * as React from 'react'

import { cn } from '../../lib/utils'

/* -------------------------------------------------------------------------------------------------
 * Variants
 * -----------------------------------------------------------------------------------------------*/

const textareaVariants = cva(
  cn(
    'flex field-sizing-content min-h-16 w-full resize-y rounded-[8px] border border-transparent bg-[#f5f5f5] px-3 py-2 text-[13px] text-[#171717] shadow-none transition-[color,box-shadow] outline-none',
    'placeholder:text-[#8a8a8a] dark:bg-[#2c2c2c] dark:text-[#f5f5f5]',
    'focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
    'disabled:cursor-not-allowed disabled:opacity-50'
  ),
  {
    variants: {
      hasError: {
        true: 'aria-invalid:border-destructive aria-invalid:ring-destructive/20',
        false: ''
      }
    },
    defaultVariants: {
      hasError: false
    }
  }
)

/* -------------------------------------------------------------------------------------------------
 * TextareaInput
 * -----------------------------------------------------------------------------------------------*/

const INPUT_NAME = 'TextareaInput'

interface TextareaInputProps extends Omit<React.ComponentPropsWithoutRef<'textarea'>, 'value' | 'defaultValue'> {
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  hasError?: boolean
  ref?: React.Ref<HTMLTextAreaElement>
}

function TextareaInput({
  value: valueProp,
  defaultValue,
  onValueChange,
  hasError = false,
  className,
  ref,
  ...props
}: TextareaInputProps) {
  const [value, setValue] = useControllableState({
    prop: valueProp,
    defaultProp: defaultValue ?? '',
    onChange: onValueChange
  })

  const handleChange = useCallbackRef((event: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = event.target.value
    if (props.maxLength && newValue.length > props.maxLength) {
      return
    }
    setValue(newValue)
  })

  return (
    <textarea
      data-slot="textarea-input"
      {...props}
      ref={ref}
      value={value}
      onChange={composeEventHandlers(props.onChange, handleChange)}
      aria-invalid={hasError}
      className={cn(textareaVariants({ hasError }), className)}
    />
  )
}

TextareaInput.displayName = INPUT_NAME

/* -------------------------------------------------------------------------------------------------
 * TextareaCharCount
 * -----------------------------------------------------------------------------------------------*/

const CHAR_COUNT_NAME = 'TextareaCharCount'

interface TextareaCharCountProps extends React.ComponentPropsWithoutRef<'div'> {
  value?: string
  maxLength?: number
}

function TextareaCharCount({ value = '', maxLength, className, ...props }: TextareaCharCountProps) {
  return (
    <div
      data-slot="textarea-char-count"
      {...props}
      className={cn('absolute right-2 bottom-2 text-xs text-muted-foreground', className)}>
      {value.length}/{maxLength}
    </div>
  )
}

TextareaCharCount.displayName = CHAR_COUNT_NAME

/* ---------------------------------------------------------------------------------------------- */

const Input = TextareaInput
const CharCount = TextareaCharCount

export { CharCount, Input }
export type { TextareaCharCountProps, TextareaInputProps }
