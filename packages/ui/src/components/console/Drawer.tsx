import {
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
  type TransitionEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type DrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  side?: 'right' | 'left';
  className?: string;
};

function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined') return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Side panel overlay — slide + backdrop fade.
 * Stays mounted through exit so CSS transitions can finish.
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
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const [mounted, setMounted] = useState(open);
  const [entered, setEntered] = useState(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      if (prefersReducedMotion()) {
        setEntered(true);
        return;
      }
      const id = requestAnimationFrame(() => {
        requestAnimationFrame(() => setEntered(true));
      });
      return () => cancelAnimationFrame(id);
    }
    if (prefersReducedMotion()) {
      setEntered(false);
      setMounted(false);
      return;
    }
    setEntered(false);
    // Fallback if transitionend is skipped (e.g. display:none mid-flight)
    const t = window.setTimeout(() => setMounted(false), 320);
    return () => clearTimeout(t);
  }, [open]);

  useEffect(() => {
    if (!mounted || !open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onOpenChange(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [mounted, open, onOpenChange]);

  function onPanelTransitionEnd(event: TransitionEvent<HTMLDivElement>) {
    if (event.target !== panelRef.current) return;
    if (event.propertyName !== 'transform') return;
    if (!open) setMounted(false);
  }

  if (!mounted || typeof document === 'undefined') return null;

  return createPortal(
    <div
      className={cn(
        'ui-drawer-root',
        entered ? 'ui-drawer-root--open' : 'ui-drawer-root--closing',
      )}
      role="presentation"
    >
      <button
        type="button"
        className="ui-drawer-backdrop"
        aria-label="Dismiss"
        onClick={() => onOpenChange(false)}
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={cn(
          'ui-drawer-panel',
          side === 'left' ? 'ui-drawer-panel--left' : 'ui-drawer-panel--right',
          className,
        )}
        onTransitionEnd={onPanelTransitionEnd}
      >
        <header className="flex items-start justify-between gap-2 px-6 pb-2 pt-5">
          <div>
            <h2 id={titleId} className="text-[15px] font-medium leading-6 text-ink">
              {title}
            </h2>
            {description ? (
              <p className="mt-0.5 text-[13px] text-ink-faint">{description}</p>
            ) : null}
          </div>
          <Button
            variant="ghost"
            size="sm"
            aria-label="Close"
            onClick={() => onOpenChange(false)}
          >
            ×
          </Button>
        </header>
        <div className="flex-1 overflow-auto px-6 py-3">{children}</div>
        {footer ? (
          <footer className="flex justify-end gap-2 px-6 pb-5 pt-3">
            {footer}
          </footer>
        ) : null}
      </div>
    </div>,
    document.body,
  );
}
