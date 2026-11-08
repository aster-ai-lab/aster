import { motion, type HTMLMotionProps } from 'motion/react'
import { forwardRef, type ReactNode } from 'react'

import { motionTokens } from '@aster-ai-lab/ui/components/arc/lib/motion-tokens'
import { useReducedMotion } from '@aster-ai-lab/ui/lib/useReducedMotion'

import { OfficeTooltip } from './office-tooltip'

type IconButtonProps = Omit<HTMLMotionProps<'button'>, 'ref' | 'aria-label'> & {
  label: string
  tooltip?: string
  selected?: boolean
  children: ReactNode
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, tooltip = label, selected, children, className = '', disabled, ...props },
  ref
) {
  const reduced = useReducedMotion()
  const button = (
    <motion.button
      ref={ref}
      type="button"
      className={`ds-icon-button ${selected ? 'selected' : ''} ${className}`}
      aria-label={label}
      aria-pressed={selected}
      title={tooltip}
      disabled={disabled}
      whileTap={disabled || reduced ? undefined : { scale: 0.96 }}
      transition={reduced ? { duration: 0 } : motionTokens.spring.snappy}
      {...props}>
      {children}
    </motion.button>
  )
  if (!tooltip.trim()) return button
  return <OfficeTooltip label={tooltip}>{button}</OfficeTooltip>
})
