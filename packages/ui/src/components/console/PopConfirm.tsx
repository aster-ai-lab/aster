import {
  cloneElement,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent,
  type ReactElement,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type PopConfirmProps = {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: 'danger' | 'default';
  disabled?: boolean;
  onConfirm: () => void | Promise<void>;
  onOpenChange?: (open: boolean) => void;
  /** Open on mount. Used when the confirm is triggered from the keyboard rather than a click. */
  defaultOpen?: boolean;
  children: ReactElement;
  className?: string;
  /** Keep the panel inside this ancestor. It shrinks when the ancestor is narrower than 18.5rem. */
  boundary?: string;
};

type PanelPos = {
  top: number;
  left: number;
  width: number;
  placement: 'above' | 'below';
  arrowX: number;
};

const PANEL_WIDTH = '18.5rem';
const PANEL_GAP = 10;

/**
 * Native `appearance: button` ignores the CSS face in WKWebView after the
 * control is measured off-screen. Paint confirm actions as ordinary boxes.
 */
const actionFace = {
  appearance: 'none',
  WebkitAppearance: 'none',
} as const;

/** Place a confirm panel inside `bounds`, shrinking it when the box is narrower than the panel. */
export function fitConfirmPanel(
  trigger: Pick<DOMRect, 'top' | 'bottom' | 'left' | 'right' | 'width'>,
  panel: { width: number; height: number },
  bounds: Pick<DOMRect, 'left' | 'right' | 'top' | 'bottom'> | null,
  viewport: { width: number; height: number },
): PanelPos {
  const minLeft = (bounds?.left ?? 0) + 8;
  const maxRight = (bounds?.right ?? viewport.width) - 8;
  const maxWidth = Math.max(160, maxRight - minLeft);
  const natural = panel.width > 0 ? panel.width : 0;
  const width = natural > maxWidth ? maxWidth : natural;
  const height = panel.height;
  const belowTop = trigger.bottom + PANEL_GAP;
  const placement: PanelPos['placement'] =
    belowTop + height > viewport.height - 8 && trigger.top - height - PANEL_GAP > 8 ? 'above' : 'below';
  const top = placement === 'below' ? belowTop : Math.max(8, trigger.top - height - PANEL_GAP);
  const left = Math.min(Math.max(minLeft, trigger.right - width), Math.max(minLeft, maxRight - width));
  const arrowX = Math.min(width - 16, Math.max(12, trigger.left + trigger.width / 2 - left - 4));
  return { top, left, width, placement, arrowX };
}

function WarnIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M12 8v5" />
      <path d="M12 16.5h.01" />
    </svg>
  );
}

/**
 * Anchored confirm — for destructive row actions that should not open a modal.
 */
