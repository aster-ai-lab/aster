import { ConfirmDialog, Dialog, DialogContent, DialogTitle, Input, SearchInput } from '@a3s-lab/ui'
import { Lock, Pencil, Plus, Trash2 } from 'lucide-react'
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { orgAllows, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  createOrgPosition,
  createOrgRole,
  deleteOrgPosition,
  deleteOrgRole,
  listOrgDepartments,
  listOrgPositions,
  listOrgRoles,
  positionErrorKey,
  renameOrgPosition,
  roleErrorKey,
  updateOrgRole,
  type OrgAccessRule,
  type OrgDataScope,
  type OrgPosition,
  type OrgRole
} from './orgApi'
import { OrgDataScopeFields, sameOrgDataScope, validOrgDataScope } from './OrgDataScopeFields'
import { OrgMemberTable, type DirectoryRow } from './OrgMemberTable'
import { GRANTABLE, PERMISSION_GROUPS } from './orgPermissions'
import { OrgRoleHistory } from './OrgRoleHistory'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName, rowActionClassName } from './orgUi'

export const orgSearchClassName =
  'h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]'

const fieldClassName = orgFieldClassName

function grantableOnly(rules: OrgAccessRule[] | undefined): OrgAccessRule[] {
  return GRANTABLE.filter((grant) =>
    (rules ?? []).some(
      (rule) =>
        (rule.action === 'manage' || rule.action === grant.action) &&
        (rule.subject === 'all' || rule.subject === grant.subject)
    )
  ).map(({ action, subject }) => ({ action, subject }))
}

function hasRule(rules: OrgAccessRule[], action: string, subject: string) {
  return rules.some((rule) => rule.action === action && rule.subject === subject)
}

function sameRules(left: OrgAccessRule[], right: OrgAccessRule[]) {
  if (left.length !== right.length) return false
  const key = (rule: OrgAccessRule) => `${rule.action}\0${rule.subject}`
  const a = left.map(key).sort()
  const b = right.map(key).sort()
  return a.every((item, index) => item === b[index])
}

/** 与文档页勾选框同一套：16px、4px 圆角，选中为实心黑底白勾。图标不单独接点击。 */
function PermCheck({ checked, disabled, label }: { checked: boolean; disabled: boolean; label: string }) {
  return (
    <span
      role="checkbox"
      aria-checked={checked}
      aria-disabled={disabled || undefined}
      aria-label={label}
      className={cn(
        'pointer-events-none flex size-4 shrink-0 items-center justify-center rounded-[4px] border',
        disabled && 'opacity-40',
        checked ? 'border-[#171717] bg-[#171717] text-white' : 'border-[#d0d0d0] bg-white text-transparent'
      )}>
      {checked ? (
        <svg viewBox="0 0 16 16" className="size-4 overflow-visible" aria-hidden="true">
          <path
            d="M3.2 8.3 6.3 11.4 12.8 4.7"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      ) : null}
    </span>
  )
}

/** 管理后台目录页：标题在上，左侧树卡片，右侧详情。 */
export function OrgDirectory({
  title,
  description,
  actions,
  treeLabel,
  detailLabel,
  treeHeading,
  tree,
  detail,
  detailClassName
}: {
  title: string
  description?: string
  actions?: ReactNode
  treeLabel: string
  detailLabel: string
  treeHeading?: ReactNode
  tree?: ReactNode
  detail: ReactNode
  detailClassName?: string
}) {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 items-start justify-between gap-4 px-6 pt-4 pb-4">
        <div className="min-w-0">
          <h2 className="m-0 truncate text-[24px] leading-tight font-bold tracking-[-0.01em] text-foreground">
            {title}
          </h2>
          {description ? (
            <p className="m-0 mt-1 max-w-2xl text-[15px] leading-6 text-[#8a8a8a]">{description}</p>
          ) : null}
        </div>
        {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
      </header>
      {tree != null ? (
        <div
          className="grid min-h-0 flex-1 items-stretch gap-2.5 px-6 pb-6"
          style={{ gridTemplateColumns: 'minmax(15rem, 18.5rem) minmax(0, 1fr)' }}>
          <aside
            aria-label={treeLabel}
            className="min-h-56 overflow-auto rounded-[12px] border border-[#ececec] bg-white px-2 pt-2.5 pb-2.5 dark:border-[#2c2c2c] dark:bg-transparent">
            {treeHeading ?? <p className="m-0 mb-1.5 px-1.5 text-[12px] leading-4 text-[#8a8a8a]">{treeLabel}</p>}
            {tree}
          </aside>
          <section
            aria-label={detailLabel}
            className={cn('flex min-h-0 min-w-0 flex-col overflow-hidden', detailClassName)}>
            {detail}
          </section>
        </div>
      ) : (
        <section aria-label={detailLabel} className="flex min-h-0 flex-1 flex-col px-6">
          {detail}
        </section>
      )}
    </div>
  )
}

const treeIconClassName = rowActionClassName

