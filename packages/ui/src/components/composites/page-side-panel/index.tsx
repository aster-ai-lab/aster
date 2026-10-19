import { XIcon } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import * as React from 'react'
import { useCallback, useEffect, useId, useRef } from 'react'
import { createPortal } from 'react-dom'

import { Button } from '../../primitives/button'
import { usePortalContainer } from '../../primitives/portal-container'
import { cn } from '../../../lib/utils'

import Scrollbar from '../scrollbar'

/**
 * A page-owned floating side panel. It portals into the nearest portal container
 * provided by `PortalContainerProvider` (e.g. a route tab root) when present,
 * otherwise to `document.body`. Scoped to a container it positions `absolute`
 * within it; at `document.body` it stays `fixed` to the viewport.
 *
 * For edge-attached modal sheets, use the shadcn `Drawer` primitive instead.
 */
const PANEL_EASE_IN = [0.16, 1, 0.3, 1] as const
const PANEL_EASE_OUT = [0.4, 0, 1, 1] as const

function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

function sidePanelMotion(reduced: boolean) {
  if (reduced) {
    return {
      open: { opacity: 1, scale: 1, transition: { duration: 0 } },
      closed: { opacity: 1, scale: 1, transition: { duration: 0 } }
    }
  }
  return {
    open: { opacity: 1, scale: 1, transition: { duration: 0.16, ease: PANEL_EASE_IN } },
    closed: { opacity: 0, scale: 0.98, transition: { duration: 0.2, ease: PANEL_EASE_OUT } }
  }
}

function scrimMotion(reduced: boolean) {
  if (reduced) {
    return {
      open: { opacity: 1, transition: { duration: 0 } },
      closed: { opacity: 1, transition: { duration: 0 } }
    }
  }
  return {
    open: { opacity: 1, transition: { duration: 0.16, ease: PANEL_EASE_IN } },
    closed: { opacity: 0, transition: { duration: 0.2, ease: PANEL_EASE_OUT } }
  }
}

type PageSidePanelPlacement = 'left' | 'right'

interface PageSidePanelProps {
  open: boolean
  onClose: () => void
  children?: React.ReactNode
  title?: React.ReactNode
  header?: React.ReactNode
  footer?: React.ReactNode
  side?: PageSidePanelPlacement
  showCloseButton?: boolean
  closeLabel?: string
  backdropClassName?: string
  contentClassName?: string
  headerClassName?: string
  bodyClassName?: string
  footerClassName?: string
  closeButtonClassName?: string
}

