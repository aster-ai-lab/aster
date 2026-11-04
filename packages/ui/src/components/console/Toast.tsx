import type { ReactNode } from 'react'
import { cn } from '../../lib/cn'
import { Alert } from '../primitives/alert'
import {
  ToastProvider as PrimitiveToastProvider,
  useToasts,
  type ToastConfig,
} from '../primitives/toast'

export type ToastTone = 'info' | 'success' | 'danger'

export type ToastItem = {
  id: string
  title: string
  description?: string
  tone?: ToastTone
  /** Optional operation / evidence id for audit jump */
  operationId?: string
  onOperationClick?: (operationId: string) => void
  actionLabel?: string
  onAction?: () => void
}

type ToastApi = {
  push: (toast: Omit<ToastItem, 'id'> & { id?: string; durationMs?: number }) => string
  dismiss: (id: string) => void
}

const toneToMethod = {
  info: 'info',
  success: 'success',
  danger: 'error',
} as const

/**
 * Console toast API on the shared Arc/primitives toast store + viewport.
 * Prefer `useToasts` / module `success`/`error` helpers for new Studio surfaces.
 */
export function useToast(): ToastApi {
  const toasts = useToasts()
  return {
    push: (toast) => {
      const method = toneToMethod[toast.tone ?? 'info']
      const description: ReactNode =
        toast.description || toast.operationId ? (
          <>
            {toast.description ? <span className="block">{toast.description}</span> : null}
            {toast.operationId ? (
              <button
                type="button"
                className="ui-toast__link mt-1 text-xs underline-offset-2 hover:underline"
                onClick={() => toast.onOperationClick?.(toast.operationId!)}>
                Operation {toast.operationId}
              </button>
            ) : null}
          </>
        ) : undefined
      const config: ToastConfig = {
        key: toast.id,
        title: toast.title,
        description,
        timeout: toast.durationMs,
        action: toast.onAction
          ? { label: toast.actionLabel ?? 'Retry', onClick: toast.onAction }
          : undefined,
      }
      return toasts[method](config)
    },
    dismiss: (id) => toasts.closeToast(id),
  }
}

/**
 * Hosts the shared toast viewport. Duration defaults are applied per `push` call.
 */
export function ToastProvider({
  children,
  durationMs: _durationMs = 3600,
}: {
  children: ReactNode
  /** @deprecated Per-toast `durationMs` on `push` wins; kept for call-site compatibility. */
  durationMs?: number
}) {
  return <PrimitiveToastProvider>{children}</PrimitiveToastProvider>
}

export type InlineBannerProps = {
  tone?: ToastTone
  title: string
  description?: string
  action?: ReactNode
  onDismiss?: () => void
  className?: string
}

function DismissIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M7 7l10 10M17 7 7 17"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
    </svg>
  )
}

/**
 * Inline status banner on shared Arc Alert. Prefer `Alert` for new product surfaces.
 */
export function InlineBanner({
  tone = 'info',
  title,
  description,
  action,
  onDismiss,
  className = '',
}: InlineBannerProps) {
  return (
    <Alert
      tone={tone === 'danger' ? 'danger' : tone === 'success' ? 'success' : 'info'}
      title={title}
      description={description}
      showIcon
      className={cn(className)}
      action={
        (action || onDismiss) ? (
          <div className="flex items-center gap-2">
            {action}
            {onDismiss ? (
              <button
                type="button"
                className="ui-toast__close"
                aria-label="Dismiss"
                onClick={onDismiss}>
                <DismissIcon />
              </button>
            ) : null}
          </div>
        ) : undefined
      }
    />
  )
}
