import * as React from 'react'

import { useConfirmationAction } from '../../../lib/use-confirmation-action'
import { Button } from '../../primitives/button'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '../../primitives/dialog'

interface ConfirmDialogProps {
  /** Controls the open state of the dialog */
  open?: boolean
  /** Callback when open state changes */
  onOpenChange?: (open: boolean) => void
  /** Dialog title */
  title: React.ReactNode
  /** Dialog description */
  description?: React.ReactNode
  /** Custom content below description */
  content?: React.ReactNode
  /** Confirm button text */
  confirmText?: string
  /** Cancel button text */
  cancelText?: string
  /** Disabled state for cancel button */
  cancelDisabled?: boolean
  /** Return false to keep the dialog open; void or true closes it. */
  onConfirm?: () => boolean | void | Promise<boolean | void>
  /** Whether this is a destructive action (e.g., delete) */
  destructive?: boolean
  /** Loading state for confirm button */
  confirmLoading?: boolean
  /** Disabled state for confirm button */
  confirmDisabled?: boolean
  /** Optional className for DialogContent */
  contentClassName?: string
  /** Optional className for DialogOverlay */
  overlayClassName?: string
  /** Corner close control. Plain confirmations leave this off. */
  showCloseButton?: boolean
  /** Accessible name for the corner close control. */
  closeLabel?: string
  /** Change when a different entity or reviewed revision replaces this confirmation. */
  operationKey?: string
  failureText?: string
}

function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  content,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  cancelDisabled = false,
  onConfirm,
  destructive = false,
  confirmLoading = false,
  confirmDisabled = false,
  contentClassName,
  overlayClassName,
  showCloseButton = false,
  closeLabel,
  operationKey,
  failureText = 'The action failed. Try again.'
}: ConfirmDialogProps) {
  const { submit, failed, busy } = useConfirmationAction({
    open,
    operationKey: operationKey ?? (typeof title === 'string' ? title : undefined),
    loading: confirmLoading,
    onConfirm,
    onClose: () => onOpenChange?.(false)
  })

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!busy) onOpenChange?.(next)
      }}>
      <DialogContent
        motion="fade-scale"
        showCloseButton={showCloseButton}
        closeLabel={closeLabel}
        className={contentClassName}
        overlayClassName={overlayClassName}>
        <DialogHeader className={showCloseButton ? 'pr-8' : undefined}>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {content}
        {failed ? (
          <p role="alert" className="text-sm text-[color:var(--color-danger-text)]">
            {failureText}
          </p>
        ) : null}
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline" className="rounded-[8px]" disabled={cancelDisabled || busy}>
              {cancelText}
            </Button>
          </DialogClose>
          <Button
            variant={destructive ? 'destructive' : 'default'}
            className="rounded-[8px]"
            onClick={() => void submit()}
            loading={busy}
            disabled={confirmDisabled || busy}>
            {confirmText}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export { ConfirmDialog, type ConfirmDialogProps }
