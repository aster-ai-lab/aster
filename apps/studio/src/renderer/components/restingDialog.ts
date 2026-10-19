import { useEffect, useState } from 'react'

/** Matches DIALOG_UNMOUNT_DELAY_MS. Hand-drawn dialogs wait this long before leaving the tree. */
export const RESTING_DIALOG_LEAVE_MS = 200

function prefersReducedMotion() {
  return typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

/**
 * Keep a resting dialog mounted through its leave.
 * `state` is `closed` while the node is still in the tree, so the leave animation can play.
 * Reopening before the timer ends cancels the unmount.
 */
export function useRestingDialog(open: boolean): { present: boolean; state: 'open' | 'closed' } {
  const [held, setHeld] = useState(open)

  useEffect(() => {
    if (open) {
      setHeld(true)
      return
    }
    if (prefersReducedMotion()) {
      setHeld(false)
      return
    }
    const timer = window.setTimeout(() => setHeld(false), RESTING_DIALOG_LEAVE_MS)
    return () => window.clearTimeout(timer)
  }, [open])

  return { present: open || held, state: open ? 'open' : 'closed' }
}
