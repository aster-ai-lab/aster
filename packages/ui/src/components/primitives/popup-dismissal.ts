import * as React from 'react'

type PopupState = {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export const PopupDismissContext = React.createContext<PopupState | null>(null)

export function usePopupState({
  open,
  defaultOpen = false,
  onOpenChange
}: {
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
}): PopupState {
  const [internalOpen, setInternalOpen] = React.useState(defaultOpen)
  const change = React.useCallback(
    (next: boolean) => {
      if (open === undefined) setInternalOpen(next)
      onOpenChange?.(next)
    },
    [open, onOpenChange]
  )
  return { open: open ?? internalOpen, onOpenChange: change }
}

export function usePopupDismissal(forwardedRef: React.Ref<HTMLDivElement> | undefined) {
  const state = React.use(PopupDismissContext)
  const content = React.useRef<HTMLDivElement | null>(null)
  const insideReactTree = React.useRef(false)
  const pointerDismissed = React.useRef(false)
  const open = state?.open
  const onOpenChange = state?.onOpenChange
  const ref = React.useCallback(
    (node: HTMLDivElement | null) => {
      content.current = node
      if (typeof forwardedRef === 'function') {
        const cleanup = forwardedRef(node)
        if (cleanup)
          return () => {
            content.current = null
            cleanup()
          }
      } else if (forwardedRef) forwardedRef.current = node
      return undefined
    },
    [forwardedRef]
  )

  React.useEffect(() => {
    if (!open || !onOpenChange) return
    let mounted = true
    const outside = (event: Event) => {
      if (!(event.target instanceof Node) || event.target === document) {
        pointerDismissed.current = true
        onOpenChange(false)
        return
      }
      insideReactTree.current = false
      // React portals belong to their parent popup even when their DOM nodes do not.
      queueMicrotask(() => {
        const node = content.current
        if (!mounted || !node || insideReactTree.current) return
        const target = event.target
        if (target instanceof Node && node.contains(target)) return
        const trigger = target instanceof Element ? target.closest('[aria-controls]') : null
        if (node.id && trigger?.getAttribute('aria-controls') === node.id) return
        pointerDismissed.current = true
        onOpenChange(false)
      })
    }
    const blur = () => {
      pointerDismissed.current = true
      onOpenChange(false)
    }
    window.addEventListener('pointerdown', outside, true)
    window.addEventListener('click', outside, true)
    window.addEventListener('blur', blur)
    return () => {
      mounted = false
      window.removeEventListener('pointerdown', outside, true)
      window.removeEventListener('click', outside, true)
      window.removeEventListener('blur', blur)
    }
  }, [open, onOpenChange])

  return {
    ref,
    inside: () => {
      insideReactTree.current = true
    },
    onCloseAutoFocus: (event: Event) => {
      if (pointerDismissed.current) event.preventDefault()
      pointerDismissed.current = false
    }
  }
}
