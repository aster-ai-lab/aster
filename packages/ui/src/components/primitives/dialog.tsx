import { composeEventHandlers } from '@radix-ui/primitive'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'
import { buttonVariants } from './button'
import { DIALOG_CLOSE_DURATION_MS } from '@a3s-lab/ui/utils'

import { PortalContainerProvider, useDialogPortalContainer } from './portal-container'

function Dialog({ ...props }: React.ComponentProps<typeof DialogPrimitive.Root>) {
  return <DialogPrimitive.Root data-slot="dialog" {...props} />
}

function DialogTrigger({ ...props }: React.ComponentProps<typeof DialogPrimitive.Trigger>) {
  return <DialogPrimitive.Trigger data-slot="dialog-trigger" {...props} />
}

function DialogPortal({ container, ...props }: React.ComponentProps<typeof DialogPrimitive.Portal>) {
  const defaultPortalContainer = useDialogPortalContainer()
  return <DialogPrimitive.Portal data-slot="dialog-portal" container={container ?? defaultPortalContainer} {...props} />
}

function DialogClose({ ...props }: React.ComponentProps<typeof DialogPrimitive.Close>) {
  return <DialogPrimitive.Close data-slot="dialog-close" {...props} />
}

function DialogOverlay({ className, onPointerDown, ...props }: React.ComponentProps<typeof DialogPrimitive.Overlay>) {
  return (
    <DialogPrimitive.Overlay
      data-slot="dialog-overlay"
      className={cn(
        'fixed inset-0 z-[80] bg-black/20',
        'motion-reduce:animate-none',
        className
      )}
      onPointerDown={composeEventHandlers(onPointerDown, (event) => event.stopPropagation(), {
        checkForDefaultPrevented: false
      })}
      {...props}
    />
  )
}

type DialogContentSize = 'sm' | 'default' | 'lg' | 'xl'
type DialogContentMotion = 'directional' | 'fade-scale'

const dialogContentSizeClass: Record<DialogContentSize, string> = {
  sm: 'sm:max-w-sm',
  default: 'sm:max-w-lg',
  lg: 'sm:max-w-xl',
  xl: 'sm:max-w-[720px]'
}

type DialogContentProps = React.ComponentProps<typeof DialogPrimitive.Content> & {
  closeLabel?: string
  closeOnOverlayClick?: boolean
  motion?: DialogContentMotion
  overlayClassName?: string
  showCloseButton?: boolean
  size?: DialogContentSize
}

/**
 * Close-animation duration (ms) of DialogContent. The leave keyframe in
 * `styles/dialog-motion.css` (`a3s-dialog-out 200ms`) must stay equal to this
 * constant. A co-located test reads that file. Imperative host removal timing
 * is DIALOG_UNMOUNT_DELAY_MS.
 */
export { DIALOG_CLOSE_DURATION_MS }

function DialogContent({
  className,
  children,
  closeLabel = 'Close',
  closeOnOverlayClick = true,
  showCloseButton = true,
  motion = 'fade-scale',
  overlayClassName,
  onPointerDown,
  size = 'default',
  ref,
  ...props
}: DialogContentProps) {
  const [contentElement, setContentElement] = React.useState<HTMLDivElement | null>(null)
  // A later pr-12 would replace only the right side of p-0 and double-inset headers that already clear the icon.
  const contentBleeds = typeof className === 'string' && /(?:^|\s)!?p-0(?![\w.-])/.test(className)
  const handleRef = React.useCallback(
    (node: HTMLDivElement | null) => {
      setContentElement(node)
      if (typeof ref === 'function') {
        ref(node)
      } else if (ref) {
        ref.current = node
      }
    },
    [ref]
  )

  return (
    <DialogPortal data-slot="dialog-portal">
      {/* Keep this tree stable when async flows temporarily disable overlay dismissal. */}
      <DialogPrimitive.Close asChild>
        <DialogOverlay
          className={overlayClassName}
          onClick={(event) => {
            if (!closeOnOverlayClick) event.preventDefault()
          }}
        />
      </DialogPrimitive.Close>
      {/*
        Keep Content as a direct Portal child so Radix Presence receives its DOM ref and
        can retain it for the closed-state animation. Nested overlays still inherit the
        content element as their portal target from the provider inside Content.

        The leave duration in styles/dialog-motion.css must equal DIALOG_CLOSE_DURATION_MS.
        Imperative host removal timing is DIALOG_UNMOUNT_DELAY_MS.
        `directional` stays in the type so existing callers compile. It uses the same
        in-place motion. A dialog does not travel.
      */}
      <DialogPrimitive.Content
        ref={handleRef}
        data-slot="dialog-content"
        data-dialog-motion={motion}
        className={cn(
          // no-drag punches the dialog's area out of any titlebar drag region it overlaps,
          // so the close button stays clickable when the dialog reaches max height (Electron).
          'fixed top-[50%] left-[50%] z-[80] grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-[12px] border-0 bg-card p-6 text-card-foreground shadow-[0_12px_32px_rgba(0,0,0,0.12)] [-webkit-app-region:no-drag]',
          'motion-reduce:animate-none',
          dialogContentSizeClass[size],
          className,
          showCloseButton && !contentBleeds && 'pr-12'
        )}
        onPointerDown={composeEventHandlers(onPointerDown, (event) => event.stopPropagation(), {
          checkForDefaultPrevented: false
        })}
        {...props}>
        <PortalContainerProvider container={contentElement}>
          {showCloseButton && (
            <DialogPrimitive.Close
              type="button"
              aria-label={closeLabel}
              data-slot="dialog-close"
              data-variant="ghost"
              className={cn(
                buttonVariants({ variant: 'ghost', size: 'icon-sm' }),
                'absolute top-4 right-4 z-10 size-6 text-[#3f3f3f] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset'
              )}>
              <X aria-hidden size={14} strokeWidth={1.7} />
            </DialogPrimitive.Close>
          )}
          {children}
        </PortalContainerProvider>
      </DialogPrimitive.Content>
    </DialogPortal>
  )
}

function DialogHeader({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot="dialog-header"
      className={cn('flex flex-col gap-2 text-center sm:text-left', className)}
      {...props}
    />
  )
}

function DialogFooter({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot="dialog-footer"
      className={cn('flex flex-col-reverse gap-2 sm:flex-row sm:justify-end', className)}
      {...props}
    />
  )
}

function DialogTitle({ className, ...props }: React.ComponentProps<typeof DialogPrimitive.Title>) {
  return (
    <DialogPrimitive.Title
      data-slot="dialog-title"
      className={cn('text-lg leading-none font-semibold', className)}
      {...props}
    />
  )
}

function DialogDescription({ className, ...props }: React.ComponentProps<typeof DialogPrimitive.Description>) {
  return (
    <DialogPrimitive.Description
      data-slot="dialog-description"
      className={cn('text-sm text-muted-foreground', className)}
      {...props}
    />
  )
}

export {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger
}