function PageSidePanel({
  open,
  onClose,
  children,
  title,
  header,
  footer,
  side = 'right',
  showCloseButton = true,
  closeLabel = 'Close',
  backdropClassName,
  contentClassName,
  headerClassName,
  bodyClassName,
  footerClassName,
  closeButtonClassName
}: PageSidePanelProps) {
  const standardTitle = title ? <span className="text-base font-semibold text-foreground">{title}</span> : null
  const headerContent = header ?? standardTitle
  const hasHeader = !!headerContent || showCloseButton
  const headerId = useId()
  const panelRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLElement | null>(null)
  const closedByPointerDownRef = useRef(false)
  const scopedContainer = usePortalContainer()
  const portalContainer = scopedContainer ?? (typeof document === 'undefined' ? null : document.body)
  const isScopedPortal =
    typeof document !== 'undefined' && portalContainer !== null && portalContainer !== document.body
  const reducedMotion = prefersReducedMotion()
  const panelMotion = sidePanelMotion(reducedMotion)
  const backdropMotion = scrimMotion(reducedMotion)

  const handleClose = useCallback(
    (event?: React.MouseEvent | React.PointerEvent | React.KeyboardEvent) => {
      event?.preventDefault()
      event?.stopPropagation()
      onClose()
    },
    [onClose]
  )

  useEffect(() => {
    if (open) {
      closedByPointerDownRef.current = false
      triggerRef.current = document.activeElement as HTMLElement | null
      requestAnimationFrame(() => {
        panelRef.current?.focus()
      })
    } else {
      triggerRef.current?.focus()
      triggerRef.current = null
    }
  }, [open])

  const panel = (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            key="backdrop"
            variants={backdropMotion}
            initial="closed"
            animate="open"
            exit="closed"
            data-slot="page-side-panel-backdrop"
            className={cn(isScopedPortal ? 'absolute inset-0' : 'fixed inset-0', 'z-60 bg-black/50', backdropClassName)}
            onClick={handleClose}
          />
          <motion.aside
            ref={panelRef}
            key="panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby={headerContent ? headerId : undefined}
            tabIndex={-1}
            onKeyDown={(e) => {
              if (e.key === 'Escape') handleClose(e)
            }}
            variants={panelMotion}
            initial="closed"
            animate="open"
            exit="closed"
            data-slot="page-side-panel"
            className={cn(
              isScopedPortal ? 'absolute' : 'fixed',
              'top-3 bottom-3 z-70 flex w-100 flex-col overflow-hidden rounded-3xl bg-card text-card-foreground shadow-xl outline-none',
              side === 'right' ? 'right-3' : 'left-3',
              contentClassName
            )}>
            {hasHeader && (
              <div
                data-slot="page-side-panel-header"
                className={cn('flex shrink-0 items-center justify-between px-6 pt-6 pb-3', headerClassName)}>
                <div id={headerContent ? headerId : undefined} className="flex min-w-0 flex-1 items-center">
                  {headerContent}
                </div>
                {showCloseButton && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onPointerDown={(event) => {
                      closedByPointerDownRef.current = true
                      handleClose(event)
                    }}
                    onClick={(event) => {
                      if (closedByPointerDownRef.current) {
                        closedByPointerDownRef.current = false
                        event.preventDefault()
                        event.stopPropagation()
                        return
                      }
                      handleClose(event)
                    }}
                    aria-label={closeLabel}
                    data-slot="page-side-panel-close"
                    className={cn(
                      'ml-3 size-6 shrink-0 appearance-none rounded-md p-0 text-[#8a8a8a] shadow-none transition-colors hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:bg-[#f2f3f5] focus-visible:text-[#171717] dark:text-[#a3a3a3] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5] dark:focus-visible:bg-[#2c2c2c] dark:focus-visible:text-[#f5f5f5]',
                      closeButtonClassName
                    )}>
                    <XIcon size={16} strokeWidth={1.7} />
                  </Button>
                )}
              </div>
            )}

            <Scrollbar
              data-slot="page-side-panel-body"
              className={cn('min-h-0 flex-1 space-y-4 px-6 py-4', bodyClassName)}>
              {children}
            </Scrollbar>

            {footer && (
              <div
                data-slot="page-side-panel-footer"
                className={cn('shrink-0 space-y-2.5 px-6 pt-3 pb-6', footerClassName)}>
                {footer}
              </div>
            )}
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  )

  return portalContainer ? createPortal(panel, portalContainer) : panel
}

interface PageSidePanelSectionProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  title: React.ReactNode
  actions?: React.ReactNode
  children: React.ReactNode
}

function PageSidePanelSection({ title, actions, children, className, ...props }: PageSidePanelSectionProps) {
  return (
    <div className={cn('flex flex-col gap-3', className)} {...props}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-semibold text-foreground">{title}</span>
        {actions && <div className="flex shrink-0 items-center gap-1">{actions}</div>}
      </div>
      {children}
    </div>
  )
}

interface PageSidePanelItemProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  title: React.ReactNode
  description?: React.ReactNode
  action?: React.ReactNode
  children?: React.ReactNode
}

function PageSidePanelItem({ title, description, action, children, className, ...props }: PageSidePanelItemProps) {
  return (
    <div className={cn('flex flex-col gap-2', className)} {...props}>
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm text-foreground">{title}</span>
          {description && <span className="text-xs text-muted-foreground">{description}</span>}
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      {children}
    </div>
  )
}

export {
  PageSidePanel,
  PageSidePanelItem,
  type PageSidePanelItemProps,
  type PageSidePanelPlacement,
  type PageSidePanelProps,
  PageSidePanelSection,
  type PageSidePanelSectionProps
}
