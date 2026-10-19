import { Combobox, Dialog, DialogContent, DialogTitle, Input, TreeSelect, type TreeSelectOption } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { orgAllows, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'

import {
  createOrgAccount,
  listOrgDepartments,
  listOrgPositions,
  listOrgRoles,
  memberErrorKey,
  type OrgDepartment
} from './orgApi'
import { filledButtonClassName, orgComboboxClassName, orgFieldClassName, quietButtonClassName } from './orgUi'

const NO_POSITION = ''

function departmentTree(rows: OrgDepartment[]): TreeSelectOption[] {
  const byParent = new Map<string, OrgDepartment[]>()
  for (const row of rows) {
    const key = row.parent_id ?? ''
    const list = byParent.get(key) ?? []
    list.push(row)
    byParent.set(key, list)
  }
  const walk = (parent: string): TreeSelectOption[] =>
    (byParent.get(parent) ?? [])
      .slice()
      .sort((left, right) => left.sort_order - right.sort_order || left.name.localeCompare(right.name))
      .map((row) => {
        const children = walk(row.id)
        return children.length > 0 ? { value: row.id, label: row.name, children } : { value: row.id, label: row.name }
      })
  const roots = walk('')
  return roots.length > 0 ? roots : rows.map((row) => ({ value: row.id, label: row.name }))
}

/** 管理员按手机号建账号。显示名由会员自己持有，这里不填。 */
export function OrgCreateAccount({
  orgId,
  org,
  onCreated
}: {
  orgId: string
  org: MemberOrg
  onCreated: () => Promise<void>
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [phone, setPhone] = useState('')
  const [password, setPassword] = useState('')
  const [departmentId, setDepartmentId] = useState('')
  const [roleId, setRoleId] = useState('')
  const [positionId, setPositionId] = useState(NO_POSITION)
  const [departments, setDepartments] = useState<OrgDepartment[]>([])
  const [roles, setRoles] = useState<Array<{ id: string; code: string; name: string }>>([])
  const [positions, setPositions] = useState<Array<{ id: string; name: string }>>([])
  const [busy, setBusy] = useState(false)
  const canAssignRole = orgAllows(org, 'assign', 'Role')
  const canAssignPosition = orgAllows(org, 'assign', 'Position')

  useEffect(() => {
    if (!open) return
    let cancelled = false
    void Promise.all([listOrgDepartments(orgId), listOrgRoles(orgId), listOrgPositions(orgId)])
      .then(([departmentRows, roleRows, positionRows]) => {
        if (cancelled) return
        setDepartments(departmentRows)
        setRoles(roleRows.filter((row) => row.code !== 'owner'))
        setPositions(positionRows)
        setDepartmentId(
          (current) => current || departmentRows.find((row) => row.parent_id == null)?.id || departmentRows[0]?.id || ''
        )
        setRoleId((current) => current || roleRows.find((row) => row.code === 'member')?.id || '')
      })
      .catch((err: unknown) => {
        if (!cancelled) toast.error(t(memberErrorKey(err)))
      })
    return () => {
      cancelled = true
    }
  }, [open, orgId, t])

  if (!canAssignRole) return null

  async function submit() {
    const trimmed = phone.trim()
    if (!trimmed || !departmentId || !roleId || busy) return
    setBusy(true)
    try {
      await createOrgAccount(orgId, {
        phone: trimmed,
        department_id: departmentId,
        role_id: roleId,
        ...(password.trim() ? { password: password.trim() } : {}),
        ...(canAssignPosition && positionId ? { position_id: positionId } : {})
      })
      toast.success(t('org.members.created'))
      setOpen(false)
      setPhone('')
      setPassword('')
      setPositionId(NO_POSITION)
      await onCreated()
    } catch (err) {
      toast.error(t(memberErrorKey(err)))
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <button
        type="button"
        data-ui="org.create-account"
        onClick={() => setOpen(true)}
        className={quietButtonClassName(30)}>
        {t('org.members.create')}
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent aria-describedby={undefined} className="sm:max-w-[440px]">
          <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.members.createTitle')}</DialogTitle>
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault()
              void submit()
            }}>
            <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              {t('org.members.col.phone')}
              <Input
                value={phone}
                autoComplete="off"
                className={orgFieldClassName}
                onChange={(event) => setPhone(event.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              {t('org.members.createPassword')}
              <Input
                type="password"
                value={password}
                autoComplete="new-password"
                className={orgFieldClassName}
                onChange={(event) => setPassword(event.target.value)}
              />
            </label>
            <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              <span id="org-create-department">{t('org.members.col.department')}</span>
              <TreeSelect
                aria-labelledby="org-create-department"
                treeData={departmentTree(departments)}
                value={departmentId}
                defaultExpandAll
                searchable
                width="100%"
                className={orgComboboxClassName}
                onChange={(value) => setDepartmentId(value)}
              />
            </div>
            <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              <span id="org-create-role">{t('org.members.col.role')}</span>
              <Combobox
                aria-labelledby="org-create-role"
                options={roles.map((row) => ({ value: row.id, label: row.name }))}
                value={roleId}
                searchable={roles.length > 8}
                width="100%"
                className={orgComboboxClassName}
                onChange={(value) => setRoleId(typeof value === 'string' ? value : '')}
              />
            </div>
            {canAssignPosition ? (
              <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
                <span id="org-create-position">{t('org.members.col.position')}</span>
                <Combobox
                  aria-labelledby="org-create-position"
                  options={[
                    { value: NO_POSITION, label: t('org.positions.unset') },
                    ...positions.map((row) => ({ value: row.id, label: row.name }))
                  ]}
                  value={positionId}
                  searchable={positions.length > 8}
                  width="100%"
                  className={orgComboboxClassName}
                  onChange={(value) => setPositionId(typeof value === 'string' ? value : NO_POSITION)}
                />
              </div>
            ) : null}
            <div className="flex justify-end gap-2">
              <button type="button" className={quietButtonClassName(30)} onClick={() => setOpen(false)}>
                {t('common.cancel')}
              </button>
              <button
                type="submit"
                disabled={busy || !phone.trim() || !departmentId || !roleId}
                className={filledButtonClassName(30)}>
                {t('org.members.createSubmit')}
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}
