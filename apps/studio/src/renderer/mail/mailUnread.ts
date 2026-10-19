import { useEffect, useSyncExternalStore } from 'react'

import { readCenterId, subscribeCenter, PERSONAL_CENTER } from '@renderer/data/memberOrgs'

import { loadMailbox } from './mailApi'
import { useTeamMailAccess } from './useTeamMailAccess'

let unread = 0
const listeners = new Set<() => void>()

export function publishMailUnread(next: number): void {
  const value = Number.isFinite(next) && next > 0 ? Math.floor(next) : 0
  if (value === unread) return
  unread = value
  listeners.forEach((listener) => listener())
}

export function readMailUnread(): number {
  return unread
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Current unread count. This does not poll. */
export function useMailUnreadCount(): number {
  return useSyncExternalStore(subscribe, readMailUnread, () => 0)
}

/** Unread count for the signed-in team. Tests do not start the polling timer. */
export function useMailUnread(): number {
  const count = useMailUnreadCount()
  const center = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const orgId = center === PERSONAL_CENTER ? undefined : center
  const access = useTeamMailAccess(orgId)
  useEffect(() => {
    if (import.meta.env.MODE === 'test') return
    let stopped = false
    let epoch = 0
    publishMailUnread(0)
    if (!orgId || !access.read) return
    const refresh = async () => {
      const request = ++epoch
      try {
        const mailbox = await loadMailbox(orgId)
        if (!stopped && epoch === request) publishMailUnread(mailbox.unread)
      } catch {
        if (!stopped && epoch === request) publishMailUnread(0)
      }
    }
    const tick = () => {
      if (!stopped) void refresh()
    }
    tick()
    const timer = window.setInterval(tick, 40_000)
    const onVisible = () => {
      if (document.visibilityState !== 'hidden') tick()
    }
    document.addEventListener('visibilitychange', onVisible)
    return () => {
      stopped = true
      window.clearInterval(timer)
      document.removeEventListener('visibilitychange', onVisible)
    }
  }, [orgId, access.authority, access.read])
  return count
}
