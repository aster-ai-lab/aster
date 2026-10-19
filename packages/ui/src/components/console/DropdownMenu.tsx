import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';

export type DropdownMenuItem = {
  id: string;
  label: string;
  icon?: ReactNode;
  tone?: 'default' | 'danger';
  disabled?: boolean;
  /** Visual separator before this item (org settings groups). */
  separator?: boolean;
  onSelect?: () => void;
};

export type DropdownMenuProps = {
  trigger: ReactNode;
  items: DropdownMenuItem[];
  align?: 'start' | 'end';
  className?: string;
  'aria-label'?: string;
};

/**
 * Anchored menu — portaled to body so table overflow does not clip it.
 */
export function DropdownMenu({
  trigger,
  items,
  align = 'end',
  className = '',
  'aria-label': ariaLabel = 'More actions',
}: DropdownMenuProps) {
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ top: number; left: number } | null>(
    null,
  );
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const menuId = useId();

  function updatePosition() {
    const triggerEl = rootRef.current;
    const menuEl = menuRef.current;
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    const menuWidth = menuEl?.offsetWidth ?? 160;
    const menuHeight = menuEl?.offsetHeight ?? 0;
    const gap = 4;
    let left =
      align === 'end' ? rect.right - menuWidth : rect.left;
    left = Math.max(8, Math.min(left, window.innerWidth - menuWidth - 8));
    let top = rect.bottom + gap;
    if (top + menuHeight > window.innerHeight - 8 && rect.top > menuHeight) {
      top = rect.top - menuHeight - gap;
    }
    setCoords({ top, left });
  }

  useLayoutEffect(() => {
    if (!open) {
      setCoords(null);
      return;
    }
    updatePosition();
    const onScroll = () => updatePosition();
    window.addEventListener('resize', onScroll);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      window.removeEventListener('resize', onScroll);
      window.removeEventListener('scroll', onScroll, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, align, items.length]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className={cn('relative inline-flex', className)}>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        aria-label={ariaLabel}
        className="inline-flex"
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
      >
        {trigger}
      </button>
      {open && typeof document !== 'undefined'
        ? createPortal(
            <ul
              ref={menuRef}
              id={menuId}
              role="menu"
              style={
                coords
                  ? {
                      position: 'fixed',
                      top: coords.top,
                      left: coords.left,
                      zIndex: 90,
                    }
                  : { position: 'fixed', top: -9999, left: -9999, zIndex: 90 }
              }
              className="min-w-[10rem] rounded-xl border border-[color:var(--color-border)] bg-bg-elevated p-1.5 shadow-[var(--shadow-elevated)]"
            >
              {items.map((item) => (
                <li key={item.id} role="none">
                  {item.separator ? (
                    <div
                      role="separator"
                      className="mx-1 my-1 h-px bg-[color:var(--color-hairline)]"
                    />
                  ) : null}
                  <button
                    type="button"
                    role="menuitem"
                    disabled={item.disabled}
                    className={cn(
                      'flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] outline-none hover:bg-surface-2 focus-visible:shadow-[inset_0_0_0_2px_rgba(0,0,0,0.1)] disabled:opacity-40 [&_svg]:size-4',
                      item.tone === 'danger'
                        ? 'text-[color:var(--color-danger-text)]'
                        : 'text-ink-muted hover:text-ink',
                    )}
                    onClick={(e) => {
                      e.stopPropagation();
                      item.onSelect?.();
                      setOpen(false);
                    }}
                  >
                    {item.icon ? (
                      <span className="inline-flex shrink-0 opacity-80">
                        {item.icon}
                      </span>
                    ) : null}
                    {item.label}
                  </button>
                </li>
              ))}
            </ul>,
            document.body,
          )
        : null}
    </div>
  );
}
