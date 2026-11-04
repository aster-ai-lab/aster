import {
  useId,
  type FormEvent,
  type ReactNode,
} from 'react'
import { cn } from '../../lib/cn'
import { Button } from '../primitives/button'
import {
  Dialog as PrimitiveDialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../primitives/dialog'

export type DialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
  /** Wider panel for forms; `full` = viewport-filling editor shell */
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  /** Hide title/description chrome (still exposed to a11y via title) */
  hideHeader?: boolean
  /** Show × in header (default true). */
  showClose?: boolean
  /** Accessible label for the close control. */
  closeLabel?: string
  className?: string
}

const contentSize = {
  sm: 'sm',
  md: 'md',
  lg: 'lg',
  xl: 'xl',
  full: 'xl',
} as const

/**
 * Platform dialog shell on the shared Arc/primitives Dialog — fixed header/footer,
 * scrollable body, optional close. Prefer this controlled API from Console forms;
 * Studio product surfaces should use primitives `Dialog` + `DialogContent` directly.
 */
export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  size = 'md',
  hideHeader = false,
  showClose = true,
  closeLabel = 'Close',
  className = '',
}: DialogProps) {
  return (
    <PrimitiveDialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        size={contentSize[size]}
        showCloseButton={showClose && !hideHeader}
        closeLabel={closeLabel}
        className={cn(
          size === 'full' && 'ui-dialog-panel--full h-[min(100vh,100dvh)] max-h-[min(100vh,100dvh)] max-w-[min(100vw,100dvw)] rounded-none sm:max-w-[min(100vw,100dvw)]',
          'grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0',
          className,
        )}
      >
        {hideHeader ? (
          <DialogTitle className="sr-only">{title}</DialogTitle>
        ) : (
          <DialogHeader className="ui-dialog-header gap-1 border-b border-[#efefef] px-5 py-4 pr-12 text-left">
            <DialogTitle className="ui-dialog-title">{title}</DialogTitle>
            {description ? (
              <DialogDescription className="ui-dialog-desc">{description}</DialogDescription>
            ) : null}
          </DialogHeader>
        )}
        <div className="ui-dialog-body min-h-0 overflow-y-auto px-5 py-4">{children}</div>
        {footer ? (
          <DialogFooter className="ui-dialog-footer border-t border-[#efefef] px-5 py-3 sm:justify-end">
            {footer}
          </DialogFooter>
        ) : null}
      </DialogContent>
    </PrimitiveDialog>
  )
}

export type FormDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  size?: DialogProps['size']
  closeLabel?: string
  cancelLabel: string
  submitLabel: string
  /** Hide cancel/submit while e.g. cropping. */
  hideFooter?: boolean
  submitting?: boolean
  submitDisabled?: boolean
  /** Native form submit — wire to react-hook-form handleSubmit. */
  onSubmit: (event: FormEvent<HTMLFormElement>) => void
  children: ReactNode
  className?: string
}

/**
 * Standard platform form dialog: scrollable body, sticky chrome, submit loading.
 * Pair with react-hook-form via `onSubmit={handleSubmit(onValid)}`.
 */
export function FormDialog({
  open,
  onOpenChange,
  title,
  description,
  size = 'md',
  closeLabel = 'Close',
  cancelLabel,
  submitLabel,
  hideFooter = false,
  submitting = false,
  submitDisabled = false,
  onSubmit,
  children,
  className = '',
}: FormDialogProps) {
  const formId = useId()

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (submitting) return
        onOpenChange(next)
      }}
      title={title}
      description={description}
      size={size}
      closeLabel={closeLabel}
      showClose
      className={className}
      footer={
        hideFooter ? null : (
          <>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={submitting}
              onClick={() => onOpenChange(false)}
            >
              {cancelLabel}
            </Button>
            <Button
              type="submit"
              size="sm"
              form={formId}
              disabled={submitDisabled || submitting}
              loading={submitting}
            >
              {submitLabel}
            </Button>
          </>
        )
      }
    >
      <form
        id={formId}
        className="ui-form-dialog-form"
        noValidate
        onSubmit={(event) => {
          // The dialog is portaled to document.body, but React still bubbles
          // submit through the component that rendered it. Stop that so a
          // stacked form dialog cannot submit the form underneath.
          event.stopPropagation()
          onSubmit(event)
        }}
      >
        {children}
      </form>
    </Dialog>
  )
}
