import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { listRoleAssignmentAudits, type OrgRoleAssignmentSnapshot } from './orgApi'
import { GRANTABLE } from './orgPermissions'
import { quietButtonClassName } from './orgUi'

const scopeLabels = {
  self: 'org.scope.self',
  department: 'org.scope.department',
  department_tree: 'org.scope.departmentTree',
  custom: 'org.scope.custom',
  custom_tree: 'org.scope.customTree',
  all: 'org.scope.all'
} as const

const adminChangeLabels = {
  keep: 'org.members.history.keep',
  grant: 'org.members.history.grant',
  revoke: 'org.members.history.revoke'
} as const

export function OrgMemberRoleHistory({
  orgId,
  userId,
  memberName,
  authorityKey,
  departmentNames,
  onClose
}: {
  orgId: string
  userId: string
  memberName: string
  authorityKey: string
  departmentNames: Map<string, string>
  onClose: () => void
}) {
  const { t, i18n } = useTranslation()
  const viewId = useId()
  const { data, error, isLoading, mutate } = useSWR(
    `studio/org-member-role-audit:${orgId}:${userId}:${authorityKey}:${viewId}`,
    () => listRoleAssignmentAudits(orgId, userId)
  )
  const history = error ? undefined : data
  const snapshot = (image: OrgRoleAssignmentSnapshot, label: string) => (
    <section aria-label={t(label)} className="min-w-0 space-y-1 text-[13px] text-[#3f3f3f]">
      <h3 className="m-0 text-[12px] font-medium text-[#8a8a8a]">{t(label)}</h3>
      <p className="m-0 font-medium">{image.role.name}</p>
      {image.role.description ? <p className="m-0 break-words">{image.role.description}</p> : null}
      <p className="m-0">
        {t(image.administrator ? 'org.members.history.administrator' : 'org.members.history.nonAdministrator')}
      </p>
      <p className="m-0 break-words">
        {t('org.members.col.department')}: {departmentNames.get(image.department_id) ?? image.department_id}
      </p>
      <p className="m-0">
        {t('org.scope.title')}: {t(scopeLabels[image.role.data_scope_kind])}
      </p>
      {image.role.scope_department_ids.length ? (
        <p className="m-0 break-words">
          {image.role.scope_department_ids.map((id) => departmentNames.get(id) ?? id).join('、')}
        </p>
      ) : null}
      <p className="m-0 text-[#8a8a8a]">
        {image.role.rules.some((rule) => rule.action === 'manage' && rule.subject === 'all')
          ? t('org.roles.history.allPermissions')
          : t('org.perm.baseline')}
      </p>
      <ul className="m-0 space-y-0.5 pl-4">
        {GRANTABLE.filter((permission) =>
          image.role.rules.some((rule) => rule.action === permission.action && rule.subject === permission.subject)
        ).map((permission) => (
          <li key={`${permission.action}:${permission.subject}`}>{t(permission.label)}</li>
        ))}
      </ul>
    </section>
  )
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}>
      <DialogContent size="xl" className="max-h-[70vh] overflow-y-auto">
        <DialogTitle>{`${t('org.members.history.title')} · ${memberName}`}</DialogTitle>
        <p className="m-0 text-[12px] text-[#8a8a8a]">{t('org.roles.history.latest')}</p>
        {error ? (
          <div role="alert" className="space-y-2 text-[13px] text-[#3f3f3f]">
            <p className="m-0">{t('org.roles.history.unavailable')}</p>
            <button
              type="button"
              className={quietButtonClassName()}
              onClick={() => void mutate().catch(() => undefined)}>
              {t('common.retry')}
            </button>
          </div>
        ) : isLoading || !history ? (
          <p role="status">{t('common.loading')}</p>
        ) : history.length === 0 ? (
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.roles.history.empty')}</p>
        ) : (
          <ol aria-label={t('org.members.history.title')} className="m-0 list-none space-y-4 p-0">
            {history.map((event) => (
              <li key={event.id} className="space-y-3 border-t border-[#efefef] pt-3">
                <div className="space-y-1">
                  <p className="m-0 text-[14px] font-medium text-[#171717]">
                    {t(
                      event.authority === 'assign_role' ? 'org.members.history.assigned' : 'org.members.history.updated'
                    )}
                  </p>
                  <time dateTime={event.occurred_at} className="text-[12px] text-[#8a8a8a]">
                    {new Date(event.occurred_at).toLocaleString(i18n.language)}
                  </time>
                  <p className="m-0 text-[12px] break-all text-[#8a8a8a]">
                    {t('org.roles.history.actor', { id: event.actor_member_id })}
                  </p>
                  <p className="m-0 text-[12px] text-[#8a8a8a]">{t(adminChangeLabels[event.admin_change])}</p>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  {snapshot(event.before, 'org.roles.history.before')}
                  {snapshot(event.after, 'org.roles.history.after')}
                </div>
              </li>
            ))}
          </ol>
        )}
      </DialogContent>
    </Dialog>
  )
}
