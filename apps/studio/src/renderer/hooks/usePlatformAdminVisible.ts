import { useSyncExternalStore } from 'react'
import useSWR from 'swr'

import {
  canSeePlatformAdmin,
  listMemberOrgs,
  PERSONAL_CENTER,
  readCenterId,
  subscribeCenter
} from '@renderer/data/memberOrgs'

/** True only while the current center is the official team and the member administers its directory. */
export function usePlatformAdminVisible(): boolean {
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const personal = centerId === PERSONAL_CENTER
  const { data: orgs } = useSWR(personal ? null : 'studio/member-orgs', () => listMemberOrgs())
  return canSeePlatformAdmin(orgs?.find((org) => org.id === centerId))
}
