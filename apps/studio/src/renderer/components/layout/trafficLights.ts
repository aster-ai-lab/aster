import { isMac } from '@renderer/utils/platform'

/** Compact traffic lights sit inside the 60px icon rail, 16px from the content top.
 *  The macOS shell writes `--a3s-traffic-light-inset: 60px` and
 *  `--a3s-titlebar-height: 44px`. WKWebView leaves `env(titlebar-area-*)` at 0.
 *  A Chromium Window Controls Overlay value still wins. The 0px fallback keeps
 *  Linux and plain browsers from reserving a gutter they do not have.
 */
export const MACOS_TRAFFIC_LIGHT_INSET = 'max(env(titlebar-area-x, 0px), var(--a3s-traffic-light-inset, 0px))'

/** Electron reports darwin on `window.electron`. ToWork's webview does not, and the
 *  shell writes the inset variable before the page loads instead.
 */
export function hasNativeTrafficLightInset(): boolean {
  if (isMac) return true
  if (typeof document === 'undefined') return false
  const inset = document.documentElement.style.getPropertyValue('--a3s-traffic-light-inset').trim()
  return inset !== '' && inset !== '0' && inset !== '0px'
}

export const MACOS_TITLEBAR_HEIGHT = 'max(env(titlebar-area-height, 0px), var(--a3s-titlebar-height, 0px))'
