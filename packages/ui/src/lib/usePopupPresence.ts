import { useEffect, useState } from 'react'

/** Keep the last content through the shared 200ms exit; reopening cancels removal. */
export function usePopupPresence<T>(value: T | null): { present: T | null; state: 'open' | 'closed' } {
  const [held, setHeld] = useState(value)
  useEffect(() => {
    if (value !== null) {
      setHeld(value)
      return
    }
    if (held === null) return
    if (
      window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ||
      document.documentElement.hasAttribute('data-motion-paused')
    ) {
      setHeld(null)
      return
    }
    const timer = window.setTimeout(() => setHeld(null), 200)
    return () => window.clearTimeout(timer)
  }, [value, held])
  return { present: value ?? held, state: value === null ? 'closed' : 'open' }
}
