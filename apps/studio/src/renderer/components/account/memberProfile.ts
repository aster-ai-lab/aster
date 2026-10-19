import { useSyncExternalStore } from 'react'

import type { MemberAccount } from '@renderer/data/memberOrgs'

/** 最近一次从 OS 读到或保存的个人资料，ActivityBar 头像与切换中心共用。 */
let current: MemberAccount | null = null
const listeners = new Set<() => void>()
const MEMBER_SCOPE_CHANGED_EVENT = 'a3s:member-scope-changed'

export function publishMemberProfile(next: MemberAccount | null) {
  const previousId = current?.id ?? null
  current = next
  if (previousId !== (next?.id ?? null) && typeof window !== 'undefined') {
    window.dispatchEvent(new Event(MEMBER_SCOPE_CHANGED_EVENT))
  }
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useMemberProfile(): MemberAccount | null {
  return useSyncExternalStore(
    subscribe,
    () => current,
    () => null
  )
}
