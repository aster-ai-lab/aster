import {
  useEffect,
  useId,
  useRef,
  type FormEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type DialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  /** Wider panel for forms; `full` = viewport-filling editor shell */
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full';
  /** Hide title/description chrome (still exposed to a11y via title) */
  hideHeader?: boolean;
  /** Show × in header (default true). */
  showClose?: boolean;
  /** Accessible label for the close control. */
  closeLabel?: string;
  className?: string;
};

const sizes = {
  sm: 'max-w-md',
  md: 'max-w-lg',
  lg: 'max-w-2xl',
  xl: 'max-w-4xl ui-dialog-panel--xl',
  full: 'ui-dialog-panel--full',
} as const;

function CloseIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M4.2 4.2 11.8 11.8M11.8 4.2 4.2 11.8"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

/**
 * Platform dialog shell: fixed header/footer, scrollable body, optional close.
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
  const titleId = useId();
  const descId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const onOpenChangeRef = useRef(onOpenChange);
  onOpenChangeRef.current = onOpenChange;

  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    const panel = panelRef.current;
    const focusable = panel?.querySelector<HTMLElement>(
      '.ui-dialog-body input, .ui-dialog-body textarea, .ui-dialog-body select, .ui-dialog-body button, .ui-dialog-body [href], .ui-dialog-body [tabindex]:not([tabindex="-1"])',
    );
    focusable?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onOpenChangeRef.current(false);
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      prev?.focus?.();
    };
  }, [open]);

  if (!open || typeof document === 'undefined') return null;

  return createPortal(
    <div
      className={cn('ui-dialog-root', size === 'full' && 'ui-dialog-root--full')}
      role="presentation"
    >
      <button
        type="button"
        className="ui-dialog-backdrop"
        aria-label={closeLabel}
        onClick={() => onOpenChange(false)}
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description && !hideHeader ? descId : undefined}
        className={cn('ui-dialog-panel', sizes[size], className)}
      >
        {hideHeader ? (
          <h2 id={titleId} className="ui-sr-only">
            {title}
          </h2>
        ) : (
          <header className="ui-dialog-header">
            <div className="ui-dialog-header__text">
              <h2 id={titleId} className="ui-dialog-title">
                {title}
              </h2>
              {description ? (
                <p id={descId} className="ui-dialog-desc">
                  {description}
                </p>
              ) : null}
            </div>
            {showClose ? (
              <button
                type="button"
                className="ui-dialog-close"
                aria-label={closeLabel}
                onClick={() => onOpenChange(false)}
              >
                <CloseIcon />
              </button>
            ) : null}
          </header>
        )}
        <div className="ui-dialog-body">{children}</div>
        {footer ? <footer className="ui-dialog-footer">{footer}</footer> : null}
      </div>
    </div>,
    document.body,
  );
}

export type FormDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  size?: DialogProps['size'];
  closeLabel?: string;
  cancelLabel: string;
  submitLabel: string;
  /** Hide cancel/submit while e.g. cropping. */
  hideFooter?: boolean;
  submitting?: boolean;
  submitDisabled?: boolean;
  /** Native form submit — wire to react-hook-form handleSubmit. */
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  children: ReactNode;
  className?: string;
};

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
  const formId = useId();

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (submitting) return;
        onOpenChange(next);
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
          event.stopPropagation();
          onSubmit(event);
        }}
      >
        {children}
      </form>
    </Dialog>
  );
}
