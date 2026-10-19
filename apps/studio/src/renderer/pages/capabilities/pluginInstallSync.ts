/** Session overlay so the plugin page and the homepage share one install flag. */

import { PLUGIN_INSTALL_EVENT } from '@shared/types/pluginInstall'

export { PLUGIN_INSTALL_EVENT }

const overlays = new Map<string, boolean>()

export function rememberPluginInstall(slug: string, installed: boolean) {
  overlays.set(slug, installed)
  if (typeof window === 'undefined') return
  window.dispatchEvent(new CustomEvent(PLUGIN_INSTALL_EVENT, { detail: { slug, installed } }))
}

export function pluginInstallOverlay(slug: string): boolean | undefined {
  return overlays.get(slug)
}

export function resetPluginInstallOverlayForTests() {
  overlays.clear()
}
