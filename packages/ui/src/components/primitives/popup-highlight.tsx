import { motion, useReducedMotion } from 'motion/react'
import { useLayoutEffect, useRef, useState } from 'react'

import '../../styles/popup-motion.css'
import '../../styles/popup-highlight.css'

import { motionTokens } from '../arc/lib/motion-tokens'

const ITEMS =
  '[role="option"], [role="menuitem"], [role="menuitemradio"], [role="menuitemcheckbox"], [data-popup-item], [data-option-id]'

type Highlight = { x: number; y: number; width: number; height: number; glide: boolean; danger: boolean }

/** One visual plate follows items without changing selection or moving their hit targets. */
export function PopupHighlight({ activeKey, visible = true }: { activeKey?: number; visible?: boolean }) {
  const ref = useRef<HTMLSpanElement>(null)
  const reduced = useReducedMotion()
  const [highlight, setHighlight] = useState<Highlight | null>(null)

  useLayoutEffect(() => {
    const root = ref.current?.closest<HTMLElement>('[data-popup-surface]')
    if (!root) return
    let current: HTMLElement | null = null
    let pointer = false
    let point: { x: number; y: number } | null = null
    let timer = 0
    let frame = 0
    const clear = () => {
      current = null
      root.removeAttribute('data-popup-highlight')
      setHighlight(null)
    }
    const show = (item: HTMLElement, glide: boolean) => {
      window.clearTimeout(timer)
      if (item.matches(':disabled, [aria-disabled="true"], [data-disabled=""], [data-disabled="true"]')) {
        clear()
        return
      }
      if (item === current) return
      const container = ref.current?.offsetParent
      if (!(container instanceof HTMLElement)) return
      current = item
      const rect = item.getBoundingClientRect()
      const parent = container.getBoundingClientRect()
      const scale = parent.width / container.offsetWidth
      root.setAttribute('data-popup-highlight', '')
      setHighlight((previous) => ({
        x: (rect.left - parent.left) / scale + container.scrollLeft - container.clientLeft,
        y: (rect.top - parent.top) / scale + container.scrollTop - container.clientTop,
        width: rect.width / scale,
        height: rect.height / scale,
        glide: glide && previous !== null,
        danger: item.dataset.variant === 'destructive' || item.dataset.tone === 'danger'
      }))
    }
    const itemFrom = (target: EventTarget | null) => {
      const item = target instanceof Element ? target.closest<HTMLElement>(ITEMS) : null
      return item?.closest('[data-popup-surface]') === root ? item : null
    }
    const update = (target: EventTarget | null) => {
      const item = itemFrom(target)
      if (item) show(item, pointer)
      else {
        window.clearTimeout(timer)
        timer = window.setTimeout(clear, pointer ? 70 : 0)
      }
    }
    const move = (event: PointerEvent) => {
      pointer = true
      point = { x: event.clientX, y: event.clientY }
      update(event.target)
    }
    const leave = () => { point = null; clear() }
    const scroll = () => {
      current = null
      const item = pointer && point ? itemFrom(root.ownerDocument.elementFromPoint(point.x, point.y)) : null
      if (item) show(item, false)
      else clear()
    }
    const focus = (event: FocusEvent) => update(event.target)
    const key = () => {
      pointer = false
      clear()
    }

    clear()
    if (!visible) return
    // Filtering can remove the hovered row without another pointer event.
    const changes = new MutationObserver(clear)
    changes.observe(root, { childList: true, subtree: true, attributes: true, attributeFilter: ['hidden'] })
    root.addEventListener('pointermove', move, true)
    root.addEventListener('focusin', focus)
    root.addEventListener('keydown', key, true)
    root.addEventListener('pointerleave', leave)
    root.addEventListener('scroll', scroll, true)
    window.addEventListener('resize', clear)
    // The quick panel keeps keyboard focus in the editor and virtualizes its rows.
    if (activeKey !== undefined) {
      frame = requestAnimationFrame(() => {
        const item = root.querySelector<HTMLElement>('[data-popup-item][aria-current="true"]')
        if (item) show(item, false)
      })
    }
    return () => {
      changes.disconnect()
      window.clearTimeout(timer)
      cancelAnimationFrame(frame)
      root.removeAttribute('data-popup-highlight')
      root.removeEventListener('pointermove', move, true)
      root.removeEventListener('focusin', focus)
      root.removeEventListener('keydown', key, true)
      root.removeEventListener('pointerleave', leave)
      root.removeEventListener('scroll', scroll, true)
      window.removeEventListener('resize', clear)
    }
  }, [activeKey, visible])

  return (
    <motion.span
      ref={ref}
      aria-hidden="true"
      data-slot="popup-highlight"
      data-tone={highlight?.danger ? 'danger' : undefined}
      initial={false}
      className="popup-highlight"
      animate={
        highlight
          ? { x: highlight.x, y: highlight.y, width: highlight.width, height: highlight.height, opacity: 1 }
          : { opacity: 0 }
      }
      transition={{
        default: reduced || !highlight?.glide ? { duration: 0 } : motionTokens.spring.morph,
        opacity: { duration: reduced || !highlight?.glide ? 0 : 0.08 }
      }}
    />
  )
}
