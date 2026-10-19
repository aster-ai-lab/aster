import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type WizardStep = {
  id: string;
  title: string;
  description?: string;
};

export type StepsProps = {
  steps: WizardStep[];
  currentId: string;
  className?: string;
  indexLabel?: (index: number) => string;
};

export function Steps({ steps, currentId, className = '', indexLabel }: StepsProps) {
  const idx = Math.max(
    0,
    steps.findIndex((s) => s.id === currentId),
  );
  return (
    <ol className={cn('flex flex-wrap gap-3', className)}>
      {steps.map((step, i) => {
        const state =
          i < idx ? 'done' : i === idx ? 'current' : 'upcoming';
        return (
          <li
            key={step.id}
            className={cn(
              'flex min-w-[120px] flex-1 flex-col gap-0.5 rounded-lg px-3 py-2',
              state === 'current' && 'bg-surface',
              state === 'done' && 'bg-[color:var(--color-control-fill)]',
              state === 'upcoming' && 'opacity-60',
            )}
            aria-current={state === 'current' ? 'step' : undefined}
          >
            <span className="text-xs text-ink-faint">
              {indexLabel ? indexLabel(i + 1) : `Step ${i + 1}`}
            </span>
            <span className="text-sm font-medium text-ink">{step.title}</span>
          </li>
        );
      })}
    </ol>
  );
}

export type WizardShellProps = {
  steps: WizardStep[];
  currentId: string;
  children: ReactNode;
  onBack?: () => void;
  onNext?: () => void;
  nextLabel?: string;
  backLabel?: string;
  nextDisabled?: boolean;
  footerExtra?: ReactNode;
  className?: string;
  indexLabel?: (index: number) => string;
};

export function WizardShell({
  steps,
  currentId,
  children,
  onBack,
  onNext,
  nextLabel = 'Continue',
  backLabel = 'Back',
  nextDisabled,
  footerExtra,
  className = '',
  indexLabel,
}: WizardShellProps) {
  const idx = steps.findIndex((s) => s.id === currentId);
  const isFirst = idx <= 0;
  const isLast = idx >= steps.length - 1;

  return (
    <div className={cn('flex flex-col gap-5', className)}>
      <Steps steps={steps} currentId={currentId} indexLabel={indexLabel} />
      <div className="flex min-h-0 flex-col gap-5">{children}</div>
      <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
        <div>{footerExtra}</div>
        <div className="flex gap-2">
          {!isFirst && onBack ? (
            <Button variant="outline" size="sm" onClick={onBack}>
              {backLabel}
            </Button>
          ) : null}
          {onNext ? (
            <Button
              size="sm"
              disabled={nextDisabled}
              onClick={onNext}
            >
              {isLast ? nextLabel : nextLabel}
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
