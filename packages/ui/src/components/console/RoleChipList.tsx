import { Badge } from '../primitives/badge';
import { cn } from '../../lib/cn';

export type RoleChip = {
  id: string;
  label: string;
};

export type RoleChipListProps = {
  items: RoleChip[];
  empty?: string;
  className?: string;
};

/** Compact AdminRole / permission key chips for tables and cards. */
export function RoleChipList({
  items,
  empty = '—',
  className = '',
}: RoleChipListProps) {
  if (items.length === 0) {
    return <span className={cn('text-ink-muted', className)}>{empty}</span>;
  }
  return (
    <div className={cn('flex flex-wrap gap-1', className)}>
      {items.map((item) => (
        <Badge key={item.id} tone="primary">
          {item.label}
        </Badge>
      ))}
    </div>
  );
}
