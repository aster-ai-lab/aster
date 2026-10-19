import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { listRolePolicyAudits, type OrgDepartment, type OrgRolePolicySnapshot } from './orgApi'
import { quietButtonClassName } from './orgUi'

const scopeLabels = {
  self: 'org.scope.self',
  department: 'org.scope.department',
  department_tree: 'org.scope.departmentTree',
  custom: 'org.scope.custom',
  custom_tree: 'org.scope.customTree',
  all: 'org.scope.all'
} as const

const actionLabels = {
  role_created: 'org.roles.history.created',
  role_updated: 'org.roles.history.updated',
  role_deleted: 'org.roles.history.deleted'
} as const

export function OrgRoleHistory({
  orgId,
  roleId,
  authorityKey,
  departments,
  permissionCatalog,
  onClose
}: {
  orgId: string
  roleId: string
  authorityKey: string
  departments: OrgDepartment[]
  permissionCatalog: { action: string; subject: string; label: string }[]
  onClose: () => void
}) {
  const { t, i18n } = useTranslation()
  const { data, error, isLoading, mutate } = useSWR(`studio/org-role-audit:${orgId}:${roleId}:${authorityKey}`, () =>
    listRolePolicyAudits(orgId, roleId)
  )
  const history = error ? undefined : data
  const policy = (snapshot: OrgRolePolicySnapshot | null, label: string) => (
    <section aria-label={t(label)} className="min-w-0 space-y-1 text-[13px] text-[#3f3f3f]">
      <h3 className="m-0 text-[12px] font-medium text-[#8a8a8a]">{t(label)}</h3>
      {snapshot ? (
        <>
          <p className="m-0 font-medium">{snapshot.name}</p>
          {snapshot.description ? <p className="m-0 break-words">{snapshot.description}</p> : null}
          <p className="m-0">
            {t('org.scope.title')}: {t(scopeLabels[snapshot.data_scope_kind])}
          </p>
          {snapshot.scope_department_ids.length ? (
            <p className="m-0 break-words">
              {snapshot.scope_department_ids
                .map((id) => departments.find((department) => department.id === id)?.name ?? id)
                .join('、')}
            </p>
          ) : null}
          <p className="m-0 text-[#8a8a8a]">
            {snapshot.rules.some((rule) => rule.action === 'manage' && rule.subject === 'all')
              ? t('org.roles.history.allPermissions')
              : t('org.perm.baseline')}
          </p>
          <ul className="m-0 space-y-0.5 pl-4">
            {permissionCatalog
              .filter((permission) =>
                snapshot.rules.some((rule) => rule.action === permission.action && rule.subject === permission.subject)
              )
              .map((permission) => (
                <li key={`${permission.action}:${permission.subject}`}>{t(permission.label)}</li>
              ))}
          </ul>
        </>
      ) : (
        <p className="m-0 text-[#8a8a8a]">{t('org.roles.history.absent')}</p>
      )}
    </section>
  )
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}>
      <DialogContent size="xl" className="max-h-[70vh] overflow-y-auto">
        <DialogTitle>{t('org.roles.history.title')}</DialogTitle>
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
          <ol aria-label={t('org.roles.history.title')} className="m-0 list-none space-y-4 p-0">
            {history.map((event) => (
              <li key={event.id} className="space-y-3 border-t border-[#efefef] pt-3">
                <div className="space-y-1">
                  <p className="m-0 text-[14px] font-medium text-[#171717]">{t(actionLabels[event.action])}</p>
                  <time dateTime={event.occurred_at} className="text-[12px] text-[#8a8a8a]">
                    {new Date(event.occurred_at).toLocaleString(i18n.language)}
                  </time>
                  <p className="m-0 text-[12px] break-all text-[#8a8a8a]">
                    {t('org.roles.history.actor', { id: event.actor_member_id })}
                  </p>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  {policy(event.before, 'org.roles.history.before')}
                  {policy(event.after, 'org.roles.history.after')}
                </div>
              </li>
            ))}
          </ol>
        )}
      </DialogContent>
    </Dialog>
  )
}
