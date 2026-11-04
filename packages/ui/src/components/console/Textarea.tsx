import type { TextareaHTMLAttributes } from 'react'
import { cn } from '../../lib/cn'
import { Input as PrimitiveTextarea } from '../primitives/textarea'

/**
 * Console fill textarea on the shared Arc/primitives Textarea input.
 * Not barrel-exported — product surfaces use `Textarea.Input` from `@aster-ai-lab/ui`.
 */
export function Textarea({
  className = '',
  value,
  defaultValue,
  onChange,
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <PrimitiveTextarea
      className={cn('min-h-[5.5rem] w-full resize-y', className)}
      value={typeof value === 'string' ? value : undefined}
      defaultValue={typeof defaultValue === 'string' ? defaultValue : undefined}
      onChange={onChange}
      {...props}
    />
  )
}
