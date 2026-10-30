import { motion } from 'motion/react'
import type { CSSProperties } from 'react'

import { useReducedMotion } from '../../lib/useReducedMotion'
import { cn } from '../../lib/utils'

/** One plate follows the owner's selection; the control and its hit targets stay still. */
export function SelectionHighlight({
  layoutId,
  className,
  style,
  'data-slot': slot = 'selection-highlight'
}: {
  layoutId: string
  className?: string
  style?: CSSProperties
  'data-slot'?: string
}) {
  const reducedMotion = useReducedMotion()
  return (
    <motion.span
      key={reducedMotion ? 'still' : 'moving'}
      aria-hidden="true"
      data-slot={slot}
      layoutId={reducedMotion ? undefined : layoutId}
      layoutDependency={layoutId}
      initial={false}
      transition={{ type: 'spring', duration: 0.42, bounce: 0.12 }}
      className={cn('pointer-events-none absolute inset-0 -z-10', className)}
      style={{ borderRadius: 'inherit', ...style }}
    />
  )
}
