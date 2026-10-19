import { useEffect, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';

/** Global loader hold + fade; contain must not mount before this. */
export const PAGE_LOADING_BOOT_MS = 1400;

const shellSeen = new Set<string>();
const pageSeen = new Set<string>();
/** Pathnames whose PageLoading has already started. */
const played = new Set<string>();
/** Claims that have not survived the first effect yet. Strict-mode remounts release these. */
const inflight = new Map<string, number>();
let claimSeq = 0;

/**
 * One PageLoading per pathname. The first render wins; a later mount on that
 * pathname renders nothing. Returns a token the caller commits after mount.
 */
export function claimPageLoading(pathname: string): number | null {
  const path = pageLoadingPath(pathname);
  if (!path || played.has(path) || inflight.has(path)) return null;
  const token = ++claimSeq;
  inflight.set(path, token);
  return token;
}

export function releasePageLoadingClaim(pathname: string, token: number) {
  const path = pageLoadingPath(pathname);
  if (inflight.get(path) === token) inflight.delete(path);
}

export function commitPageLoading(pathname: string, token: number) {
  const path = pageLoadingPath(pathname);
  if (inflight.get(path) !== token) return;
  inflight.delete(path);
  played.add(path);
  shellSeen.add(path);
}

function pageLoadingPlayed(pathname: string): boolean {
  return played.has(pageLoadingPath(pathname));
}

/** Stable page key: ignore a trailing slash. */
export function pageLoadingPath(pathname: string): string {
  return pathname.replace(/\/+$/, '') || '/';
}

/** Drop first-entry memory. Sign-out clears this with the shell keep-alive cache. */
export function clearPageLoadingSession() {
  shellSeen.clear();
  pageSeen.clear();
  played.clear();
  inflight.clear();
}

/**
 * Remount key for the shell's container `PageLoading`.
 * One flash per pathname. The first path is covered by global boot loading
 * unless `immediate` is set (shells that do not mount the global loader).
 */
export function useRoutePageLoading(
  pathname: string,
  options?: { immediate?: boolean },
): string | null {
  const immediate = options?.immediate ?? false;
  const initialPathRef = useRef(pageLoadingPath(pathname));
  const [bootDone, setBootDone] = useState(immediate);
  const [routeKey, setRouteKey] = useState<string | null>(null);

  useEffect(() => {
    if (immediate) return;
    const timer = window.setTimeout(() => {
      setBootDone(true);
    }, PAGE_LOADING_BOOT_MS);
    return () => window.clearTimeout(timer);
  }, [immediate]);

  useEffect(() => {
    if (!bootDone) return;
    const path = pageLoadingPath(pathname);
    if (path === initialPathRef.current || shellSeen.has(path) || played.has(path)) {
      shellSeen.add(path);
      played.add(path);
      setRouteKey(null);
      return;
    }
    setRouteKey(pathname);
    // Mark after this effect replay so a strict-mode rerun does not cancel the flash.
    window.setTimeout(() => {
      shellSeen.add(path);
    }, 0);
  }, [pathname, bootDone]);

  return routeKey;
}

/**
 * True only while this page is still on its first load.
 * A later mount of the same pathname, or an effect replay after the first load
 * finishes, stays false so the page does not play `PageLoading` again.
 * `slot` separates stacked loaders on one pathname (home vs. its usage block).
 */
export function useShowPageLoading(pending: boolean, slot = 'page'): boolean {
  const { pathname } = useLocation();
  const path = pageLoadingPath(pathname);
  const key = `${path}\0${slot}`;
  const startedAsFirst = useRef(!pageSeen.has(key) && !pageLoadingPlayed(path));
  const finished = useRef(false);

  useEffect(() => {
    if (!startedAsFirst.current) return;
    window.setTimeout(() => {
      pageSeen.add(key);
    }, 0);
  }, [key]);

  if (!pending) finished.current = true;
  return startedAsFirst.current && !finished.current && pending;
}
