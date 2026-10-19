'use client'

import * as HoverCardPrimitive from '@radix-ui/react-hover-card'
import * as React from 'react'

import { cn } from '../../lib/utils'

import { usePortalContainer } from './portal-container'

function HoverCard({ ...props }: React.ComponentProps<typeof HoverCardPrimitive.Root>) {
  return <HoverCardPrimitive.Root data-slot="hover-card" {...props} />
}

function HoverCardTrigger({ ...props }: React.ComponentProps<typeof HoverCardPrimitive.Trigger>) {
  return <HoverCardPrimitive.Trigger data-slot="hover-card-trigger" {...props} />
}

function HoverCardContent({
  className,
  align = 'center',
  sideOffset = 4,
  portalContainer,
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof HoverCardPrimitive.Portal>['container']
}) {
  const defaultPortalContainer = usePortalContainer()

  return (
    <HoverCardPrimitive.Portal container={portalContainer ?? defaultPortalContainer ?? undefined}>
      <HoverCardPrimitive.Content
        data-slot="hover-card-content"
        align={align}
        sideOffset={sideOffset}
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
