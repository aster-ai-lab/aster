import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Badge, type BadgeTone } from '../primitives/badge';

export type TimelineLevel = 'info' | 'warn' | 'error' | 'success';

export type TimelineItem = {
  id: string;
  title: string;
  description?: string;
  timestamp?: string;
  level?: TimelineLevel;
  href?: string;
  meta?: ReactNode;
};

const levelTone: Record<TimelineLevel, BadgeTone> = {
  info: 'neutral',
  warn: 'warn',
  error: 'danger',
  success: 'success',
};

export type TimelineProps = {
  items: TimelineItem[];
  className?: string;
  empty?: ReactNode;
};

export function Timeline({ items, className = '', empty }: TimelineProps) {
  if (items.length === 0) return <>{empty ?? null}</>;

  return (
    <ol className={cn('relative space-y-0', className)}>
      {items.map((item, i) => (
        <li key={item.id} className="relative flex gap-3 pb-4 last:pb-0">
          {i < items.length - 1 ? (
            <span
              className="absolute left-[5px] top-4 bottom-0 w-px bg-[color:var(--color-hairline)]"
              aria-hidden
            />
          ) : null}
          <span
            className="mt-1.5 size-[11px] shrink-0 rounded-full border-2 border-[color:var(--color-rail-plate)] bg-bg-elevated"
            aria-hidden
          />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              {item.level ? (
                <Badge tone={levelTone[item.level]}>{item.level}</Badge>
              ) : null}
              {item.href ? (
                <a
                  href={item.href}
                  className="text-sm font-medium text-ink hover:opacity-80"
                >
                  {item.title}
                </a>
              ) : (
                <p className="text-sm font-medium text-ink">{item.title}</p>
              )}
              {item.timestamp ? (
                <time className="text-xs text-ink-faint">{item.timestamp}</time>
              ) : null}
            </div>
            {item.description ? (
              <p className="mt-1 text-sm text-ink-muted">{item.description}</p>
            ) : null}
            {item.meta}
          </div>
        </li>
      ))}
    </ol>
  );
}
