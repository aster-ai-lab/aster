import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { Input } from '../primitives/input';

export type CommandItem = {
  id: string;
  label: string;
  hint?: string;
  keywords?: string;
  onSelect: () => void;
};

export type CommandPaletteProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  items: CommandItem[];
  placeholder?: string;
  emptyText?: string;
};

export function CommandPalette({
  open,
  onOpenChange,
  items,
  placeholder = 'Type a command…',
  emptyText = 'No commands',
}: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((item) =>
      `${item.label} ${item.hint ?? ''} ${item.keywords ?? ''}`
        .toLowerCase()
        .includes(q),
    );
  }, [items, query]);

  useEffect(() => {
    if (!open) {
      setQuery('');
      setActive(0);
    }
  }, [open]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, onOpenChange]);

  if (!open || typeof document === 'undefined') return null;

  return createPortal(
    <div className="ui-dialog-root" role="presentation">
      <button
        type="button"
        className="ui-dialog-backdrop"
        aria-label="Close"
        onClick={() => onOpenChange(false)}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        className="ui-command-panel"
      >
        <Input
          autoFocus
          value={query}
          placeholder={placeholder}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive((i) => Math.min(i + 1, filtered.length - 1));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive((i) => Math.max(i - 1, 0));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              const item = filtered[active];
              if (item) {
                item.onSelect();
                onOpenChange(false);
              }
            } else if (e.key === 'Escape') {
              onOpenChange(false);
            }
          }}
        />
        <ul className="mt-2 flex max-h-72 flex-col gap-0.5 overflow-auto" role="listbox">
          {filtered.length === 0 ? (
            <li className="px-2.5 py-3 text-[13px] text-ink-faint">{emptyText}</li>
          ) : (
            filtered.map((item, i) => (
              <li key={item.id}>
                <button
                  type="button"
                  role="option"
                  aria-selected={i === active}
                  className={cn(
                    'flex h-9 w-full items-center justify-between rounded-lg px-2.5 text-left text-[13px]',
                    i === active ? 'bg-surface font-medium text-ink' : 'text-ink-muted hover:bg-surface-2',
                  )}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => {
                    item.onSelect();
                    onOpenChange(false);
                  }}
                >
                  <span>{item.label}</span>
                  {item.hint ? (
                    <span className="text-xs text-ink-faint">{item.hint}</span>
                  ) : null}
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>,
    document.body,
  );
}
