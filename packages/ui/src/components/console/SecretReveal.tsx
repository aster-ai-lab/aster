import { useState } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';
import { Card } from './Card';
import { CopyButton } from '../primitives/copy-button';

export type SecretRevealProps = {
  /** Full secret value — only shown once until dismissed */
  value: string;
  label?: string;
  revealLabel?: string;
  hideLabel?: string;
  className?: string;
};

/**
 * One-shot secret display (PRINCIPLES invariant #3).
 * After hide, value cannot be revealed again in this instance.
 */
export function SecretReveal({
  value,
  label = 'Secret',
  revealLabel = 'Show once',
  hideLabel = 'Hide permanently',
  className = '',
}: SecretRevealProps) {
  const [phase, setPhase] = useState<'masked' | 'visible' | 'locked'>(
    'masked',
  );

  return (
    <Card className={cn('px-3 py-2', className)}>
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs font-medium text-ink-muted">{label}</p>
        <div className="flex items-center gap-1">
          {phase === 'masked' ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPhase('visible')}
            >
              {revealLabel}
            </Button>
          ) : null}
          {phase === 'visible' ? (
            <>
              <CopyButton value={value} />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setPhase('locked')}
              >
                {hideLabel}
              </Button>
            </>
          ) : null}
        </div>
      </div>
      <p className="mt-2 break-all font-mono text-sm text-ink">
        {phase === 'visible'
          ? value
          : phase === 'locked'
            ? '••••••••••••••••'
            : '•••••••• (hidden)'}
      </p>
      {phase === 'locked' ? (
        <p className="mt-1 text-xs text-ink-faint">
          Secret cannot be shown again. Rotate if lost.
        </p>
      ) : null}
    </Card>
  );
}
