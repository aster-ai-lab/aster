import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { MetaSummary, type MetaSummaryItem } from './MetaSummary';

export type ResourceMetaHeaderProps = {
  title: ReactNode;
  badge?: ReactNode;
  /** Free-form muted line (legacy). Prefer `summary` for directory panes. */
  meta?: ReactNode;
  /** Structured summary chips — avoids a second Stat strip. */
  summary?: MetaSummaryItem[];
  actions?: ReactNode;
  /**
   * `card` = bordered surface (page-level org strip).
   * `flush` = no outer card (already inside DirectorySplit detail).
   */
  variant?: 'card' | 'flush';
  className?: string;
};

/** Title + status badge + meta/summary + secondary actions (org / resource heads). */
export function ResourceMetaHeader({
  title,
  badge,
  meta,
  summary,
  actions,
  variant = 'card',
  className = '',
}: ResourceMetaHeaderProps) {
  return (
    <header
      className={cn(
        'ui-resource-meta',
        variant === 'flush' ? 'ui-resource-meta--flush' : 'ui-card',
        className,
      )}
    >
      <div className="ui-resource-meta__main">
        <div className="ui-resource-meta__title-row">
          <h2 className="ui-resource-meta__title">{title}</h2>
          {badge}
        </div>
        {summary && summary.length > 0 ? (
          <MetaSummary items={summary} />
        ) : meta ? (
          <p className="ui-resource-meta__meta">{meta}</p>
        ) : null}
      </div>
      {actions ? (
        <div className="ui-resource-meta__actions">{actions}</div>
      ) : null}
    </header>
  );
}
