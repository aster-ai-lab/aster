import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';
import { InlineBanner } from './Toast';

export type ErrorBannerProps = {
  title?: string;
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
  className?: string;
};

export function ErrorBanner({
  title = 'Something went wrong',
  message,
  onRetry,
  retryLabel = 'Retry',
  className = '',
}: ErrorBannerProps) {
  return (
    <InlineBanner
      tone="danger"
      title={title}
      description={message}
      className={className}
      action={
        onRetry ? (
          <Button variant="outline" size="sm" onClick={onRetry}>
            {retryLabel}
          </Button>
        ) : null
      }
    />
  );
}

export type ForbiddenPageProps = {
  title?: string;
  description?: string;
  permissionId?: string;
  action?: ReactNode;
  className?: string;
};

export function ForbiddenPage({
  title = 'Access denied',
  description = 'You do not have permission to view this resource.',
  permissionId,
  action,
  className = '',
}: ForbiddenPageProps) {
  return (
    <div
      className={cn(
        'mx-auto flex max-w-lg flex-col items-start gap-3 py-10',
        className,
      )}
      role="alert"
    >
      <div>
        <h1 className="text-xl font-medium text-ink">{title}</h1>
        <p className="mt-1.5 text-sm text-ink-muted">{description}</p>
        {permissionId ? (
          <p className="mt-2 font-mono text-xs text-ink-faint">
            Missing permission: {permissionId}
          </p>
        ) : null}
      </div>
      {action}
    </div>
  );
}
