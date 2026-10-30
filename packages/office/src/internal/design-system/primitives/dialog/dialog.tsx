import { X } from 'lucide-react';
import { usePresence } from 'motion/react';
import { useReducedMotion } from '@a3s-lab/ui/lib/useReducedMotion';
import { DIALOG_UNMOUNT_DELAY_MS } from '@a3s-lab/ui/utils';
import { type ReactNode, useEffect, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import { IconButton } from '../button/icon-button';
import { OfficeControlSizeContext } from '../form/control-size';
import { useDialogFocusScope } from '../overlay/dialog-focus-scope';
import { hasOpenOfficePopover } from '../overlay/popover';
import { officeOverlayPortalRoot } from '../overlay/portal-root';

export function Dialog({
  title,
  closeLabel,
  description,
  children,
  footer,
  onClose,
  onEscape = onClose,
  closeDisabled = false,
  className,
  focusKey,
  restoreFocusTarget,
}: {
  title: string;
  closeLabel: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
  /** Escape / dialog cancel. Defaults to `onClose`. Use to restore dirty drafts first. */
  onEscape?: () => void;
  closeDisabled?: boolean;
  className?: string;
  focusKey?: string | number;
  restoreFocusTarget?: () => HTMLElement | null;
}) {
  const [present, safeToRemove] = usePresence();
  const reducedMotion = useReducedMotion();
  const titleId = useId();
  const descriptionId = useId();
  const portalRootRef = useRef<HTMLElement | null>(null);
  if (!portalRootRef.current && typeof document !== 'undefined') {
    portalRootRef.current = officeOverlayPortalRoot(
      document,
      restoreFocusTarget?.(),
    );
  }
  const focusScope = useDialogFocusScope<HTMLElement>({
    onEscape,
    escapeDisabled: closeDisabled || !present,
    restoreFocusTarget,
  });
  useEffect(() => {
    if (focusKey !== undefined) focusScope.focusInitial();
  }, [focusKey, focusScope.focusInitial]);
  useEffect(() => {
    if (present || !safeToRemove) return;
    if (reducedMotion) {
      safeToRemove();
      return;
    }
    // Background tabs can suspend animationend; use the shared host deadline.
    const timer = window.setTimeout(safeToRemove, DIALOG_UNMOUNT_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [present, reducedMotion, safeToRemove]);
  const dialog = (
    <OfficeControlSizeContext value="md">
      <dialog
        open
        className="ds-dialog-backdrop a3s-dialog-scrim"
        data-state={present ? 'open' : 'closed'}
        role="presentation"
        onClick={(event) => {
          if (event.target !== event.currentTarget || closeDisabled || !present || hasOpenOfficePopover()) return;
          onEscape();
        }}
        onCancel={(event) => {
          event.preventDefault();
          if (closeDisabled || !present || hasOpenOfficePopover()) return;
          onEscape();
        }}
      >
        <section
          ref={focusScope.scopeRef}
          className={`ds-dialog a3s-dialog-surface${className ? ` ${className}` : ''}`}
          data-state={present ? 'open' : 'closed'}
          inert={!present}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={description ? descriptionId : undefined}
          onKeyDown={focusScope.handleKeyDown}
          onAnimationEnd={(event) => {
            if (!present && event.target === event.currentTarget && event.animationName === 'a3s-dialog-out') safeToRemove?.();
          }}
        >
          <header>
            <div>
              <h2 id={titleId}>{title}</h2>
              {description && <p id={descriptionId}>{description}</p>}
            </div>
            <IconButton className="ds-dialog-close" label={closeLabel} disabled={closeDisabled || !present} onClick={onClose}>
              <X aria-hidden size={14} strokeWidth={1.7} />
            </IconButton>
          </header>
          <div className="ds-dialog-body">{children}</div>
          {footer && <footer>{footer}</footer>}
        </section>
      </dialog>
    </OfficeControlSizeContext>
  );
  return typeof document === 'undefined' || !portalRootRef.current
    ? dialog
    : createPortal(dialog, portalRootRef.current);
}
