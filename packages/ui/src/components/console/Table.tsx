import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Card } from './Card';

export function Table({
  columns,
  children,
  className = '',
}: {
  columns: string[];
  children: ReactNode;
  className?: string;
}) {
  return (
    <Card className={cn('overflow-x-auto', className)}>
      <table className="w-full min-w-[640px] border-collapse text-left text-sm">
        <thead className="bg-bg-elevated text-xs text-ink-faint">
          <tr>
            {columns.map((col) => (
              <th key={col} className="h-9 border-b border-[color:var(--color-hairline)] px-3 font-normal">
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="text-ink-muted [&>tr+tr]:border-t [&>tr+tr]:border-[color:var(--color-hairline)] [&>tr:hover]:bg-surface-2">{children}</tbody>
      </table>
    </Card>
  );
}
