import { composeEventHandlers } from '@radix-ui/primitive'
import { useCallbackRef } from '@radix-ui/react-use-callback-ref'
import { useControllableState } from '@radix-ui/react-use-controllable-state'
import * as React from 'react'

import arcTextarea from '../arc/textarea/textarea.module.css'

import { cn } from '../../lib/utils'

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
  'aria-invalid': ariaInvalid,
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
      aria-invalid={hasError ? true : ariaInvalid}
      className={cn('arc', arcTextarea.control, className)}
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
