import { cn } from '../../lib/cn';

export type PersonCellProps = {
  name: string;
  email?: string | null;
  /**
   * Leading mark for directory / leaders:
   * - `true` → auto initials
   * - string → custom text
   * - omit / false → no mark
   */
  mark?: boolean | string;
  size?: 'sm' | 'md';
  className?: string;
};

function initialsFrom(name: string, email?: string | null): string {
  const source = (name || email || '?').trim();
  const parts = source.split(/\s+/).filter(Boolean);
  if (parts.length >= 2) {
    return `${parts[0][0] ?? ''}${parts[1][0] ?? ''}`.toUpperCase();
  }
  return source.slice(0, 2).toUpperCase();
}

/** Primary name + optional secondary email for directory / membership tables. */
export function PersonCell({
  name,
  email,
  mark,
  size = 'md',
  className = '',
}: PersonCellProps) {
  const leading =
    mark === true
      ? initialsFrom(name, email)
      : typeof mark === 'string'
        ? mark
        : null;

  return (
    <div
      className={cn(
        'inline-flex min-w-0 items-center gap-2',
        size === 'sm' && 'gap-1.5',
        className,
      )}
    >
      {leading != null && leading !== '' ? (
        <span
          className={cn(
            'ui-person-cell__mark',
            size === 'sm' && 'ui-person-cell__mark--sm',
          )}
          aria-hidden
        >
          {leading}
        </span>
      ) : null}
      <div className="min-w-0">
        <div
          className={cn(
            'truncate font-medium text-ink',
            size === 'sm' && 'text-sm',
          )}
        >
          {name || email || '—'}
        </div>
        {email ? (
          <div
            className={cn(
              'truncate text-ink-faint',
              size === 'sm' ? 'text-[11px]' : 'text-xs',
            )}
          >
            {email}
          </div>
        ) : null}
      </div>
    </div>
  );
}
