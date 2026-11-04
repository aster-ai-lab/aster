import { motion } from 'motion/react'
import type { CSSProperties } from 'react'

import { motionTokens } from '../arc/lib/motion-tokens'
import { useReducedMotion } from '../../lib/useReducedMotion'
import { cn } from '../../lib/utils'

/** One plate follows the owner's selection; the control and its hit targets stay still. */
export function SelectionHighlight({
  layoutId,
  layoutDependency,
  className,
  style,
  'data-slot': slot = 'selection-highlight'
}: {
  layoutId: string
  /** Identity of the current selection — must change when the plate should move. */
  layoutDependency: string | number
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
      layoutDependency={layoutDependency}
      initial={false}
      transition={reducedMotion ? { duration: 0 } : motionTokens.spring.morph}
      className={cn('pointer-events-none absolute inset-0 -z-10', className)}
      style={{ borderRadius: 'inherit', ...style }}
    />
  )
}
