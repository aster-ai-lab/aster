import { useEffect, useState, type ReactNode } from 'react';
import { Button } from '../primitives/button';
import { Dialog } from './Dialog';
import { Input } from '../primitives/input';
import { FormField } from './FormField';
import { useConfirmationAction } from '../../lib/use-confirmation-action';

export type ConfirmDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  /** When set, user must type this exact string to confirm */
  confirmText?: string;
  /** Field label. Omitted callers keep the English default sentence. */
  confirmHint?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: 'danger' | 'default';
  loading?: boolean;
  onConfirm: () => boolean | void | Promise<boolean | void>;
  children?: ReactNode;
  operationKey?: string;
  failureText?: string;
};

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmText,
  confirmHint,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  tone = 'danger',
  loading = false,
  onConfirm,
  children,
  operationKey,
  failureText = 'The action failed. Try again.',
}: ConfirmDialogProps) {
  const [typed, setTyped] = useState('');
  const { submit, failed, busy } = useConfirmationAction({
    open,
    operationKey: operationKey ?? `${title}\u0000${confirmText ?? ''}`,
    loading,
    onConfirm,
    onClose: () => onOpenChange(false),
  });

  useEffect(() => {
    setTyped('');
  }, [open, title, confirmText, operationKey]);

  const blocked = Boolean(confirmText) && typed !== confirmText;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!busy) onOpenChange(next);
      }}
      title={title}
      description={description}
      size="sm"
      footer={
        <>
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => onOpenChange(false)}
          >
            {cancelLabel}
          </Button>
          <Button
            variant={tone === 'danger' ? 'outline' : 'primary'}
            size="sm"
            className={
              tone === 'danger'
                ? '!text-[color:var(--color-danger-text)]'
                : undefined
            }
            disabled={busy || blocked}
            loading={busy}
            onClick={() => void submit()}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
      {failed ? (
        <p
          role="alert"
          className="text-sm text-[color:var(--color-danger-text)]"
        >
          {failureText}
        </p>
      ) : null}
      {confirmText ? (
        <FormField
          label={confirmHint ?? `Type “${confirmText}” to confirm`}
          className="mt-3"
        >
          <Input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
        </FormField>
      ) : null}
    </Dialog>
  );
}
