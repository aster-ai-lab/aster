import type { ReactNode } from 'react'
import { cn } from '../../lib/cn'
import { Button } from '../primitives/button'
import {
  Drawer as PrimitiveDrawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from '../primitives/drawer'

export type DrawerProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
  side?: 'right' | 'left'
  className?: string
}

/**
 * Side panel overlay on the shared primitives Drawer (vaul + Arc chrome).
 * Prefer primitives `Drawer` + `DrawerContent` for new Studio surfaces.
 */
export function Drawer({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  side = 'right',
  className = '',
}: DrawerProps) {
  return (
    <PrimitiveDrawer open={open} onOpenChange={onOpenChange} direction={side}>
      <DrawerContent className={cn('ui-drawer-root ui-drawer-panel', className)}>
        <DrawerHeader className="flex flex-row items-start justify-between gap-2 px-6 pb-2 pt-5 text-left">
          <div>
            <DrawerTitle className="text-[15px] font-medium leading-6 text-ink">{title}</DrawerTitle>
            {description ? (
              <DrawerDescription className="mt-0.5 text-[13px] text-ink-faint">
                {description}
              </DrawerDescription>
            ) : null}
          </div>
          <DrawerClose asChild>
            <Button variant="ghost" size="sm" aria-label="Close">
              ×
            </Button>
          </DrawerClose>
        </DrawerHeader>
        <div className="flex-1 overflow-auto px-6 py-3">{children}</div>
        {footer ? (
          <DrawerFooter className="flex justify-end gap-2 px-6 pb-5 pt-3 sm:flex-row">
            {footer}
          </DrawerFooter>
        ) : null}
      </DrawerContent>
    </PrimitiveDrawer>
  )
}
