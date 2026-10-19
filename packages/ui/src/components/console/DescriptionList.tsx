import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Card } from './Card';

export type DescriptionItem = {
  label: string;
  value: ReactNode;
};

export type DescriptionListProps = {
  items: DescriptionItem[];
  className?: string;
};

export function DescriptionList({
  items,
  className = '',
}: DescriptionListProps) {
  return (
    <Card
      as="dl"
      className={cn('divide-y-0', className)}
    >
      {items.map((item) => (
        <div
          key={item.label}
          className="grid min-h-9 grid-cols-[120px_1fr] items-center gap-3 px-4 py-2 text-sm [&+&]:border-t [&+&]:border-[color:var(--color-hairline)]"
        >
          <dt className="text-ink-faint">{item.label}</dt>
          <dd className="min-w-0 text-ink">{item.value}</dd>
        </div>
      ))}
    </Card>
  );
}

/** Alias */
export const KeyValue = DescriptionList;
