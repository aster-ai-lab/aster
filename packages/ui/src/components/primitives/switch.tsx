import * as SwitchPrimitive from '@radix-ui/react-switch'
import * as React from 'react'
import { useId } from 'react'

import arcSwitch from '../arc/switch/switch.module.css'

import { cn } from '../../lib/utils'

// Enhanced Switch component with loading state support
interface SwitchProps extends Omit<React.ComponentProps<typeof SwitchPrimitive.Root>, 'children'> {
  /** When true, displays a loading animation in the switch thumb. Defaults to false when undefined. */
  loading?: boolean
  size?: 'xs' | 'sm' | 'md' | 'lg'
  classNames?: {
    root?: string
    thumb?: string
    thumbSvg?: string
  }
  /** Inline caption. Console forms pass this; the control stays beside the text. */
  label?: string
}

function Switch({ loading = false, size = 'md', className, classNames, label, id, disabled, ...props }: SwitchProps) {
  const autoId = useId()
  const switchId = id ?? (label ? autoId : undefined)
  const control = (
    <SwitchPrimitive.Root
      id={switchId}
      data-slot="switch"
      data-size={size}
      data-loading={loading || undefined}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cn('arc', arcSwitch.track, 'group peer', className, classNames?.root)}
      {...props}>
      <SwitchPrimitive.Thumb data-slot="switch-thumb" className={cn(arcSwitch.thumb, classNames?.thumb)}>
        {loading && (
          <svg
            viewBox="0 0 20 20"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            className={cn('motion-safe:animate-spin', classNames?.thumbSvg)}>
            <circle cx="10" cy="10" r="7" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeDasharray="24 20" />
          </svg>
        )}
      </SwitchPrimitive.Thumb>
    </SwitchPrimitive.Root>
  )
  if (!label) return control
  return (
    <label
      htmlFor={switchId}
      className="inline-flex cursor-pointer items-center gap-2 text-[13px] text-[#171717] dark:text-[#f5f5f5]">
      {control}
      <span>{label}</span>
    </label>
  )
}

interface DescriptionSwitchProps extends SwitchProps {
  /** Text label displayed next to the switch. */
  label: string
  /** Optional helper text shown below the label. */
  description?: string
  /** Switch position relative to label. Defaults to 'right'. */
  position?: 'left' | 'right'
}

// TODO: It's not finished. We need to use Typography components instead of native html element.
const DescriptionSwitch = ({
  label,
  description,
  position = 'right',
  size = 'md',
  ...props
}: DescriptionSwitchProps) => {
  const isLeftSide = position === 'left'
  const id = useId()
  return (
    <div className={cn('flex w-full justify-between gap-3 p-2', isLeftSide && 'flex-row-reverse')}>
      <label className={cn('flex cursor-pointer flex-col gap-1')} htmlFor={id}>
        {/* TODO: use standard typography component */}
        <p
          className={cn(
            'font-medium tracking-normal',
            {
              'text-sm leading-4': size === 'sm',
              'text-md leading-4.5': size === 'md',
              'text-lg leading-5.5': size === 'lg'
            },
            isLeftSide && 'text-right'
          )}>
          {label}
        </p>
        {/* TODO: use standard typography component */}
        {description && (
          <span
            className={cn('text-muted-foreground', {
              'text-[10px] leading-3': size === 'sm',
              'text-xs leading-3.5': size === 'md',
              'text-sm leading-4': size === 'lg'
            })}>
            {description}
          </span>
        )}
      </label>
      <div className="flex items-center justify-center">
        <Switch id={id} size={size} {...props} />
      </div>
    </div>
  )
}

Switch.displayName = 'Switch'

export { DescriptionSwitch, Switch }
export type { SwitchProps }
