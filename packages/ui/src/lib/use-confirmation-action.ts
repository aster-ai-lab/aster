import { useEffect, useRef, useState } from 'react'

/** One confirmation generation owns its result. A replaced or closed dialog cannot be settled by an old request. */
export function useConfirmationAction({
  open,
  operationKey,
  loading,
  onConfirm,
  onClose
}: {
  open: boolean | undefined
  operationKey?: string
  loading: boolean
  onConfirm?: () => boolean | void | Promise<boolean | void>
  onClose: () => void
}) {
  const generation = useRef(0)
  const pending = useRef(false)
  const [working, setWorking] = useState(false)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    generation.current += 1
    pending.current = false
    setWorking(false)
    setFailed(false)
    return () => {
      generation.current += 1
    }
  }, [open, operationKey])

  const submit = async () => {
    if (pending.current || loading || open === false) return
    const current = generation.current
    pending.current = true
    setWorking(true)
    setFailed(false)
    try {
      const result = await onConfirm?.()
      if (generation.current === current && result !== false) onClose()
    } catch {
      if (generation.current === current) setFailed(true)
    } finally {
      if (generation.current === current) {
        pending.current = false
        setWorking(false)
      }
    }
  }
  return { submit, failed, busy: loading || working }
}
