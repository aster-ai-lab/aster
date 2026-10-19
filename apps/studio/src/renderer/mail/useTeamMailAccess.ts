import useSWR from 'swr'

import { listMemberOrgs, orgAllows } from '@renderer/data/memberOrgs'

export function useTeamMailAccess(orgId?: string) {
  const { data: teams, error } = useSWR(orgId ? 'studio/member-orgs' : null, () => listMemberOrgs())
  const team = error ? undefined : teams?.find((row) => row.id === orgId)
  const read = team?.canReadMail === true && orgAllows(team, 'read', 'MailMessage')
  const create = team?.canCreateMail === true && orgAllows(team, 'create', 'MailMessage')
  const send = team?.canSendMail === true && orgAllows(team, 'send', 'MailMessage')
  const update = read && team?.canUpdateMail === true && orgAllows(team, 'update', 'MailMessage')
  const remove = read && team?.canDeleteMail === true && orgAllows(team, 'delete', 'MailMessage')
  const pick = team?.canReadMailRecipients === true && orgAllows(team, 'read', 'MailRecipient')
  return {
    ready: teams !== undefined || !!error,
    read,
    create,
    send,
    update,
    remove,
    pick,
    authority: JSON.stringify([orgId, read, create, send, update, remove, pick, team?.dataScope])
  }
}
