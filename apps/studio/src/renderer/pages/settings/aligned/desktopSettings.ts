import { preferenceService } from '@data/PreferenceService'
import { studioProductName } from '@renderer/tenantBrand'

import { version as packageVersion } from '../../../../../package.json'

export type DesktopPermissions = {
  computerUse: boolean
  agentControl: boolean
}

export function readDesktopPermissions(): DesktopPermissions {
  return {
    computerUse: preferenceService.getCachedValue('app.permissions.computer_use') === true,
    agentControl: preferenceService.getCachedValue('app.permissions.agent_control') === true
  }
}

export async function writeDesktopPermission(key: 'computerUse' | 'agentControl', value: boolean): Promise<void> {
  await preferenceService.set(
    key === 'computerUse' ? 'app.permissions.computer_use' : 'app.permissions.agent_control',
    value
  )
}

/** Product name comes from the launch tenant. Version is the packaged app version. */
export function fallbackAbout(): { name: string; version: string } {
  return { name: studioProductName(), version: packageVersion }
}
