import type { AnchorHTMLAttributes, ReactNode } from 'react'

/**
 * Studio routes with TanStack Router. The shared button still imports
 * `Link` for console pages that pass `to`. This anchor is only reached
 * when a studio screen passes `to`.
 *
 * Shared page-loading reads the router. Studio has none, so those hooks
 * report a static page and the loader takes its non-router path.
 */
const STUDIO_LOCATION = { pathname: '/', search: '', hash: '', state: null, key: 'studio' }

export function Link({
  to,
  children,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { to: string; children?: ReactNode }) {
  return (
    <a href={to} {...props}>
      {children}
    </a>
  )
}

export function useInRouterContext(): boolean {
  return false
}

export function useLocation(): typeof STUDIO_LOCATION {
  return STUDIO_LOCATION
}
