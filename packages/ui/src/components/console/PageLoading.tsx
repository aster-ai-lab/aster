import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useInRouterContext, useLocation } from 'react-router-dom';
import { cn } from '../../lib/cn';
import {
  claimPageLoading,
  commitPageLoading,
  releasePageLoadingClaim,
} from '../../hooks/useRoutePageLoading';

const GLOBAL_MIN_MS = 1000;
const CONTAIN_MIN_MS = 280;
const GLOBAL_FADE_MS = 320;
const CONTAIN_FADE_MS = 180;
const GLOBAL_HOLD_MS = 960;
const CONTAIN_HOLD_MS = 380;

type Phase = 'drawing' | 'leaving' | 'hidden';

export type PageLoadingVariant = 'global' | 'contain';

export type PageLoadingProps = {
  /** Accessible status label. */
  label?: string;
  /** Minimum visible duration before fade-out (ms). */
  minMs?: number;
  /**
   * `global` — fixed full-viewport overlay (portal). Once on boot/refresh.
   * `contain` — fills the nearest positioned ancestor. First entry of a page only;
   * `useRoutePageLoading` and `useShowPageLoading` skip later entries.
   */
  variant?: PageLoadingVariant;
  className?: string;
  /** Stay on screen until unmounted. The route flash still hides itself. */
  persistent?: boolean;
  /**
   * Play on this mount even when the route already showed PageLoading.
   * Opening a note inside a page uses this.
   */
  replay?: boolean;
};

/**
 * Page loading flash. The spinner is the same ink ring on every tenant.
 * A pathname plays it once. Another mount on that pathname renders nothing.
 * - First paint / hard refresh: `<PageLoading variant="global" />` (mount once).
 * - First entry of a page: `<PageLoading variant="contain" />` inside a
 *   `position: relative` container. Entering that page again does not mount it.
 */
export function PageLoading(props: PageLoadingProps = {}) {
  const routed = useInRouterContext();
  if (!routed) return <PageLoadingView {...props} pageKey={null} />;
  return <PageLoadingRouted {...props} />;
}

function PageLoadingRouted(props: PageLoadingProps) {
  const { pathname } = useLocation();
  const pageKey = useRef(pathname);
  return <PageLoadingView {...props} pageKey={pageKey.current} />;
}

function PageLoadingView({
  label = 'Loading',
  minMs,
  variant = 'global',
  className = '',
  persistent = false,
  replay = false,
  pageKey,
}: PageLoadingProps & { pageKey: string | null }) {
  const token = useRef<number | null | undefined>(undefined);
  if (token.current === undefined) {
    token.current = replay || pageKey === null ? -1 : claimPageLoading(pageKey);
  }
  const allowed = token.current !== null;

  useEffect(() => {
    const claim = token.current;
    if (pageKey === null || claim === null || claim === undefined || claim < 0) return;
    const timer = window.setTimeout(() => commitPageLoading(pageKey, claim), 0);
    return () => {
      window.clearTimeout(timer);
      releasePageLoadingClaim(pageKey, claim);
    };
  }, [pageKey]);

  const [phase, setPhase] = useState<Phase>('drawing');
  const resolvedMinMs =
    minMs ?? (variant === 'contain' ? CONTAIN_MIN_MS : GLOBAL_MIN_MS);
  const markSize = variant === 'contain' ? 56 : 72;
  const fadeMs = variant === 'contain' ? CONTAIN_FADE_MS : GLOBAL_FADE_MS;
  const isContain = variant === 'contain';

  useEffect(() => {
    if (persistent) return;
    const floor = isContain ? CONTAIN_HOLD_MS : GLOBAL_HOLD_MS;
    const hold = Math.max(resolvedMinMs, floor);

    const leaveTimer = window.setTimeout(() => {
      setPhase('leaving');
    }, hold);

    const hideTimer = window.setTimeout(() => {
      setPhase('hidden');
    }, hold + fadeMs);

    return () => {
      window.clearTimeout(leaveTimer);
      window.clearTimeout(hideTimer);
    };
  }, [resolvedMinMs, isContain, fadeMs, persistent]);

  if (!allowed || phase === 'hidden') return null;

  const node = (
    <div
      className={cn(
        'page-loader',
        variant === 'contain' ? 'page-loader--contain' : 'page-loader--global',
        persistent && 'page-loader--persistent',
        phase === 'leaving' && 'is-leaving',
        className,
      )}
      role="status"
      aria-live="polite"
      aria-label={label}
      aria-busy={phase === 'drawing'}
    >
      <div className="page-loader-spinner-wrap">
        <svg
          className="page-loader-spinner"
          viewBox="0 0 48 48"
          width={markSize}
          height={markSize}
          aria-hidden
        >
          <circle
            cx="24"
            cy="24"
            r="18"
            fill="none"
            stroke="currentColor"
            strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray="28 85"
          />
        </svg>
      </div>
    </div>
  );

  if (variant === 'contain') return node;
  return createPortal(node, document.body);
}

/** @deprecated Prefer `PageLoading`. */
export const PageLoader = PageLoading;
