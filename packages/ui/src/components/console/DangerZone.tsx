import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type DangerZoneProps = {
  title?: string;
  description?: string;
  actionLabel: string;
  onAction: () => void;
  disabled?: boolean;
  children?: ReactNode;
  className?: string;
};

/**
 * Destructive region — DESIGN semantic-error for emphasis;
 * action stays outline (black remains the only filled CTA).
 */
export function DangerZone({
  title = 'Danger zone',
  description,
  actionLabel,
  onAction,
  disabled,
  children,
  className = '',
}: DangerZoneProps) {
  return (
    <section
      className={cn(
        'rounded-xl border border-border bg-bg-elevated px-4 py-3.5',
        className,
      )}
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-ink">
            {title}
          </h3>
          {description ? (
            <p className="mt-1 max-w-xl text-xs leading-5 text-ink-faint">{description}</p>
          ) : null}
          {children}
        </div>
        <Button
          size="sm"
          variant="outline"
          className="!text-[color:var(--color-danger-text)]"
          disabled={disabled}
          onClick={onAction}
        >
          {actionLabel}
        </Button>
      </div>
    </section>
  );
}
