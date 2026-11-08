import { motion, type HTMLMotionProps } from 'motion/react'
import { forwardRef, type ReactNode } from 'react'

import { motionTokens } from '@aster-ai-lab/ui/components/arc/lib/motion-tokens'
import { useReducedMotion } from '@aster-ai-lab/ui/lib/useReducedMotion'

export type ButtonTone = 'primary' | 'secondary' | 'quiet' | 'danger'
export type ButtonSize = 'standard' | 'compact'

type ButtonProps = Omit<HTMLMotionProps<'button'>, 'ref'> & {
  tone?: ButtonTone
  size?: ButtonSize
  loading?: boolean
  children: ReactNode
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { tone = 'secondary', size = 'standard', loading = false, children, className = '', disabled, ...props },
  ref
) {
  const reduced = useReducedMotion()
  const idle = disabled || loading
  return (
    <motion.button
      ref={ref}
      type="button"
      className={`ds-button ${tone} ${size} ${className}`}
      disabled={idle}
      aria-busy={loading || undefined}
      whileTap={idle || reduced ? undefined : { scale: 0.97 }}
      transition={reduced ? { duration: 0 } : motionTokens.spring.snappy}
      {...props}>
      {loading && <span className="ds-button-spinner" aria-hidden="true" />}
      <span className="ds-button-content">{children}</span>
    </motion.button>
  )
})
