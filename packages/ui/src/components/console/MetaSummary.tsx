import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

export type MetaSummaryItem = {
  id: string;
  label: string;
  value: ReactNode;
  icon?: ReactNode;
};

export type MetaSummaryProps = {
  items: MetaSummaryItem[];
  className?: string;
  /** Accessible name for the summary group */
  'aria-label'?: string;
};

/**
 * Compact directory summary row — “负责人 · 成员 N · 子部门 N”.
 * Prefer this over a second Stat strip inside Master–Detail panes.
 */
export function MetaSummary({
  items,
  className = '',
  'aria-label': ariaLabel = 'Summary',
}: MetaSummaryProps) {
  if (items.length === 0) return null;
  return (
    <ul
      className={cn('ui-meta-summary', className)}
      aria-label={ariaLabel}
    >
      {items.map((item, index) => (
        <li key={item.id} className="ui-meta-summary__item">
          {index > 0 ? (
            <span className="ui-meta-summary__sep" aria-hidden>
              ·
            </span>
          ) : null}
          {item.icon ? (
            <span className="ui-meta-summary__icon" aria-hidden>
              {item.icon}
            </span>
          ) : null}
          <span className="ui-meta-summary__label">{item.label}</span>
          <span className="ui-meta-summary__value">{item.value}</span>
        </li>
      ))}
    </ul>
  );
}
