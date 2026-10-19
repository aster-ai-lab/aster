import { useEffect, useSyncExternalStore } from 'react'

/** The Messages page's only data source: the shared IM store pointed at the selected org. */
import { listMemberOrgs, PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'

import { type ImSnapshot, type ImStore, sharedImStore } from './imStore'
import { activityBarUnread } from './imView'

let creatorLookup = 0

/** The open org shows the team ToBot only when this member created it. A failure hides the row. */
export function refreshTeamCreator(store: ImStore): Promise<void> {
  const ticket = ++creatorLookup
  const orgId = readCenterId()
  if (!orgId || orgId === PERSONAL_CENTER) {
    store.setTeamCreator(false)
    return Promise.resolve()
  }
  return listMemberOrgs()
    .then((orgs) => {
      if (ticket !== creatorLookup || readCenterId() !== orgId) return
      store.setTeamCreator(orgs.some((org) => org.id === orgId && org.creator === true))
    })
    .catch(() => {
      if (ticket === creatorLookup) store.setTeamCreator(false)
    })
}

function bindCenter(store: ImStore): () => void {
  const sync = () => {
    store.start(readCenterId())
    void refreshTeamCreator(store)
  }
  sync()
  return subscribeCenter(sync)
}

export function useIm(store: ImStore = sharedImStore()): { snapshot: ImSnapshot; store: ImStore } {
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot)

  useEffect(() => {
    const unsubscribe = bindCenter(store)
    const onVisible = () => {
      if (document.visibilityState !== 'hidden') store.onVisible()
    }
    document.addEventListener('visibilitychange', onVisible)
    window.addEventListener('focus', onVisible)
    return () => {
      unsubscribe()
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener('focus', onVisible)
      void store.open(null)
    }
  }, [store])

  return { snapshot, store }
}

/** Keeps the shared store connected from the app shell so unread arrives before the page is opened. */
export function useImUnread(store: ImStore = sharedImStore()): number {
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot)

  useEffect(() => bindCenter(store), [store])

  return activityBarUnread(snapshot)
}
