import * as CheckboxPrimitive from '@radix-ui/react-checkbox'
import { cva, type VariantProps } from 'class-variance-authority'
import { CheckIcon, SquareIcon } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'

export type CheckedState = CheckboxPrimitive.CheckedState

const checkboxVariants = cva(
  cn(
    // 16px box. Radius stays 4px: 8px would be a circle, and pills are reserved for the session dot.
    'inline-flex shrink-0 items-center justify-center border p-0 leading-none outline-none',
    'aspect-square rounded-[4px] border-[#e5e5e5] bg-white text-white shadow-none',
    'hover:border-[#171717] hover:bg-white',
    'data-[state=checked]:border-[#171717] data-[state=checked]:bg-[#171717] data-[state=checked]:text-white',
    'data-[state=indeterminate]:border-[#171717] data-[state=indeterminate]:bg-[#171717] data-[state=indeterminate]:text-white',
    'hover:data-[state=checked]:border-[#171717] hover:data-[state=checked]:bg-[#171717] hover:data-[state=checked]:text-white',
    'hover:data-[state=indeterminate]:border-[#171717] hover:data-[state=indeterminate]:bg-[#171717]',
    'focus-visible:border-[#171717] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
    'aria-invalid:border-[#b42318]',
    'disabled:cursor-not-allowed disabled:opacity-40'
  ),
  {
    variants: {
      size: {
        sm: 'size-4',
        md: 'size-4',
        lg: 'size-6'
      }
    },
    defaultVariants: {
      size: 'md'
    }
  }
)

const checkboxIconVariants = cva('', {
  variants: {
    size: {
      sm: 'size-3',
      md: 'size-3',
      lg: 'size-4'
    }
  },
  defaultVariants: {
    size: 'md'
  }
})

function Checkbox({
  className,
  size = 'md',
  ...props
}: React.ComponentProps<typeof CheckboxPrimitive.Root> & VariantProps<typeof checkboxVariants>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      data-size={size}
      className={cn(checkboxVariants({ size }), className)}
      {...props}>
      <CheckboxPrimitive.Indicator
        data-slot="checkbox-indicator"
        className="group/indicator flex items-center justify-center">
        <CheckIcon
          strokeWidth={1.7}
          className={cn(
            checkboxIconVariants({ size }),
            'text-current group-data-[state=indeterminate]/indicator:hidden'
          )}
        />
        <SquareIcon
          fill="currentColor"
          strokeWidth={0}
          className={cn(
            checkboxIconVariants({ size }),
            'hidden text-current group-data-[state=indeterminate]/indicator:block'
          )}
        />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  )
}

export { Checkbox, checkboxVariants }
