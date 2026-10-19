import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

export function PageHeader({
  eyebrow,
  title,
  titlePrefix,
  leading,
  titleAdornment,
  description,
  actions,
  className = '',
}: {
  eyebrow?: string;
  title: string;
  /** Owner shown before the name, in the repository `owner / name` heading. */
  titlePrefix?: string;
  leading?: ReactNode;
  titleAdornment?: ReactNode;
  description?: string;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-col gap-2 sm:flex-row sm:justify-between',
        titlePrefix ? 'sm:items-center' : 'sm:items-start',
        className,
      )}
    >
      <div className="min-w-0">
        {eyebrow ? (
          <p className="mb-1 text-xs text-ink-faint">
            {eyebrow}
          </p>
        ) : null}
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          {leading}
          <h1
            className={
              titlePrefix
                ? 'flex min-w-0 flex-wrap items-center gap-x-1.5 text-[20px] font-normal leading-tight text-ink'
                : 'text-2xl font-bold leading-tight tracking-[-0.01em] text-ink'
            }
          >
            {titlePrefix ? (
              <>
                <span className="max-w-[16rem] truncate text-ink-muted">{titlePrefix}</span>
                <span className="text-ink-faint" aria-hidden>
                  /
                </span>
                <span className="max-w-[24rem] truncate font-semibold">{title}</span>
              </>
            ) : (
              title
            )}
          </h1>
          {titleAdornment}
        </div>
        {description ? (
          <p className="mt-1 max-w-2xl text-[15px] leading-6 text-ink-faint">
            {description}
          </p>
        ) : null}
      </div>
      {actions ? (
        <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>
      ) : null}
    </div>
  );
}
