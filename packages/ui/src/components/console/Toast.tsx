import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';

export type ToastTone = 'info' | 'success' | 'danger';

export type ToastItem = {
  id: string;
  title: string;
  description?: string;
  tone?: ToastTone;
  /** Optional operation / evidence id for audit jump */
  operationId?: string;
  onOperationClick?: (operationId: string) => void;
  actionLabel?: string;
  onAction?: () => void;
};

type ToastApi = {
  push: (toast: Omit<ToastItem, 'id'> & { id?: string; durationMs?: number }) => string;
  dismiss: (id: string) => void;
};

const ToastContext = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast must be used within ToastProvider');
  }
  return ctx;
}

function ToneIcon({ tone }: { tone: ToastTone }) {
  if (tone === 'success') {
    return (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
        <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.75" />
        <path
          d="m8.5 12.2 2.4 2.4 4.6-5.2"
          stroke="currentColor"
          strokeWidth="1.75"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    );
  }
  if (tone === 'danger') {
    return (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
        <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.75" />
        <path d="M12 8v5" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
        <path d="M12 16.5h.01" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      </svg>
    );
  }
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.75" />
      <path d="M12 11v5.5" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
      <path d="M12 8h.01" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M7 7l10 10M17 7 7 17"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
    </svg>
  );
}

function ToastCard({
  item,
  onDismiss,
}: {
  item: ToastItem;
  onDismiss: (id: string) => void;
}) {
  const tone = item.tone ?? 'info';
  const [entered, setEntered] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [dragY, setDragY] = useState(0);
  const [dragging, setDragging] = useState(false);
  const leaveTimer = useRef<number | null>(null);
  const dragYRef = useRef(0);
  const dragRef = useRef<{
    pointerId: number;
    startY: number;
    lastY: number;
    lastAt: number;
    velocity: number;
  } | null>(null);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => setEntered(true));
    return () => window.cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    return () => {
      if (leaveTimer.current) window.clearTimeout(leaveTimer.current);
    };
  }, []);

  const dismiss = (offset = -48) => {
    if (leaving) return;
    setLeaving(true);
    const next = Math.min(offset, -48);
    dragYRef.current = next;
    setDragY(next);
    setDragging(false);
    leaveTimer.current = window.setTimeout(() => onDismiss(item.id), 200);
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (leaving || event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest('button, a, input, textarea, select')) return;
    dragRef.current = {
      pointerId: event.pointerId,
      startY: event.clientY,
      lastY: event.clientY,
      lastAt: event.timeStamp,
      velocity: 0,
    };
    setDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const dy = event.clientY - drag.startY;
    const next = Math.min(0, dy);
    const dt = Math.max(1, event.timeStamp - drag.lastAt);
    drag.velocity = (event.clientY - drag.lastY) / dt;
    drag.lastY = event.clientY;
    drag.lastAt = event.timeStamp;
    dragYRef.current = next;
    setDragY(next);
  };

  const endDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;
    const offset = dragYRef.current;
    const shouldDismiss = offset <= -48 || drag.velocity < -0.45;
    if (shouldDismiss) {
      dismiss(Math.min(offset, -72));
      return;
    }
    setDragging(false);
    dragYRef.current = 0;
    setDragY(0);
  };

  const opacity =
    leaving || dragY < 0
      ? Math.max(0, 1 + Math.min(0, dragY) / 96)
      : undefined;

  return (
    <div
      className={cn(
        'ui-toast-slot',
        leaving && 'is-leaving',
        dragging && 'is-dragging',
      )}
    >
      <div className="ui-toast-slot__inner">
        <div
          className={cn(
            'ui-toast',
            entered && 'is-entered',
            leaving && 'is-leaving',
            dragging && 'is-dragging',
            tone === 'success' && 'ui-toast--success',
            tone === 'danger' && 'ui-toast--danger',
            tone === 'info' && 'ui-toast--info',
          )}
          role={tone === 'danger' ? 'alert' : 'status'}
          style={{
            transform:
              entered || dragging || leaving
                ? `translateY(${leaving ? Math.min(dragY, -48) : dragY}px)`
                : undefined,
            opacity,
          }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
        >
          <span className="ui-toast__icon" aria-hidden>
            <ToneIcon tone={tone} />
          </span>
          <div className="ui-toast__body">
            <p className="ui-toast__title">{item.title}</p>
            {item.description ? (
              <p className="ui-toast__desc">{item.description}</p>
            ) : null}
            {item.operationId ? (
              <button
                type="button"
                className="ui-toast__link"
                onClick={() => item.onOperationClick?.(item.operationId!)}
              >
                Operation {item.operationId}
              </button>
            ) : null}
            {item.onAction ? (
              <button
                type="button"
                className="ui-toast__link"
                onClick={() => {
                  item.onAction?.();
                  dismiss();
                }}
              >
                {item.actionLabel ?? 'Retry'}
              </button>
            ) : null}
          </div>
          <button
            type="button"
            className="ui-toast__close"
            aria-label="Dismiss"
            onClick={() => dismiss()}
          >
            <CloseIcon />
          </button>
        </div>
      </div>
    </div>
  );
}

