'use client'

import * as PopoverPrimitive from '@radix-ui/react-popover'
import * as React from 'react'

import { cn } from '../../lib/utils'
import { PopupDismissContext, usePopupDismissal, usePopupState } from './popup-dismissal'
import { usePortalContainer } from './portal-container'

function Popover({ open, defaultOpen, onOpenChange, ...props }: React.ComponentProps<typeof PopoverPrimitive.Root>) {
  const state = usePopupState({ open, defaultOpen, onOpenChange })
  return (
    <PopupDismissContext value={state}>
      <PopoverPrimitive.Root data-slot="popover" {...props} {...state} />
    </PopupDismissContext>
  )
}

function PopoverTrigger({ ...props }: React.ComponentProps<typeof PopoverPrimitive.Trigger>) {
  return <PopoverPrimitive.Trigger data-slot="popover-trigger" {...props} />
}

function PopoverContent({
  className,
  align = 'center',
  sideOffset = 4,
  forceMount,
  portalContainer,
  ref,
  onPointerDownCapture,
  onClickCapture,
  onCloseAutoFocus,
  ...props
}: React.ComponentProps<typeof PopoverPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof PopoverPrimitive.Portal>['container']
}) {
  const defaultPortalContainer = usePortalContainer()
  const dismissal = usePopupDismissal(ref)

  return (
    <PopoverPrimitive.Portal forceMount={forceMount} container={portalContainer ?? defaultPortalContainer ?? undefined}>
      <PopoverPrimitive.Content
        data-slot="popover-content"
        align={align}
        sideOffset={sideOffset}
        forceMount={forceMount}
        ref={dismissal.ref}
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
        className={cn(
          // no-drag punches the popup's area out of any titlebar drag region it overlaps,
          // so hover/click reach the items instead of the window-drag hit test (Electron).
          'z-[240] w-72 origin-(--radix-popover-content-transform-origin) rounded-[12px] border border-[color:var(--color-border)] bg-[var(--color-bg-elevated)] p-4 text-[13px] text-[var(--color-ink)] shadow-[var(--shadow-elevated)] outline-hidden [-webkit-app-region:no-drag]',
          className
        )}
        {...props}
      />
    </PopoverPrimitive.Portal>
  )
}

function PopoverAnchor({ ...props }: React.ComponentProps<typeof PopoverPrimitive.Anchor>) {
  return <PopoverPrimitive.Anchor data-slot="popover-anchor" {...props} />
}

export { Popover, PopoverAnchor, PopoverContent, PopoverTrigger }
