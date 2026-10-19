import { throttle } from 'es-toolkit/compat'
import * as React from 'react'

// Original: src/renderer/components/scrollbar/index.tsx
import { cn } from '../../../lib/utils'

export interface ScrollbarProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onScroll'> {
  /**
   * Hide the scrollbar when scrolling stops.
   *
   * @default true
   */
  autoHideScrollbar?: boolean
  /**
   * Overlay thumb: no reserved gutter and no `scrollbar-color`.
   * `scrollbar-color` opts WebKit into the classic thick bar.
   * The host styles `[data-scrollbar='overlay']`.
   *
   * @default false
   */
  overlay?: boolean
  onScroll?: () => void
  showOnHover?: boolean
}

type OverlayThumb = {
  top: number
  height: number
}

function assignScrollerRef(ref: React.Ref<HTMLDivElement> | undefined, node: HTMLDivElement | null) {
  if (typeof ref === 'function') {
    ref(node)
    return
  }
  if (ref) ref.current = node
}

const Scrollbar = ({
  ref,
  autoHideScrollbar = true,
  children,
  className,
  onScroll: externalOnScroll,
  overlay = false,
  style,
  showOnHover = false,
  ...htmlProps
}: ScrollbarProps & { ref?: React.Ref<HTMLDivElement> }) => {
  const scrollerRef = React.useRef<HTMLDivElement | null>(null)
  const [isScrolling, setIsScrolling] = React.useState(false)
  const [hovered, setHovered] = React.useState(false)
  const [thumb, setThumb] = React.useState<OverlayThumb | null>(null)
  const timeoutRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearScrollingTimeout = React.useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
      timeoutRef.current = null
    }
  }, [])

  const handleScroll = React.useCallback(() => {
    if (!autoHideScrollbar) return

    setIsScrolling(true)
    clearScrollingTimeout()
    timeoutRef.current = setTimeout(() => {
      setIsScrolling(false)
      timeoutRef.current = null
    }, 1500)
  }, [autoHideScrollbar, clearScrollingTimeout])

  const throttledInternalScrollHandler = React.useMemo(
    () => throttle(handleScroll, 100, { leading: true, trailing: true }),
    [handleScroll]
  )

  const combinedOnScroll = React.useCallback(() => {
    throttledInternalScrollHandler()
    externalOnScroll?.()
  }, [externalOnScroll, throttledInternalScrollHandler])

  const measureThumb = React.useCallback(() => {
    const element = scrollerRef.current
    if (!overlay || !element) return
    const { clientHeight, scrollHeight, scrollTop } = element
    const overflow = scrollHeight - clientHeight
    if (overflow <= 1 || clientHeight <= 0) {
      setThumb(null)
      return
    }
    const height = Math.max(28, (clientHeight / scrollHeight) * clientHeight)
    const travel = Math.max(1, clientHeight - height)
    setThumb({ top: (scrollTop / overflow) * travel, height })
  }, [overlay])

  React.useEffect(() => {
    measureThumb()
    const element = scrollerRef.current
    if (!overlay || !element) return
    const observer = new ResizeObserver(measureThumb)
    observer.observe(element)
    return () => observer.disconnect()
  }, [measureThumb, overlay])

  React.useEffect(() => {
    return () => {
      clearScrollingTimeout()
      throttledInternalScrollHandler.cancel()
    }
  }, [clearScrollingTimeout, throttledInternalScrollHandler])

  const isScrollbarVisible = !autoHideScrollbar || isScrolling
  const useHoverVisibility = showOnHover && autoHideScrollbar
  const setNode = (node: HTMLDivElement | null) => {
    scrollerRef.current = node
    assignScrollerRef(ref, node)
  }

  const scroller = (
    <div
      {...htmlProps}
      ref={setNode}
      className={cn(
        overlay ? 'overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden' : '[scrollbar-gutter:stable] overflow-y-auto',
        !overlay &&
          useHoverVisibility &&
          '[scrollbar-color:transparent_transparent] hover:[scrollbar-color:var(--scrollbar-thumb-hover)_transparent] data-[scrolling=true]:[scrollbar-color:var(--scrollbar-thumb)_transparent]',
        !overlay &&
          !useHoverVisibility &&
          '[&::-webkit-scrollbar-thumb]:transition-[background] [&::-webkit-scrollbar-thumb]:duration-[2000ms] [&::-webkit-scrollbar-thumb:hover]:bg-[var(--scrollbar-thumb-hover)]',
        !overlay &&
          !useHoverVisibility &&
          (isScrollbarVisible
            ? '[&::-webkit-scrollbar-thumb]:bg-[var(--scrollbar-thumb)]'
            : '[&::-webkit-scrollbar-thumb]:bg-transparent'),
        className
      )}
      data-scrollbar={overlay ? 'overlay' : undefined}
      data-scrolling={isScrolling ? 'true' : 'false'}
      onMouseEnter={
        overlay
          ? () => {
              setHovered(true)
              measureThumb()
            }
          : undefined
      }
      onMouseLeave={overlay ? () => setHovered(false) : undefined}
      onScroll={() => {
        combinedOnScroll()
        measureThumb()
      }}
      style={{
        ...style,
        ...(!overlay &&
          !useHoverVisibility && {
            scrollbarColor: isScrollbarVisible ? 'var(--scrollbar-thumb) transparent' : 'transparent transparent'
          })
      }}>
      {children}
    </div>
  )

  if (!overlay) return scroller

  const thumbVisible = Boolean(thumb) && (hovered || isScrolling)

  return (
    <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
      {scroller}
      {thumb ? (
        <div
          aria-hidden="true"
          data-overlay-scrollbar-thumb=""
          className="absolute right-0.5 z-10 w-1 rounded-full"
          style={{
            top: thumb.top,
            height: thumb.height,
            background: 'rgb(23 23 23 / 32%)',
            opacity: thumbVisible ? 1 : 0,
            transition: 'opacity 160ms ease'
          }}
        />
      ) : null}
    </div>
  )
}

Scrollbar.displayName = 'Scrollbar'

export default Scrollbar
