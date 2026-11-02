import * as DropdownMenuPrimitive from '@radix-ui/react-dropdown-menu'
import { CheckIcon, ChevronRightIcon, CircleIcon } from 'lucide-react'
import { motion } from 'motion/react'
import * as React from 'react'

import { useReducedMotion } from '../../lib/useReducedMotion'
import { cn } from '../../lib/utils'
import { PopupDismissContext, usePopupDismissal, usePopupState } from './popup-dismissal'
import { usePortalContainer } from './portal-container'

import arcDropdown from '../arc/dropdown-menu/dropdown-menu.module.css'

function DropdownMenu({
  open,
  defaultOpen,
  onOpenChange,
  modal = false,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Root>) {
  const state = usePopupState({ open, defaultOpen, onOpenChange })
  return (
    <PopupDismissContext value={state}>
      <DropdownMenuPrimitive.Root data-slot="dropdown-menu" modal={modal} {...props} {...state} />
    </PopupDismissContext>
  )
}

function DropdownMenuPortal({ container, ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Portal>) {
  const defaultPortalContainer = usePortalContainer()
  return (
    <DropdownMenuPrimitive.Portal
      data-slot="dropdown-menu-portal"
      container={container ?? defaultPortalContainer ?? undefined}
      {...props}
    />
  )
}

function DropdownMenuTrigger({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Trigger>) {
  return <DropdownMenuPrimitive.Trigger data-slot="dropdown-menu-trigger" {...props} />
}

type DropdownMenuContentProps = React.ComponentProps<typeof DropdownMenuPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof DropdownMenuPrimitive.Portal>['container']
}

/**
 * A press on the menu's own trigger re-opens or closes the menu itself, so the
 * layer it must not reach is the one that is still animating out. Exported for
 * the primitive's tests: the handler it guards can only be observed in a browser.
 */
export function isTriggerPress(event: { target: EventTarget | null }): boolean {
  const target = event.target as Element | null
  return Boolean(target?.closest?.('[data-slot="dropdown-menu-trigger"]'))
}

function DropdownMenuContent({
  className,
  sideOffset = 4,
  portalContainer,
  onPointerDownOutside,
  ref,
  onPointerDownCapture,
  onClickCapture,
  onCloseAutoFocus,
  onFocusCapture,
  onKeyDownCapture,
  onPointerMoveCapture,
  children,
  ...props
}: DropdownMenuContentProps) {
  const dismissal = usePopupDismissal(ref)
  const popupState = React.use(PopupDismissContext)
  const contentRef = React.useRef<HTMLDivElement | null>(null)
  const focusedItemRef = React.useRef<HTMLElement | null>(null)
  const highlightObserverRef = React.useRef<ResizeObserver | null>(null)
  const pointer = React.useRef(false)
  const clearTimer = React.useRef(0)
  const reducedMotion = useReducedMotion()
  const isAster = typeof document !== 'undefined' && document.documentElement.dataset.tenant === 'aster'
  const canAnimateHighlight = isAster && !props.asChild
  const [highlight, setHighlight] = React.useState<{
    top: number
    left: number
    width: number
    height: number
    glide: boolean
    danger: boolean
  } | null>(null)
  const stopObservingHighlight = React.useCallback(() => {
    highlightObserverRef.current?.disconnect()
    highlightObserverRef.current = null
    focusedItemRef.current = null
  }, [])
  const measureHighlight = React.useCallback((target: HTMLElement, glide: boolean) => {
    const content = contentRef.current
    if (!content?.contains(target)) return
    const box = target.getBoundingClientRect()
    const bounds = content.getBoundingClientRect()
    const top = box.top - bounds.top + content.scrollTop - content.clientTop
    const left = box.left - bounds.left + content.scrollLeft - content.clientLeft
    const danger = target.dataset.variant === 'destructive'
    setHighlight((current) => {
      if (
        current &&
        current.top === top &&
        current.left === left &&
        current.width === box.width &&
        current.height === box.height &&
        current.danger === danger
      ) return current
      return { top, left, width: box.width, height: box.height, glide: glide && current !== null, danger }
    })
  }, [])
  React.useEffect(
    () => () => {
      window.clearTimeout(clearTimer.current)
      stopObservingHighlight()
    },
    [stopObservingHighlight]
  )
  React.useEffect(() => {
    if (popupState?.open !== false) return
    window.clearTimeout(clearTimer.current)
    pointer.current = false
    stopObservingHighlight()
    setHighlight(null)
  }, [popupState?.open, stopObservingHighlight])
  const setContentRef = React.useCallback(
    (node: HTMLDivElement | null) => {
      contentRef.current = node
      const cleanup = dismissal.ref(node)
      if (cleanup)
        return () => {
          contentRef.current = null
          cleanup()
        }
      return undefined
    },
    [dismissal.ref]
  )
  return (
    <DropdownMenuPortal container={portalContainer}>
      <DropdownMenuPrimitive.Content
        data-slot="dropdown-menu-content"
        sideOffset={sideOffset}
        ref={setContentRef}
        data-aster-gliding-highlight={canAnimateHighlight && highlight ? '' : undefined}
        onPointerMoveCapture={(event) => {
          pointer.current = true
          onPointerMoveCapture?.(event)
        }}
        onKeyDownCapture={(event) => {
          pointer.current = false
          onKeyDownCapture?.(event)
        }}
        onFocusCapture={(event) => {
          onFocusCapture?.(event)
          if (!canAnimateHighlight) return
          window.clearTimeout(clearTimer.current)
          const content = contentRef.current
          // A submenu is portalled but still bubbles React focus events through here.
          if (!content?.contains(event.target)) return
          const target =
            event.target instanceof Element
              ? event.target.closest<HTMLElement>(
                  '[role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]'
                )
              : null
          if (!target || !content.contains(target)) {
            stopObservingHighlight()
            clearTimer.current = window.setTimeout(() => setHighlight(null), pointer.current ? 70 : 0)
            return
          }
          if (focusedItemRef.current !== target) {
            stopObservingHighlight()
            focusedItemRef.current = target
            if (typeof ResizeObserver !== 'undefined') {
              const observer = new ResizeObserver(() => {
                if (focusedItemRef.current === target) measureHighlight(target, false)
              })
              highlightObserverRef.current = observer
              observer.observe(content)
              observer.observe(target)
            }
          }
          measureHighlight(target, pointer.current)
        }}
        onPointerDownCapture={(event) => {
          dismissal.inside()
          onPointerDownCapture?.(event)
        }}
        onClickCapture={(event) => {
          dismissal.inside()
          onClickCapture?.(event)
        }}
        onCloseAutoFocus={(event) => {
          onCloseAutoFocus?.(event)
          dismissal.onCloseAutoFocus(event)
        }}
        // Popups keep their content mounted for the length of the leave animation
        // in popup-motion.css (200ms), and that dying layer is still registered as
        // a dismissable layer. A press on the trigger during that window is
        // therefore an "outside" press for it, and it dismisses the menu the same
        // press just opened: the menu blinks shut and the trigger looks dead. Hand
        // the trigger's own press back to the trigger.
        onPointerDownOutside={(event) => {
          onPointerDownOutside?.(event)
          if (event.defaultPrevented) return
          if (isTriggerPress(event)) event.preventDefault()
        }}
        className={cn(
          // no-drag punches the popup's area out of any titlebar drag region it overlaps,
          // so hover/click reach the items instead of the window-drag hit test (Electron).
          'arc',
          arcDropdown.menuSurface,
          'relative z-[240] max-h-(--radix-dropdown-menu-content-available-height) min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-x-hidden overflow-y-auto text-[13px] [-webkit-app-region:no-drag] data-[state=closed]:pointer-events-none',
          className
        )}
        {...props}>
        {canAnimateHighlight ? (
          <>
            {highlight && (
              <motion.span
                aria-hidden="true"
                data-slot="dropdown-menu-highlight"
                className={arcDropdown.highlight}
                style={{ left: 0, right: 'auto' }}
                data-tone={highlight.danger ? 'danger' : undefined}
                initial={false}
                animate={{
                  x: highlight.left,
                  y: highlight.top,
                  width: highlight.width,
                  height: highlight.height,
                  opacity: 1
                }}
                transition={
                  highlight.glide && !reducedMotion ? { type: 'spring', duration: 0.24, bounce: 0 } : { duration: 0 }
                }
              />
            )}
            {children}
          </>
        ) : (
          children
        )}
      </DropdownMenuPrimitive.Content>
    </DropdownMenuPortal>
  )
}

function DropdownMenuGroup({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Group>) {
  return <DropdownMenuPrimitive.Group data-slot="dropdown-menu-group" {...props} />
}

function DropdownMenuItem({
  className,
  inset,
  variant = 'default',
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Item> & {
  inset?: boolean
  variant?: 'default' | 'destructive'
}) {
  return (
    <DropdownMenuPrimitive.Item
      data-slot="dropdown-menu-item"
      data-inset={inset}
      data-variant={variant}
      className={cn(
        'arc',
        arcDropdown.item,
        'relative flex items-center gap-2 text-[13px] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8 data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 data-[variant=destructive]:focus:text-destructive dark:data-[variant=destructive]:focus:bg-destructive/20 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4 data-[variant=destructive]:[&_svg:not([class*="text-"])]:text-destructive!',
        className
      )}
      {...props}
    />
  )
}

function DropdownMenuCheckboxItem({
  className,
  children,
  checked,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.CheckboxItem>) {
  return (
    <DropdownMenuPrimitive.CheckboxItem
      data-slot="dropdown-menu-checkbox-item"
      className={cn(
        'arc',
        arcDropdown.item,
        'relative flex items-center gap-2 pr-2 pl-8 text-[13px] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
        className
      )}
      checked={checked}
      {...props}>
      <span className="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        <DropdownMenuPrimitive.ItemIndicator>
          <CheckIcon className="size-4" />
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
      {children}
    </DropdownMenuPrimitive.CheckboxItem>
  )
}

function DropdownMenuRadioGroup({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.RadioGroup>) {
  return <DropdownMenuPrimitive.RadioGroup data-slot="dropdown-menu-radio-group" {...props} />
}

function DropdownMenuRadioItem({
  className,
  children,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.RadioItem>) {
  return (
    <DropdownMenuPrimitive.RadioItem
      data-slot="dropdown-menu-radio-item"
      className={cn(
        'arc',
        arcDropdown.item,
        'relative flex items-center gap-2 pr-2 pl-8 text-[13px] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
        className
      )}
      {...props}>
      <span className="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        <DropdownMenuPrimitive.ItemIndicator>
          <CircleIcon className="size-2 fill-current" />
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
      {children}
    </DropdownMenuPrimitive.RadioItem>
  )
}

function DropdownMenuLabel({
  className,
  inset,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Label> & {
  inset?: boolean
}) {
  return (
    <DropdownMenuPrimitive.Label
      data-slot="dropdown-menu-label"
      data-inset={inset}
      className={cn('px-2 py-1.5 text-sm font-medium data-[inset]:pl-8', className)}
      {...props}
    />
  )
}

function DropdownMenuSeparator({ className, ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Separator>) {
  return (
    <DropdownMenuPrimitive.Separator
      data-slot="dropdown-menu-separator"
      className={cn('-mx-1 my-1 h-px bg-border', className)}
      {...props}
    />
  )
}

function DropdownMenuShortcut({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <span
      data-slot="dropdown-menu-shortcut"
      className={cn('ml-auto text-xs tracking-widest text-muted-foreground', className)}
      {...props}
    />
  )
}

function DropdownMenuSub({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Sub>) {
  return <DropdownMenuPrimitive.Sub data-slot="dropdown-menu-sub" {...props} />
}

function DropdownMenuSubTrigger({
  className,
  inset,
  children,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.SubTrigger> & {
  inset?: boolean
}) {
  return (
    <DropdownMenuPrimitive.SubTrigger
      data-slot="dropdown-menu-sub-trigger"
      data-inset={inset}
      className={cn(
        'arc',
        arcDropdown.item,
        'relative flex cursor-default items-center gap-2 text-[13px] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8 data-[state=open]:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
        className
      )}
      {...props}>
      {children}
      <ChevronRightIcon className="ml-auto size-4" />
    </DropdownMenuPrimitive.SubTrigger>
  )
}

function DropdownMenuSubContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.SubContent>) {
  return (
    <DropdownMenuPortal>
      <DropdownMenuPrimitive.SubContent
        data-slot="dropdown-menu-sub-content"
        className={cn(
          'arc',
          arcDropdown.menuSurface,
          'z-[240] min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-hidden text-[13px] [-webkit-app-region:no-drag]',
          className
        )}
        {...props}
      />
    </DropdownMenuPortal>
  )
}

export {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger
}
