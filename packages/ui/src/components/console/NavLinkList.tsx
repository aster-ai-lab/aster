import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

export type NavLinkListItem = {
  id: string;
  label: string;
  meta?: ReactNode;
  disabled?: boolean;
  onSelect?: () => void;
};

export type NavLinkListProps = {
  items: NavLinkListItem[];
  empty?: ReactNode;
  className?: string;
  'aria-label'?: string;
};

/**
 * Compact one-column nav list — child departments, related resources, etc.
 * Replaces page-local `os-org__child-*` markup.
 */
export function NavLinkList({
  items,
  empty,
  className = '',
  'aria-label': ariaLabel,
}: NavLinkListProps) {
  if (items.length === 0) {
    return empty ? (
      <div className={cn('ui-nav-link-list__empty', className)}>{empty}</div>
    ) : null;
  }

  return (
    <ul
      className={cn('ui-nav-link-list', className)}
      aria-label={ariaLabel}
    >
      {items.map((item) => (
        <li key={item.id}>
          <button
            type="button"
            className="ui-nav-link-list__item"
            disabled={item.disabled}
            onClick={() => item.onSelect?.()}
          >
            <span className="ui-nav-link-list__label">{item.label}</span>
            {item.meta != null ? (
              <span className="ui-nav-link-list__meta">{item.meta}</span>
            ) : null}
          </button>
        </li>
      ))}
    </ul>
  );
}