const MAX_TOASTS = 5;

export function ToastProvider({
  children,
  durationMs = 3600,
}: {
  children: ReactNode;
  durationMs?: number;
}) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const timers = useRef(new Map<string, number>());

  const clearTimer = useCallback((id: string) => {
    const timer = timers.current.get(id);
    if (timer) {
      window.clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const dismiss = useCallback(
    (id: string) => {
      clearTimer(id);
      setItems((prev) => prev.filter((t) => t.id !== id));
    },
    [clearTimer],
  );

  const push = useCallback(
    (toast: Omit<ToastItem, 'id'> & { id?: string; durationMs?: number }) => {
      const { durationMs: itemDuration, ...item } = toast;
      const id =
        item.id ??
        `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      setItems((prev) => {
        const withoutDup = prev.filter((t) => t.id !== id);
        const next = [{ ...item, id }, ...withoutDup];
        if (next.length <= MAX_TOASTS) return next;
        for (const drop of next.slice(MAX_TOASTS)) {
          clearTimer(drop.id);
        }
        return next.slice(0, MAX_TOASTS);
      });
      clearTimer(id);
      const timer = window.setTimeout(() => dismiss(id), itemDuration ?? durationMs);
      timers.current.set(id, timer);
      return id;
    },
    [clearTimer, dismiss, durationMs],
  );

  useEffect(() => {
    return () => {
      for (const timer of timers.current.values()) window.clearTimeout(timer);
      timers.current.clear();
    };
  }, []);

  const api = useMemo(() => ({ push, dismiss }), [push, dismiss]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      {typeof document !== 'undefined'
        ? createPortal(
            <div className="ui-toast-viewport" aria-live="polite">
              {items.map((item) => (
                <ToastCard key={item.id} item={item} onDismiss={dismiss} />
              ))}
            </div>,
            document.body,
          )
        : null}
    </ToastContext.Provider>
  );
}

export type InlineBannerProps = {
  tone?: ToastTone;
  title: string;
  description?: string;
  action?: ReactNode;
  onDismiss?: () => void;
  className?: string;
};

export function InlineBanner({
  tone = 'info',
  title,
  description,
  action,
  onDismiss,
  className = '',
}: InlineBannerProps) {
  return (
    <div
      className={cn(
        'flex items-start gap-2 rounded-lg border-0 px-3 py-2.5 shadow-[var(--shadow-card)]',
        tone === 'success' && 'bg-[#e8f6ef] text-success',
        tone === 'danger' && 'bg-rose-500/10 text-rose-700',
        tone === 'info' && 'bg-bg-elevated text-ink',
        className,
      )}
      role="status"
    >
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium">{title}</p>
        {description ? (
          <p className="mt-0.5 text-xs opacity-80">{description}</p>
        ) : null}
      </div>
      {action}
      {onDismiss ? (
        <button
          type="button"
          className="ui-toast__close"
          aria-label="Dismiss"
          onClick={onDismiss}
        >
          <CloseIcon />
        </button>
      ) : null}
    </div>
  );
}