function treeRowClassName(selected: boolean) {
  return cn(
    'group flex h-9 w-full items-center gap-1 rounded-[8px] pr-1 text-left text-[13px] outline-none',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

function RowAction({
  label,
  disabled,
  onClick,
  children
}: {
  label: string
  disabled?: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={rowActionClassName}>
      {children}
    </button>
  )
}

function CatalogStatus({ failed, loading, onRetry }: { failed: boolean; loading: boolean; onRetry: () => void }) {
  const { t } = useTranslation()
  if (failed) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3">
        <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
        <button type="button" onClick={onRetry} className={quietButtonClassName(30)}>
          {t('common.retry')}
        </button>
      </div>
    )
  }
  if (loading) return <p className="m-0 flex-1 py-16 text-center text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
  return null
}

function dialogActions(busy: boolean, canSave: boolean, onCancel: () => void, cancel: string, save: string) {
  return (
    <div className="flex justify-end gap-2">
      <button type="button" disabled={busy} onClick={onCancel} className={quietButtonClassName(36)}>
        {cancel}
      </button>
      <button type="submit" disabled={busy || !canSave} className={filledButtonClassName(36)}>
        {save}
      </button>
    </div>
  )
}

/** 职位页：左侧职位树，右侧担任该职位的成员。 */
export function OrgPositionPanel({
  orgId,
  org,
  members,
  departmentNames,
  directoryRevision,
  directoryFailed,
  policyEpoch,
  onChanged,
  onRefresh
}: {
  orgId: string
  org: MemberOrg
  members: DirectoryRow[]
  departmentNames: Map<string, string>
  directoryRevision: number
  directoryFailed: boolean
  policyEpoch: number
  onChanged: () => void
  onRefresh: () => Promise<void>
}) {
  const { t } = useTranslation()
  const wholeTeam = org.dataScope?.kind === 'all' && org.dataScope.departmentIds.length === 0
  const policyKey = JSON.stringify([orgId, policyEpoch, org.creator, org.membership, org.rules, org.dataScope])
  const policy = useRef({ key: policyKey, generation: 0 })
  if (policy.current.key !== policyKey) {
    policy.current = { key: policyKey, generation: policy.current.generation + 1 }
  }
  const catalogKey = `${orgId}:${policy.current.generation}:${policyKey}`
  const {
    data: catalog,
    error,
    isLoading,
    mutate
  } = useSWR(`studio/org-positions:${catalogKey}`, () => listOrgPositions(orgId))
  const data = error ? undefined : catalog
  const ready = data !== undefined
  const canCreate = ready && wholeTeam && orgAllows(org, 'create', 'Position')
  const canEdit = ready && wholeTeam && orgAllows(org, 'update', 'Position')
  const canDelete = ready && wholeTeam && orgAllows(org, 'delete', 'Position')
  const authorityKey = JSON.stringify([catalogKey, ready, !!error])
  const authority = useRef({ key: authorityKey, generation: 0 })
  if (authority.current.key !== authorityKey) {
    authority.current = { key: authorityKey, generation: authority.current.generation + 1 }
  }
  const generation = authority.current.generation
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const isCurrent = (version: number) => mounted.current && authority.current.generation === version
  const [selectedId, setSelectedId] = useState('all')
  const [memberQuery, setMemberQuery] = useState('')
  const [draft, setDraft] = useState<{ mode: 'create' } | { mode: 'rename'; id: string } | null>(null)
  const [name, setName] = useState('')
  const [nameError, setNameError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<OrgPosition | null>(null)
  useEffect(() => {
    setDraft(null)
    setDeleteTarget(null)
    setName('')
    setNameError(null)
    setBusy(false)
  }, [generation])

  const sorted = useMemo(
    () => [...(data ?? [])].sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name, 'zh')),
    [data]
  )
  useEffect(() => {
    if (selectedId === 'all' || sorted.some((row) => row.id === selectedId)) return
    setSelectedId('all')
  }, [selectedId, sorted])
  const selected = sorted.find((row) => row.id === selectedId) ?? null
  const scoped = selected ? members.filter((row) => row.position_id === selected.id) : members

  const openDraft = (next: { mode: 'create' } | { mode: 'rename'; id: string; name: string }) => {
    if (busy || (next.mode === 'create' ? !canCreate : !canEdit)) return
    setDraft(next.mode === 'create' ? { mode: 'create' } : { mode: 'rename', id: next.id })
    setName(next.mode === 'rename' ? next.name : '')
    setNameError(null)
  }

  async function save() {
    if (busy || !isCurrent(generation) || !draft || (draft.mode === 'create' ? !canCreate : !canEdit)) return
    const trimmed = name.trim()
    if (!trimmed) {
      setNameError(t('org.positions.nameInvalid'))
      return
    }
    setBusy(true)
    setNameError(null)
    try {
      if (draft?.mode === 'rename') await renameOrgPosition(orgId, draft.id, trimmed)
      else await createOrgPosition(orgId, { name: trimmed })
      if (!isCurrent(generation)) return
      await mutate()
      if (!isCurrent(generation)) return
      onChanged()
      toast.success(t(draft?.mode === 'rename' ? 'org.positions.renamed' : 'org.positions.created'))
      setDraft(null)
    } catch (err) {
      if (isCurrent(generation)) setNameError(t(positionErrorKey(err)))
    } finally {
      if (isCurrent(generation)) setBusy(false)
    }
  }

  return (
    <div data-ui="org.positions" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <OrgDirectory
        title={t('org.nav.positions')}
        description={selected?.name ?? t('org.positions.all')}
        treeLabel={t('org.nav.positions')}
        detailLabel={t('org.nav.positions')}
        treeHeading={
          <div className="mb-1.5 flex items-center justify-between gap-1 px-1.5">
            <p className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{t('org.nav.positions')}</p>
            {canCreate ? (
              <button
                type="button"
                aria-label={t('org.positions.add')}
                title={t('org.positions.add')}
                disabled={busy}
                onClick={() => openDraft({ mode: 'create' })}
                className={treeIconClassName}>
                <Plus className="size-3.5 text-current" strokeWidth={1.7} />
              </button>
            ) : null}
          </div>
        }
        tree={
          data ? (
            <div role="tree" aria-label={t('org.nav.positions')}>
              <div
                role="treeitem"
                aria-selected={selectedId === 'all'}
                className={treeRowClassName(selectedId === 'all')}>
                <button
                  type="button"
                  onClick={() => setSelectedId('all')}
                  className="min-w-0 flex-1 truncate bg-transparent px-1.5 text-start">
                  {t('org.positions.all')}
                </button>
              </div>
              {sorted.map((row) => (
                <div
                  key={row.id}
                  role="treeitem"
                  aria-selected={row.id === selectedId}
                  className={treeRowClassName(row.id === selectedId)}
                  style={{ paddingLeft: 16 }}>
                  <button
                    type="button"
                    onClick={() => setSelectedId(row.id)}
                    className="min-w-0 flex-1 truncate bg-transparent text-start">
                    {row.name}
                  </button>
                  {canEdit || canDelete ? (
                    <span className="inline-flex items-center opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                      {canEdit ? (
                        <RowAction
                          label={`${t('org.positions.rename')} ${row.name}`}
                          disabled={busy}
                          onClick={() => openDraft({ mode: 'rename', id: row.id, name: row.name })}>
                          <Pencil className="size-3.5 text-current" strokeWidth={1.7} />
                        </RowAction>
                      ) : null}
                      {canDelete ? (
                        <RowAction
                          label={`${t('org.positions.delete')} ${row.name}`}
                          disabled={busy}
                          onClick={() => setDeleteTarget(row)}>
                          <Trash2 className="size-3.5 text-current" strokeWidth={1.7} />
                        </RowAction>
                      ) : null}
                    </span>
                  ) : null}
                </div>
              ))}
            </div>
          ) : (
            <CatalogStatus
              failed={!!error && !data}
              loading={!!isLoading && !data}
              onRetry={() => void mutate().catch(() => undefined)}
            />
          )
        }
        detail={
          <div className="flex h-full min-h-0 flex-col">
            {directoryFailed ? (
              <CatalogStatus failed loading={false} onRetry={() => void onRefresh().catch(() => undefined)} />
            ) : (
              <OrgMemberTable
                orgId={orgId}
                rows={scoped}
                departmentNames={departmentNames}
                sourceCount={scoped.length}
                org={org}
                query={memberQuery}
                listScope={`position:${selectedId}`}
                directoryRevision={directoryRevision}
                toolbar={
                  <SearchInput
                    value={memberQuery}
                    onChange={(event) => setMemberQuery(event.target.value)}
                    onClear={() => setMemberQuery('')}
                    clearLabel={t('common.clear')}
                    aria-label={t('org.members.search')}
                    placeholder={t('org.members.search')}
                    containerClassName={orgSearchClassName}
                  />
                }
                onRefresh={onRefresh}
              />
            )}
          </div>
        }
      />

      <Dialog
        key={`position-draft:${generation}`}
        open={draft !== null && (draft.mode === 'create' ? canCreate : canEdit)}
        onOpenChange={(open) => {
          if (!open && isCurrent(generation)) setDraft(null)
        }}>
        <DialogContent aria-describedby={undefined} className="sm:max-w-sm">
          <DialogTitle className="text-[15px] leading-5 font-medium">
            {t(draft?.mode === 'rename' ? 'org.positions.renameTitle' : 'org.positions.createTitle')}
          </DialogTitle>
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault()
              void save()
            }}>
            <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-position-name">
              {t('org.positions.name')}
              <Input
                id="org-position-name"
                value={name}
                autoFocus
                disabled={busy}
                maxLength={64}
                placeholder={t('org.positions.namePlaceholder')}
                aria-invalid={nameError ? true : undefined}
                className={fieldClassName}
                onChange={(event) => {
                  setName(event.target.value)
                  setNameError(null)
                }}
              />
            </label>
            {nameError ? (
              <p className="m-0 text-[12px] text-[#b42318]" role="alert">
                {nameError}
              </p>
            ) : null}
            {dialogActions(
              busy,
              !!name.trim() && (draft?.mode === 'create' ? canCreate : canEdit),
              () => setDraft(null),
              t('common.cancel'),
              t('common.save')
            )}
          </form>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        key={`position-delete:${generation}`}
        operationKey={`${orgId}:${generation}:${deleteTarget?.id}`}
        failureText={t('common.delete_failed')}
        open={canDelete && deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open && isCurrent(generation)) setDeleteTarget(null)
        }}
        destructive
        title={t('org.positions.deleteTitle')}
        description={t('org.positions.deleteDesc')}
        confirmText={t('org.positions.delete')}
        cancelText={t('common.cancel')}
        confirmLoading={busy}
        confirmDisabled={busy}
        cancelDisabled={busy}
        onConfirm={async () => {
          const target = deleteTarget
          if (busy || !isCurrent(generation) || !canDelete || !target) return false
          setBusy(true)
          try {
            await deleteOrgPosition(orgId, target.id)
            if (!isCurrent(generation)) return false
            await mutate()
            if (!isCurrent(generation)) return false
            onChanged()
            toast.success(t('org.positions.deleted'))
            setDeleteTarget((present) => (present === target ? null : present))
          } catch (err) {
            if (isCurrent(generation)) toast.error(t(positionErrorKey(err)))
            return false
          } finally {
            if (isCurrent(generation)) {
              setBusy(false)
            }
          }
          return isCurrent(generation)
        }}
      />
    </div>
  )
}

