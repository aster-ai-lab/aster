import type { Dispatch, SetStateAction } from 'react'

import { HOME_APPLET_SLUG } from '@renderer/pages/agents/homeApplets'
import { toastNotOpen } from '@renderer/pages/agents/notOpenToast'
import { openWorkbenchAppletWindow } from '@renderer/pages/agents/openWorkbenchAppletWindow'
import { INBOX_APPLET_ID } from '@renderer/pages/agents/WorkbenchApplets'
import { installCapability, listCapabilities, type AppletContract } from '@renderer/pages/capabilities/capabilitiesData'
import { pluginInstallOverlay, rememberPluginInstall } from '@renderer/pages/capabilities/pluginInstallSync'
import { toast } from '@renderer/services/toast'

export const ORG_APPLICATIONS = [
  'approval',
  'leave',
  'travel',
  'expense',
  'meeting',
  'seal',
  'probation',
  'clock',
  'ai-resource'
] as const
export type OrgApplication = (typeof ORG_APPLICATIONS)[number]
export type OrgHomeApp = OrgApplication | typeof INBOX_APPLET_ID

export const ORG_FAVORITES_KEY = 'org.workbench.favorites.v1'
const EMPTY_FAVORITES: OrgHomeApp[] = []
export const DEFAULT_ORG_FAVORITES: OrgHomeApp[] = ['inbox', ...ORG_APPLICATIONS]

/** Installed office packages. Org-only windows open without one. */
const OFFICE_PACKAGE_SLUG: Partial<Record<OrgApplication, string>> = {
  approval: HOME_APPLET_SLUG.approval,
  leave: HOME_APPLET_SLUG.leave,
  travel: HOME_APPLET_SLUG.travel,
  expense: HOME_APPLET_SLUG.expense,
  meeting: HOME_APPLET_SLUG.meeting,
  seal: HOME_APPLET_SLUG.seal
}

const listeners = new Set<() => void>()
const snapshots = new Map<string, { raw: string | null; value: OrgHomeApp[] }>()

export function isOrgHomeApp(id: string): id is OrgHomeApp {
  return id === INBOX_APPLET_ID || (ORG_APPLICATIONS as readonly string[]).includes(id)
}

export function subscribeOrgFavorites(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

function notifyOrgFavorites() {
  for (const listener of listeners) listener()
}

function storageRaw(orgId: string): string | null {
  try {
    return localStorage.getItem(`${ORG_FAVORITES_KEY}:${orgId}`)
  } catch {
    return null
  }
}

function parseOrgFavorites(raw: string | null): OrgHomeApp[] {
  if (raw == null) return DEFAULT_ORG_FAVORITES
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return DEFAULT_ORG_FAVORITES
    const ids = [...new Set(parsed.filter((id): id is OrgHomeApp => typeof id === 'string' && isOrgHomeApp(id)))]
    return ids.length === 0 ? EMPTY_FAVORITES : ids
  } catch {
    return DEFAULT_ORG_FAVORITES
  }
}

/** Stable snapshot for the selected team's pinned mini apps. */
export function readOrgFavorites(orgId: string): OrgHomeApp[] {
  const raw = storageRaw(orgId)
  const cached = snapshots.get(orgId)
  if (cached && cached.raw === raw) return cached.value
  const value = parseOrgFavorites(raw)
  snapshots.set(orgId, { raw, value })
  return value
}

export function writeOrgFavorites(orgId: string, ids: readonly string[]) {
  const value = [...new Set(ids.filter(isOrgHomeApp))]
  const raw = JSON.stringify(value)
  try {
    localStorage.setItem(`${ORG_FAVORITES_KEY}:${orgId}`, raw)
  } catch {
    // Private browsing can reject storage. The in-memory order still stays on screen.
  }
  snapshots.set(orgId, { raw, value })
  notifyOrgFavorites()
}

export async function openOrgHomeApp(
  id: OrgHomeApp,
  title: string,
  t: (key: string) => string,
  setPending?: Dispatch<SetStateAction<OrgHomeApp | null>>
) {
  let contract: AppletContract | undefined
  const slug = id === INBOX_APPLET_ID ? undefined : OFFICE_PACKAGE_SLUG[id]
  if (slug) {
    setPending?.(id)
    try {
      if (pluginInstallOverlay(slug) !== true) {
        const page = await listCapabilities({ kind: 'plugin', pageSize: 100 })
        const entry = page.items.find((item) => item.slug === slug)
        contract = entry?.applet
        if (entry?.installed !== true) {
          await installCapability(slug, globalThis.fetch)
          rememberPluginInstall(slug, true)
        }
      }
    } catch {
      toast.error(t('workbench.applet.install_failed'))
    } finally {
      setPending?.((current) => (current === id ? null : current))
    }
  }
  const opened = contract
    ? await openWorkbenchAppletWindow(id, title, contract)
    : await openWorkbenchAppletWindow(id, title)
  if (!opened) toastNotOpen(t)
}
