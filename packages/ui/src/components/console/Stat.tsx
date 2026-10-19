import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Card } from './Card';

export function Stat({
  label,
  value,
  hint,
  className = '',
}: {
  label: string;
  value: ReactNode;
  hint?: string;
  className?: string;
}) {
  return (
    <Card className={cn('min-w-0 px-4 py-3', className)}>
      <p className="truncate text-xs text-ink-faint">{label}</p>
      <p className="mt-1 text-xl font-semibold tabular-nums leading-none tracking-[-0.02em] text-ink">
        {value}
      </p>
      {hint ? (
        <p className="mt-1 truncate text-xs text-ink-faint">{hint}</p>
      ) : null}
    </Card>
  );
}