type RoleDraft = { mode: 'create' } | { mode: 'edit'; id: string; expected_revision: string }

/** 角色页：左侧分组树，右侧勾选先留在本地，点保存再写入。超级管理员只读。 */
export function OrgRolePanel({
  orgId,
  org,
  policyEpoch,
  canCreate: createGranted,
  canEdit: editGranted,
  canDelete: deleteGranted,
  onChanged
}: {
  orgId: string
  org: MemberOrg
  policyEpoch: number
  canCreate: boolean
  canEdit: boolean
  canDelete: boolean
  onChanged: () => void
}) {
  const { t } = useTranslation()
  const actorScope = org.dataScope
    ? { kind: org.dataScope.kind as OrgDataScope['kind'], department_ids: org.dataScope.departmentIds }
    : undefined
  const allowedKinds: OrgDataScope['kind'][] = !validOrgDataScope(actorScope)
    ? []
    : actorScope.kind === 'all'
      ? ['self', 'department', 'department_tree', 'custom', 'custom_tree', 'all']
      : actorScope.kind === 'self'
        ? ['self']
        : actorScope.kind === 'department' || actorScope.kind === 'custom'
          ? ['self', 'department', 'custom']
          : ['self', 'department', 'department_tree', 'custom', 'custom_tree']
  const canCreatePolicy = createGranted && allowedKinds.length > 0
  const canEditPolicy = editGranted && allowedKinds.length > 0
  const canDeletePolicy = deleteGranted && allowedKinds.length > 0
  const policyKey = JSON.stringify([
    orgId,
    policyEpoch,
    org.creator,
    org.isOfficial,
    org.membership,
    org.rules,
    org.dataScope,
    canCreatePolicy,
    canEditPolicy,
    canDeletePolicy
  ])
  const policy = useRef({ key: policyKey, generation: 0 })
  if (policy.current.key !== policyKey) {
    policy.current = { key: policyKey, generation: policy.current.generation + 1 }
  }
  const catalogKey = `${orgId}:${policy.current.generation}:${policyKey}`
  const {
    data: catalog,
    error,
    isLoading,
    mutate
  } = useSWR(`studio/org-roles:${catalogKey}`, () => listOrgRoles(orgId))
  const {
    data: departmentCatalog,
    error: departmentsError,
    mutate: mutateDepartments
  } = useSWR(`studio/org-role-departments:${catalogKey}`, () => listOrgDepartments(orgId))
  const data = error ? undefined : catalog
  const departments = departmentsError ? undefined : departmentCatalog
  const ready = data !== undefined && departments !== undefined
  const canCreate = ready && canCreatePolicy
  const canEdit = ready && canEditPolicy
  const canDelete = ready && canDeletePolicy
  const scopeAssignable = (scope: OrgDataScope | undefined): scope is OrgDataScope =>
    validOrgDataScope(scope) &&
    allowedKinds.includes(scope.kind) &&
    scope.department_ids.every((id) => departments?.some((department) => department.id === id))
  const authorityKey = JSON.stringify([
    catalogKey,
    ready,
    !!error,
    !!departmentsError,
    data?.map((role) => [role.id, role.code, role.can_update, role.can_delete])
  ])
  const authority = useRef({ key: authorityKey, generation: 0 })
  if (authority.current.key !== authorityKey) {
    authority.current = { key: authorityKey, generation: authority.current.generation + 1 }
  }
  const generation = authority.current.generation
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const isCurrent = (version: number) => mounted.current && authority.current.generation === version
  const [dataScope, setDataScope] = useState<OrgDataScope | undefined>({ kind: 'self', department_ids: [] })
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<RoleDraft | null>(null)
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [rules, setRules] = useState<OrgAccessRule[]>([])
  const [formError, setFormError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<OrgRole | null>(null)
  const [historyTarget, setHistoryTarget] = useState<OrgRole | null>(null)
  const deletion = useRef({ target: deleteTarget, roles: data })
  deletion.current = { target: deleteTarget, roles: data }
  const [pendingRules, setPendingRules] = useState<{
    id: string
    rules: OrgAccessRule[]
    data_scope: OrgDataScope | undefined
  } | null>(null)
  const [savingPermissions, setSavingPermissions] = useState(false)
  const pendingRef = useRef<{
    id: string
    expected_revision: string
    name: string
    description: string
    rules: OrgAccessRule[]
    data_scope: OrgDataScope | undefined
  } | null>(null)
  useEffect(() => {
    setDraft(null)
    setDeleteTarget(null)
    setHistoryTarget(null)
    pendingRef.current = null
    setPendingRules(null)
    setName('')
    setDescription('')
    setRules([])
    setDataScope({ kind: 'self', department_ids: [] })
    setFormError(null)
    setBusy(false)
    setSavingPermissions(false)
  }, [generation])

  const sorted = useMemo(
    () => [...(data ?? [])].sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name, 'zh')),
    [data]
  )
  useEffect(() => {
    if (selectedId && sorted.some((row) => row.id === selectedId)) return
    setSelectedId(sorted[0]?.id ?? null)
  }, [selectedId, sorted])
  const selected = sorted.find((row) => row.id === selectedId) ?? null
  const builtIn = sorted.filter((row) => row.built_in)
  const custom = sorted.filter((row) => !row.built_in)
  const roleHasRevision = (role: OrgRole | null | undefined): role is OrgRole & { policy_revision: string } =>
    !!role && typeof role.policy_revision === 'string' && /^[a-f0-9]{64}$/.test(role.policy_revision)
  const roleCanEdit = (role: OrgRole | null | undefined): role is OrgRole & { policy_revision: string } =>
    canEdit && roleHasRevision(role) && role.can_update === true
  const roleCanDelete = (role: OrgRole | null | undefined): role is OrgRole & { policy_revision: string } =>
    canDelete && roleHasRevision(role) && role.can_delete === true
  const canAudit = ready && orgAllows(org, 'audit', 'Role')
  useEffect(() => {
    const pending = pendingRef.current
    const pendingRole = data?.find((row) => row.id === pending?.id)
    if (pending && (!pendingRole?.can_update || pendingRole.policy_revision !== pending.expected_revision)) {
      pendingRef.current = null
      setPendingRules(null)
    }
    setDraft((current) =>
      current?.mode === 'edit' &&
      (!data?.find((row) => row.id === current.id)?.can_update ||
        data?.find((row) => row.id === current.id)?.policy_revision !== current.expected_revision)
        ? null
        : current
    )
    const target = deletion.current.target
    setHistoryTarget((current) =>
      current && !data?.some((role) => role.id === current.id && role.policy_revision === current.policy_revision)
        ? null
        : current
    )
    const fresh = data?.find((row) => row.id === target?.id)
    if (target && (!fresh?.can_delete || !roleHasRevision(fresh) || fresh.policy_revision !== target.policy_revision)) {
      setDeleteTarget(null)
      setBusy(false)
    }
  }, [data])

  const openCreate = () => {
    if (busy || savingPermissions || !canCreate) return
    setDraft({ mode: 'create' })
    setName('')
    setDescription('')
    setRules([])
    setDataScope({ kind: 'self', department_ids: [] })
    setFormError(null)
  }

  const openEdit = (role: OrgRole) => {
    if (busy || savingPermissions || !roleCanEdit(role)) return
    setDraft({ mode: 'edit', id: role.id, expected_revision: role.policy_revision })
    setName(role.name)
    setDescription(role.description)
    setRules(grantableOnly(role.rules))
    setDataScope(role.data_scope)
    setFormError(null)
  }

  async function save() {
    if (busy || savingPermissions || !isCurrent(generation) || !draft) return
    const trimmed = name.trim()
    if (!trimmed) {
      setFormError(t('org.roles.nameInvalid'))
      return
    }
    if (
      !scopeAssignable(dataScope) ||
      (draft?.mode === 'edit' ? !roleCanEdit(sorted.find((row) => row.id === draft.id)) : !canCreate)
    )
      return
    setBusy(true)
    setFormError(null)
    const input = { name: trimmed, description: description.trim(), rules, data_scope: dataScope }
    try {
      if (draft?.mode === 'edit')
        await updateOrgRole(orgId, draft.id, { ...input, expected_revision: draft.expected_revision })
      else await createOrgRole(orgId, input)
      if (!isCurrent(generation)) return
      await mutate()
      if (!isCurrent(generation)) return
      onChanged()
      toast.success(t(draft?.mode === 'edit' ? 'org.roles.renamed' : 'org.roles.created'))
      setDraft(null)
    } catch (err) {
      if (isCurrent(generation)) {
        setFormError(t(roleErrorKey(err)))
        if (roleErrorKey(err) === 'org.roles.changed') void mutate().catch(() => undefined)
      }
    } finally {
      if (isCurrent(generation)) setBusy(false)
    }
  }

  const shownRules = (role: OrgRole) => (pendingRules?.id === role.id ? pendingRules.rules : grantableOnly(role.rules))

  const permissionDirty =
    selected != null &&
    pendingRules?.id === selected.id &&
    (!sameRules(pendingRules.rules, grantableOnly(selected.rules)) ||
      !sameOrgDataScope(pendingRules.data_scope, selected.data_scope))

  // 勾选只改本地。保存后刷新当前权限和目录。
  async function savePermissions() {
    const snapshot = pendingRef.current
    if (
      !snapshot ||
      busy ||
      !isCurrent(generation) ||
      !selected ||
      snapshot.id !== selected.id ||
      selected.code === 'owner' ||
      !roleCanEdit(selected) ||
      savingPermissions ||
      !scopeAssignable(snapshot.data_scope) ||
      (sameRules(snapshot.rules, grantableOnly(selected.rules)) &&
        sameOrgDataScope(snapshot.data_scope, selected.data_scope))
    ) {
      return
    }
    setSavingPermissions(true)
    try {
      const saved = await updateOrgRole(orgId, snapshot.id, {
        name: snapshot.name,
        description: snapshot.description,
        rules: snapshot.rules,
        data_scope: snapshot.data_scope,
        expected_revision: snapshot.expected_revision
      })
      if (!isCurrent(generation)) return
      onChanged()
      await mutate(
        (current) =>
          !isCurrent(generation)
            ? current
            : current?.map((role) => {
                if (role.id !== snapshot.id) return role
                const kept = (role.rules ?? []).filter(
                  (rule) => !GRANTABLE.some((item) => item.action === rule.action && item.subject === rule.subject)
                )
                return {
                  ...role,
                  rules: [...kept, ...snapshot.rules],
                  data_scope: snapshot.data_scope,
                  policy_revision: saved.policy_revision
                }
              }),
        { revalidate: false }
      )
      if (!isCurrent(generation)) return
      if (pendingRef.current === snapshot) {
        pendingRef.current = null
        setPendingRules(null)
      }
      toast.success(t('org.roles.assigned'))
    } catch (err) {
      if (isCurrent(generation)) {
        toast.error(t(roleErrorKey(err)))
        if (roleErrorKey(err) === 'org.roles.changed') void mutate().catch(() => undefined)
      }
    } finally {
      if (isCurrent(generation)) setSavingPermissions(false)
    }
  }

  function toggleRule(action: string, subject: string, enabled: boolean) {
    if (
      busy ||
      savingPermissions ||
      !selected ||
      selected.code === 'owner' ||
      !roleCanEdit(selected) ||
      !orgAllows(org, action, subject)
    )
      return
    const base = pendingRef.current?.id === selected.id ? pendingRef.current.rules : grantableOnly(selected.rules)
    const rest = base.filter((item) => !(item.action === action && item.subject === subject))
    const next = enabled ? [...rest, { action, subject }] : rest
    const snapshot = {
      id: selected.id,
      expected_revision:
        pendingRef.current?.id === selected.id ? pendingRef.current.expected_revision : selected.policy_revision,
      name: selected.name,
      description: selected.description,
      rules: next,
      data_scope: pendingRef.current?.id === selected.id ? pendingRef.current.data_scope : selected.data_scope
    }

    pendingRef.current = snapshot
    setPendingRules({ id: snapshot.id, rules: snapshot.rules, data_scope: snapshot.data_scope })
  }

  const shownScope = (role: OrgRole) => (pendingRules?.id === role.id ? pendingRules.data_scope : role.data_scope)
  const changeScope = (data_scope: OrgDataScope) => {
    if (
      busy ||
      !selected ||
      selected.code === 'owner' ||
      !roleCanEdit(selected) ||
      savingPermissions ||
      !allowedKinds.includes(data_scope.kind)
    )
      return
    const snapshot = {
      id: selected.id,
      expected_revision:
        pendingRef.current?.id === selected.id ? pendingRef.current.expected_revision : selected.policy_revision,
      name: selected.name,
      description: selected.description,
      rules: shownRules(selected),
      data_scope
    }
    pendingRef.current = snapshot
    setPendingRules({ id: snapshot.id, rules: snapshot.rules, data_scope })
  }

  const roleRow = (row: OrgRole) => {
    const isSelected = row.id === selectedId
    return (
      <div
        key={row.id}
        role="treeitem"
        aria-selected={isSelected}
        className={treeRowClassName(isSelected)}
        style={{ paddingLeft: 8 }}>
        {row.built_in ? <Lock className="ml-1 size-3.5 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden /> : null}
        <button
          type="button"
          onClick={() => setSelectedId(row.id)}
          className="min-w-0 flex-1 truncate bg-transparent px-1.5 text-start">
          {row.name}
        </button>
        {!row.built_in && (roleCanEdit(row) || roleCanDelete(row)) ? (
          <span className="inline-flex items-center opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
            {roleCanEdit(row) ? (
              <RowAction
                disabled={busy || savingPermissions}
                label={`${t('common.edit')} ${row.name}`}
                onClick={() => openEdit(row)}>
                <Pencil className="size-3.5 text-current" strokeWidth={1.7} />
              </RowAction>
            ) : null}
            {roleCanDelete(row) ? (
              <RowAction
                label={
                  row.member_count > 0
                    ? `${t('org.roles.hasMembers')} ${row.name}`
                    : `${t('org.roles.delete')} ${row.name}`
                }
                disabled={row.member_count > 0 || busy || savingPermissions}
                onClick={() => setDeleteTarget(row)}>
                <Trash2 className="size-3.5 text-current" strokeWidth={1.7} />
              </RowAction>
            ) : null}
          </span>
        ) : null}
      </div>
    )
  }

  return (
    <div data-ui="org.roles" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <OrgDirectory
        title={t('org.nav.roles')}
        description={
          selected?.code === 'owner'
            ? t('org.roles.builtInLocked')
            : selected?.description.trim() ||
              (selected ? (selected.built_in ? t('org.perm.baseline') : t('org.roles.custom')) : undefined)
        }
        treeLabel={t('org.nav.roles')}
        detailLabel={t('org.nav.roles')}
        treeHeading={
          <div className="mb-1.5 flex items-center justify-between gap-1 px-1.5">
            <p className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{t('org.nav.roles')}</p>
            {canCreate ? (
              <button
                type="button"
                aria-label={t('org.roles.create')}
                disabled={busy || savingPermissions}
                title={t('org.roles.create')}
                onClick={openCreate}
                className={treeIconClassName}>
                <Plus className="size-3.5 text-current" strokeWidth={1.7} />
              </button>
            ) : null}
          </div>
        }
        tree={
          data ? (
            <div role="tree" aria-label={t('org.nav.roles')}>
              {builtIn.length > 0 ? (
                <>
                  <p className="m-0 mt-1 mb-1 px-1.5 text-[12px] leading-4 text-[#8a8a8a]">{t('org.roles.builtIn')}</p>
                  {builtIn.map(roleRow)}
                </>
              ) : null}
              {custom.length > 0 ? (
                <>
                  <p className="m-0 mt-2 mb-1 px-1.5 text-[12px] leading-4 text-[#8a8a8a]">{t('org.roles.custom')}</p>
                  {custom.map(roleRow)}
                </>
              ) : null}
            </div>
          ) : (
            <CatalogStatus
              failed={!!error && !data}
              loading={!!isLoading && !data}
              onRetry={() => void mutate().catch(() => undefined)}
            />
          )
        }
        detail={
          selected ? (
            <div className="grid gap-3" data-ui="org.permissions">
              <CatalogStatus
                failed={!!departmentsError}
                loading={false}
                onRetry={() => void mutateDepartments().catch(() => undefined)}
              />
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="m-0 truncate text-[15px] leading-5 font-medium text-[#171717]">{selected.name}</p>
                  <p className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                    {selected.description.trim() ||
                      (selected.code === 'owner' ? t('org.roles.builtInLocked') : t('org.perm.baseline'))}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-3">
                  <span className="text-[12px] leading-5 text-[#8a8a8a] tabular-nums">
                    {t('org.members.count', { n: selected.member_count })}
                  </span>
                  {canAudit && roleHasRevision(selected) ? (
                    <button type="button" className={quietButtonClassName()} onClick={() => setHistoryTarget(selected)}>
                      {t('org.roles.history.title')}
                    </button>
                  ) : null}
                  {roleCanEdit(selected) && selected.code !== 'owner' ? (
                    <button
                      type="button"
                      className={filledButtonClassName(30)}
                      disabled={!permissionDirty || !scopeAssignable(shownScope(selected)) || busy || savingPermissions}
                      onClick={() => void savePermissions()}>
                      {t('common.save')}
                    </button>
                  ) : null}
                </div>
              </div>
              <OrgDataScopeFields
                scope={shownScope(selected)}
                departments={departments}
                allowedKinds={allowedKinds}
                disabled={!roleCanEdit(selected) || selected.code === 'owner' || busy || savingPermissions}
                onChange={changeScope}
              />
              <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.perm.inviteScopeHint')}</p>
              <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.perm.settingsScopeHint')}</p>
              <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.perm.positionScopeHint')}</p>
              {PERMISSION_GROUPS.filter((group) => !group.officialOnly || org.isOfficial).map((group) => {
                const granted = shownRules(selected)
                const on = group.rules.filter((rule) => hasRule(granted, rule.action, rule.subject)).length
                return (
                  <div key={group.label} className="border-t border-[#efefef] pt-3">
                    <div className="mb-1 flex items-center gap-2 px-1.5">
                      <p className="m-0 text-[13px] font-medium text-[#171717]">{t(group.label)}</p>
                      <span className="ml-auto text-[12px] text-[#8a8a8a] tabular-nums">
                        {on}/{group.rules.length}
                      </span>
                    </div>
                    <div className="grid grid-cols-[repeat(auto-fill,minmax(11.5rem,1fr))] gap-x-3">
                      {group.rules.map((rule) => {
                        const checked = hasRule(granted, rule.action, rule.subject)
                        const locked =
                          busy ||
                          savingPermissions ||
                          !roleCanEdit(selected) ||
                          selected.code === 'owner' ||
                          !orgAllows(org, rule.action, rule.subject)
                        return (
                          <button
                            key={`${rule.action}:${rule.subject}`}
                            type="button"
                            disabled={locked}
                            aria-label={t(rule.label)}
                            onClick={() => toggleRule(rule.action, rule.subject, !checked)}
                            className={cn(
                              'flex h-8 w-full items-center gap-2 rounded-[8px] border-0 bg-transparent px-1.5 text-start text-[13px] text-[#3f3f3f]',
                              locked ? 'cursor-default' : 'cursor-pointer hover:bg-[#f7f7f7]'
                            )}>
                            <PermCheck checked={checked} disabled={locked} label={t(rule.label)} />
                            {t(rule.label)}
                          </button>
                        )
                      })}
                    </div>
                  </div>
                )
              })}
            </div>
          ) : data ? (
            <p className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">{t('org.roles.empty')}</p>
          ) : null
        }
        detailClassName={
          selected ? 'overflow-auto rounded-[12px] border border-[#ececec] bg-white px-4 pt-3.5 pb-4' : undefined
        }
      />

      <Dialog
        key={`role-draft:${generation}`}
        open={
          draft !== null &&
          (draft.mode === 'create' ? canCreate : roleCanEdit(sorted.find((role) => role.id === draft.id)))
        }
        onOpenChange={(open) => {
          if (!open && isCurrent(generation)) setDraft(null)
        }}>
        <DialogContent aria-describedby={undefined} className="sm:max-w-md">
          <DialogTitle className="text-[15px] leading-5 font-medium">
            {t(draft?.mode === 'edit' ? 'org.roles.editTitle' : 'org.roles.createTitle')}
          </DialogTitle>
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault()
              void save()
            }}>
            <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-role-name">
              {t('org.roles.name')}
              <Input
                id="org-role-name"
                value={name}
                autoFocus
                disabled={busy}
                maxLength={32}
                placeholder={t('org.roles.namePlaceholder')}
                aria-invalid={formError ? true : undefined}
                className={fieldClassName}
                onChange={(event) => {
                  setName(event.target.value)
                  setFormError(null)
                }}
              />
            </label>
            <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-role-description">
              {t('org.roles.description')}
              <Input
                id="org-role-description"
                value={description}
                disabled={busy}
                maxLength={200}
                placeholder={t('org.roles.descriptionPlaceholder')}
                className={fieldClassName}
                onChange={(event) => setDescription(event.target.value)}
              />
            </label>
            <OrgDataScopeFields
              scope={dataScope}
              departments={departments}
              allowedKinds={allowedKinds}
              disabled={busy}
              onChange={setDataScope}
            />
            {formError ? (
              <p className="m-0 text-[12px] text-[#b42318]" role="alert">
                {formError}
              </p>
            ) : null}
            {dialogActions(
              busy,
              !!name.trim() && scopeAssignable(dataScope),
              () => setDraft(null),
              t('common.cancel'),
              t('common.save')
            )}
          </form>
        </DialogContent>
      </Dialog>

      {canAudit &&
      historyTarget &&
      roleHasRevision(historyTarget) &&
      data?.some((role) => role.id === historyTarget.id && role.policy_revision === historyTarget.policy_revision) ? (
        <OrgRoleHistory
          key={`${generation}:${historyTarget.id}:${historyTarget.policy_revision}`}
          orgId={orgId}
          roleId={historyTarget.id}
          authorityKey={`${authorityKey}:${historyTarget.policy_revision}`}
          departments={departments ?? []}
          permissionCatalog={GRANTABLE}
          onClose={() => {
            if (isCurrent(generation)) setHistoryTarget(null)
          }}
        />
      ) : null}
      <ConfirmDialog
        key={`role-delete:${generation}:${deleteTarget?.id}:${deleteTarget?.policy_revision}`}
        operationKey={`${orgId}:${generation}:${deleteTarget?.id}:${deleteTarget?.policy_revision}`}
        failureText={t('common.delete_failed')}
        open={
          deleteTarget !== null &&
          roleCanDelete(sorted.find((role) => role.id === deleteTarget.id)) &&
          sorted.find((role) => role.id === deleteTarget.id)?.policy_revision === deleteTarget.policy_revision
        }
        onOpenChange={(open) => {
          if (!open && isCurrent(generation) && deletion.current.target === deleteTarget) setDeleteTarget(null)
        }}
        destructive
        title={t('org.roles.deleteTitle')}
        description={t('org.roles.deleteDesc')}
        confirmText={t('org.roles.delete')}
        cancelText={t('common.cancel')}
        confirmLoading={busy}
        confirmDisabled={busy || savingPermissions}
        cancelDisabled={busy}
        onConfirm={async () => {
          const target = deleteTarget
          if (busy || savingPermissions || !isCurrent(generation) || !roleCanDelete(target)) return false
          const current = () => {
            const fresh = deletion.current.roles?.find((role) => role.id === target.id)
            return (
              isCurrent(generation) &&
              deletion.current.target === target &&
              roleCanDelete(fresh) &&
              fresh.policy_revision === target.policy_revision
            )
          }
          if (!current()) return false
          setBusy(true)
          try {
            await deleteOrgRole(orgId, target.id, target.policy_revision)
            if (!current()) return false
            onChanged()
            toast.success(t('org.roles.deleted'))
            await mutate()
          } catch (err) {
            if (current()) {
              const key = roleErrorKey(err)
              toast.error(t(key === 'org.roles.changed' ? 'org.roles.deleteChanged' : key))
              onChanged()
              void mutate().catch(() => undefined)
            }
          } finally {
            if (isCurrent(generation) && deletion.current.target === target) {
              setDeleteTarget(null)
              setBusy(false)
            }
          }
          return false
        }}
      />
    </div>
  )
}