export function PopConfirm({
  title,
  description,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  tone = 'danger',
  disabled = false,
  onConfirm,
  onOpenChange,
  defaultOpen = false,
  children,
  className = '',
  boundary,
}: PopConfirmProps) {
  const [open, setOpen] = useState(defaultOpen);
  const [busy, setBusy] = useState(false);
  const [pos, setPos] = useState<PanelPos | null>(null);
  const triggerRef = useRef<HTMLSpanElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const panelId = useId();
  const descId = useId();

  const place = () => {
    const trigger = triggerRef.current;
    const panel = panelRef.current;
    if (!trigger || !panel) return;
    const rect = trigger.getBoundingClientRect();
    const bounds = boundary ? trigger.closest(boundary)?.getBoundingClientRect() ?? null : null;
    const fitted = fitConfirmPanel(
      rect,
      { width: panel.offsetWidth, height: panel.offsetHeight },
      bounds,
      { width: window.innerWidth, height: window.innerHeight },
    );
    if (fitted.width > 0 && fitted.width < panel.offsetWidth) {
      panel.style.width = `${fitted.width}px`;
      const refit = fitConfirmPanel(
        rect,
        { width: fitted.width, height: panel.offsetHeight },
        bounds,
        { width: window.innerWidth, height: window.innerHeight },
      );
      setPos(refit);
      return;
    }
    setPos(fitted);
  };

  useLayoutEffect(() => {
    if (!open) {
      setPos(null);
      return;
    }
    place();
    const frame = window.requestAnimationFrame(() => {
      if (tone !== 'danger') return;
      panelRef.current
        ?.querySelector<HTMLButtonElement>('.ui-popconfirm__actions button')
        ?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, tone, boundary]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (
        triggerRef.current?.contains(target) ||
        panelRef.current?.contains(target)
      ) {
        return;
      }
      changeOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') changeOpen(false);
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', place);
    window.addEventListener('scroll', place, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', place, true);
    };
  }, [open]);

  const changeOpen = (next: boolean) => {
    setOpen(next);
    onOpenChange?.(next);
  };

  const confirm = async () => {
    setBusy(true);
    try {
      await onConfirm();
      changeOpen(false);
    } catch {
      /* caller surfaces the error; keep the popover open */
    } finally {
      setBusy(false);
    }
  };

  const panel =
    open && typeof document !== 'undefined'
      ? createPortal(
          <div
            ref={panelRef}
            id={panelId}
            role="dialog"
            aria-label={title}
            aria-describedby={description ? descId : undefined}
            data-placement={pos?.placement ?? 'below'}
            data-tone={tone}
            className="ui-popconfirm fixed z-[90] box-border rounded-lg border border-[#e5e5e5] bg-white px-[0.95rem] pt-[0.9rem] pb-[0.8rem] text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)] dark:border-[#2f2f33] dark:bg-[#171717] dark:text-[#f5f5f5] dark:shadow-[0_16px_48px_rgba(0,0,0,0.55)]"
            style={{
              position: 'fixed',
              zIndex: 90,
              width: pos?.width ? `${pos.width}px` : PANEL_WIDTH,
              // Off-screen until placed. visibility:hidden makes WKWebView lay
              // out native buttons and then never paint them once the panel
              // is shown, so the actions keep their box and lose their face.
              ...(pos
                ? { top: pos.top, left: pos.left }
                : { top: -9999, left: 0, pointerEvents: 'none' as const }),
            }}
          >
            <span
              className={cn(
                'ui-popconfirm__arrow pointer-events-none absolute size-[0.55rem] bg-white dark:bg-[#171717]',
                (pos?.placement ?? 'below') === 'below'
                  ? '-top-[0.3rem] rotate-45 border-t border-l border-[#e5e5e5] dark:border-[#2f2f33]'
                  : '-bottom-[0.3rem] rotate-45 border-r border-b border-[#e5e5e5] dark:border-[#2f2f33]',
              )}
              style={{ left: pos?.arrowX ?? 16 }}
              aria-hidden
            />
            <div className="ui-popconfirm__body flex items-start gap-[0.55rem]">
              {tone === 'danger' ? (
                <span className="ui-popconfirm__icon mt-px grid size-5 shrink-0 place-items-center text-[#c23b3b]">
                  <WarnIcon />
                </span>
              ) : null}
              <div className="min-w-0">
                <p className="ui-popconfirm__title m-0 text-[13px] leading-[1.45] font-medium">{title}</p>
                {description ? (
                  <p id={descId} className="ui-popconfirm__desc m-0 mt-[0.2rem] text-[12px] leading-[1.45] text-[#8a8a8a]">
                    {description}
                  </p>
                ) : null}
              </div>
            </div>
            <div className="ui-popconfirm__actions mt-[0.85rem] flex justify-end gap-[0.4rem]">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                className="appearance-none"
                style={actionFace}
                disabled={busy}
                onClick={() => changeOpen(false)}
              >
                {cancelLabel}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="primary"
                className={
                  tone === 'danger'
                    ? 'ui-popconfirm__danger appearance-none !border-transparent !bg-[#c23b3b] !text-white hover:!bg-[#a83232]'
                    : 'appearance-none'
                }
                style={
                  tone === 'danger'
                    ? { ...actionFace, backgroundColor: '#c23b3b', color: '#ffffff', borderColor: 'transparent' }
                    : actionFace
                }
                disabled={busy}
                onClick={() => void confirm()}
              >
                {confirmLabel}
              </Button>
            </div>
          </div>,
          document.body,
        )
      : null;

  const activate = (event: { preventDefault(): void; stopPropagation(): void }) => {
    event.preventDefault();
    event.stopPropagation();
    if (disabled || busy) return;
    changeOpen(!open);
  };

  // The confirm state lives on the wrapper. A press on the inner control, including
  // an accessibility press, has to open it too. Stopping the event keeps one toggle.
  const clickable = children as ReactElement<{ onClick?: (event: MouseEvent<HTMLElement>) => void }>;
  const triggerChild = cloneElement(clickable, {
    onClick: (event: MouseEvent<HTMLElement>) => {
      clickable.props.onClick?.(event);
      if (!event.defaultPrevented) activate(event);
    },
  });

  return (
    <>
      <span ref={triggerRef} className={cn('inline-flex', className)} onClick={activate}>
        {triggerChild}
        <span className="sr-only" aria-expanded={open} aria-controls={panelId} />
      </span>
      {panel}
    </>
  );
}
