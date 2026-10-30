'use client'

import * as HoverCardPrimitive from '@radix-ui/react-hover-card'
import * as React from 'react'

import { cn } from '../../lib/utils'
import { PopupDismissContext, usePopupDismissal, usePopupState } from './popup-dismissal'
import { usePortalContainer } from './portal-container'

function HoverCard({
  open,
  defaultOpen,
  onOpenChange,
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Root>) {
  const state = usePopupState({ open, defaultOpen, onOpenChange })
  return (
    <PopupDismissContext value={state}>
      <HoverCardPrimitive.Root data-slot="hover-card" {...props} {...state} />
    </PopupDismissContext>
  )
}

function HoverCardTrigger({ ...props }: React.ComponentProps<typeof HoverCardPrimitive.Trigger>) {
  return <HoverCardPrimitive.Trigger data-slot="hover-card-trigger" {...props} />
}

function HoverCardContent({
  className,
  align = 'center',
  sideOffset = 4,
  portalContainer,
  ref,
  onPointerDownCapture,
  onClickCapture,
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof HoverCardPrimitive.Portal>['container']
}) {
  const defaultPortalContainer = usePortalContainer()
  const dismissal = usePopupDismissal(ref)

  return (
    <HoverCardPrimitive.Portal container={portalContainer ?? defaultPortalContainer ?? undefined}>
      <HoverCardPrimitive.Content
        data-slot="hover-card-content"
        align={align}
        sideOffset={sideOffset}
        ref={dismissal.ref}
        onPointerDownCapture={(event) => {
          dismissal.inside()
          onPointerDownCapture?.(event)
        }}
        onClickCapture={(event) => {
          dismissal.inside()
          onClickCapture?.(event)
        }}
        className={cn(
          // no-drag punches the popup's area out of any titlebar drag region it overlaps,
          // so hover/click reach the items instead of the window-drag hit test (Electron).
          'z-[80] w-80 origin-(--radix-hover-card-content-transform-origin) rounded-lg border-[0.5px] bg-popover p-4 text-popover-foreground shadow-lg outline-hidden [-webkit-app-region:no-drag]',
          className
        )}
        {...props}
      />
    </HoverCardPrimitive.Portal>
  )
}

export { HoverCard, HoverCardContent, HoverCardTrigger }
