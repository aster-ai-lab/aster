import {
  Button,
  Checkbox,
  Combobox,
  ConfirmDialog,
  rowMotionClassName,
  rowMotionStyle,
  TreeSelect,
  type ComboboxOption,
  type TreeSelectOption
} from '@a3s-lab/ui'
import { Ban, Check, History, Pencil, RefreshCw, RotateCcw, UserMinus, UserX } from 'lucide-react'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import {
  beginOptimistic,
  useOptimisticRows,
  type OptimisticChange
} from '@renderer/components/collection/optimisticRows'
import { useCollectionPage } from '@renderer/components/collection/useCollectionPage'
import { useCollectionSelection } from '@renderer/components/collection/useCollectionSelection'
import { ItemActionBar, type ItemActionIcon, type ItemActionMenuEntry } from '@renderer/components/ItemActionBar'
import { useRestingDialog } from '@renderer/components/restingDialog'
import { orgAllows, orgAllowsJoin, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { adminErrorKey, setOrgMemberStatus, type MemberStatus } from './orgAdminApi'
import {
  approveJoinRequest,
  listOrgDepartments,
  listOrgPositions,
  listOrgRoles,
  memberErrorKey,
  rejectJoinRequest,
  removeOrgMember,
  setOrgMemberDepartment,
  setOrgMemberPosition,
  setOrgMemberRole,
  type OrgDepartment,
  type OrgMemberRow,
  type OrgPosition,
  type OrgRole
} from './orgApi'
import { OrgMemberRoleHistory } from './OrgMemberRoleHistory'
import { ghostIconButtonClassName, orgComboboxClassName, rowActionClassName } from './orgUi'

export type DirectoryRow = OrgMemberRow & { applicantId?: string }

const DASH = '—'
const rowId = (row: DirectoryRow) => row.id

function assignableRoleIds(row: DirectoryRow): string[] {
  const ids = row.assignable_role_ids
  return Array.isArray(ids) && ids.every((id) => typeof id === 'string' && id.length > 0) ? ids : []
}
function assignableDepartmentIds(row: DirectoryRow): string[] {
  const ids = row.assignable_department_ids
  return Array.isArray(ids) &&
    ids.every((id) => typeof id === 'string' && id.trim().length > 0 && id === id.trim()) &&
    new Set(ids).size === ids.length
    ? ids
    : []
}

function statusKey(status: string): string | null {
  if (status === 'active') return 'org.status.active'
  if (status === 'invited') return 'org.status.invited'
  if (status === 'disabled') return 'org.status.disabled'
  if (status === 'resigned') return 'org.status.resigned'
  return null
}

function personInitial(name: string): string {
  const text = name.trim()
  return [...text][0] ?? DASH
}

/** 列表和编辑框用同一个称呼。昵称为空时用手机号，再没有才用邮箱。 */
function memberLabel(row: Pick<DirectoryRow, 'name' | 'phone' | 'email'>): string {
  return row.name.trim() || row.phone?.trim() || row.email.trim() || ''
}

function roleLabel(row: DirectoryRow, t: (key: string) => string): string {
  if (row.role_name) return row.role_name
  return t(row.creator ? 'org.enterprise.creator' : row.admin ? 'org.members.admin' : 'org.members.role.member')
}

const NO_POSITION = '__none__'

/** 编辑弹窗里下拉框共用的搜索防抖与匹配。 */
const MEMBER_SELECT_DEBOUNCE_MS = 200

function filterComboboxByLabel(option: ComboboxOption, search: string): boolean {
  const needle = search.trim().toLowerCase()
  return !needle || option.label.toLowerCase().includes(needle)
}

/** 部门下拉是一棵可搜索的树。行上只显示本级名称，搜索仍匹配从根到本级的路径。 */
function departmentTreeOptions(
  rows: OrgDepartment[],
  currentId: string,
  currentName: string,
  allowed: ReadonlySet<string>
): TreeSelectOption[] {
  const byId = new Map(rows.map((row) => [row.id, row]))
  const children = new Map<string, OrgDepartment[]>()
  const roots: OrgDepartment[] = []
  for (const row of rows) {
    if (row.parent_id && row.parent_id !== row.id && byId.has(row.parent_id)) {
      const list = children.get(row.parent_id) ?? []
      list.push(row)
      children.set(row.parent_id, list)
    } else {
      roots.push(row)
    }
  }
  const byName = (left: OrgDepartment, right: OrgDepartment) =>
    left.sort_order - right.sort_order || left.name.localeCompare(right.name, 'zh')
  roots.sort(byName)
  for (const list of children.values()) list.sort(byName)

  const pathLabel = (id: string): string => {
    const names: string[] = []
    const seen = new Set<string>()
    let current = byId.get(id)
    while (current && !seen.has(current.id)) {
      seen.add(current.id)
      names.unshift(current.name)
      current = current.parent_id ? byId.get(current.parent_id) : undefined
    }
    return names.join(' / ')
  }

  const toOption = (row: OrgDepartment, seen: Set<string>): TreeSelectOption => {
    const nextSeen = new Set(seen)
    nextSeen.add(row.id)
    const nested = (children.get(row.id) ?? [])
      .filter((child) => !nextSeen.has(child.id))
      .map((child) => toOption(child, nextSeen))
    return {
      value: row.id,
      label: row.name,
      disabled: !allowed.has(row.id),
      searchText: pathLabel(row.id),
      children: nested.length > 0 ? nested : undefined
    }
  }

  const options = roots.map((row) => toOption(row, new Set()))
  if (currentId && !byId.has(currentId)) {
    options.unshift({
      value: currentId,
      label: currentName || currentId,
      searchText: currentName || currentId,
      disabled: !allowed.has(currentId)
    })
  }
  return options
}

/** 团队页的成员表：工具栏、行内审批 / 编辑 / 移除，以及编辑部门、角色与职位的对话框。 */
export function OrgMemberTable({
  orgId,
  rows,
  departmentNames,
  sourceCount,
  org,
  query,
  listScope = 'members',
  directoryRevision = 0,
  toolbar,
  onRefresh
}: {
  orgId: string
  rows: DirectoryRow[]
  departmentNames: Map<string, string>
  sourceCount: number
  org: MemberOrg
  query: string
  /** Department, position and status selections reset the page without invalidating mutation authority. */
  listScope?: string
  directoryRevision?: number
  toolbar?: ReactNode
  onRefresh: () => Promise<void>
}) {
  const { t } = useTranslation()
  const canApprove = orgAllowsJoin(org, 'approve')
  const canReject = orgAllowsJoin(org, 'reject')
  const canRemove = orgAllows(org, 'delete', 'Member')
  const canAssignRole = orgAllows(org, 'assign', 'Role')
  const canAuditRole = orgAllows(org, 'audit', 'Role')
  const canAssignPosition = orgAllows(org, 'assign', 'Position')
  const canAssignDepartment = orgAllows(org, 'assign', 'Department')
  const canRemoveRow = (row: DirectoryRow) => canRemove && row.can_remove === true && !row.creator
  const canAssignRoleRow = (row: DirectoryRow) =>
    canAssignRole && row.can_assign_role === true && !row.creator && assignableRoleIds(row).length > 0
  const canAssignPositionRow = (row: DirectoryRow) => canAssignPosition && row.can_assign_position === true
  const canAssignDepartmentRow = (row: DirectoryRow) =>
    canAssignDepartment && row.can_assign_department === true && assignableDepartmentIds(row).length > 0
  const canEditRow = (row: DirectoryRow) =>
    canAssignRoleRow(row) || canAssignPositionRow(row) || canAssignDepartmentRow(row)
  const canUpdateStatus = orgAllows(org, 'update', 'Member')
  const canStatusRow = (row: DirectoryRow) =>
    canUpdateStatus && row.can_update_status === true && !row.creator && !row.applicantId
  const authority = JSON.stringify([
    orgId,
    org.creator,
    org.membership,
    org.rules,
    org.dataScope,
    org.canReviewJoin,
    rows.map((row) => [
      row.id,
      row.creator,
      row.admin,
      row.role_code,
      row.department_id,
      row.position_id,
      row.can_remove,
      row.can_update_status,
      row.status,
      row.can_assign_position,
      row.can_assign_department,
      row.assignable_department_ids,
      row.can_assign_role,
      row.assignable_role_ids,
      row.applicantId
    ])
  ])
  const policy = useRef(authority)
  const epoch = useRef(0)
  if (policy.current !== authority) {
    policy.current = authority
    epoch.current += 1
  }
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const generation = epoch.current
  const current = () => mounted.current && epoch.current === generation && policy.current === authority
  const activeRequest = useRef(false)
  const [roles, setRoles] = useState<OrgRole[]>([])
  const [positions, setPositions] = useState<OrgPosition[]>([])
  const [departments, setDepartments] = useState<OrgDepartment[]>([])
  const [pendingId, setPendingId] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const refreshingRef = useRef(false)
  const historyAuthority = JSON.stringify([generation, authority, directoryRevision])
  const [historyTarget, setHistoryTarget] = useState<{ id: string; authority: string } | null>(null)
  const [editTarget, setEditTarget] = useState<DirectoryRow | null>(null)
  const [roleId, setRoleId] = useState('')
  const [positionChoice, setPositionChoice] = useState('')
  const [departmentChoice, setDepartmentChoice] = useState('')
  const [removeTarget, setRemoveTarget] = useState<DirectoryRow | null>(null)
  const [approveTarget, setApproveTarget] = useState<DirectoryRow | null>(null)
  const [statusTarget, setStatusTarget] = useState<{ id: string; next: MemberStatus } | null>(null)
  const [batchConfirm, setBatchConfirm] = useState(false)
  useEffect(() => {
    setEditTarget(null)
    setRemoveTarget(null)
    setApproveTarget(null)
    setStatusTarget(null)
    setBatchConfirm(false)
    setRoleId('')
    setDepartmentChoice('')
    setPositionChoice('')
    setPendingId(null)
    setRefreshing(false)
    refreshingRef.current = false
    activeRequest.current = false
  }, [authority])

  useEffect(() => {
    setHistoryTarget(null)
  }, [historyAuthority])
  const liveHistoryTarget =
    canAuditRole && historyTarget?.authority === historyAuthority
      ? rows.find((row) => row.id === historyTarget.id && !row.applicantId)
      : undefined

  const liveEditTarget = editTarget ? rows.find((row) => row.id === editTarget.id) : undefined
  const liveRemoveTarget = removeTarget ? rows.find((row) => row.id === removeTarget.id) : undefined
  const canEdit = !!liveEditTarget && canEditRow(liveEditTarget)
  const editRole = !!liveEditTarget && canAssignRoleRow(liveEditTarget)
  const editPosition = !!liveEditTarget && canAssignPositionRow(liveEditTarget)
  const editDepartment = !!liveEditTarget && canAssignDepartmentRow(liveEditTarget)

  useEffect(() => {
    let cancelled = false
    setRoles([])
    setPositions([])
    setDepartments([])
    void Promise.all([
      listOrgRoles(orgId).catch(() => [] as OrgRole[]),
      listOrgPositions(orgId).catch(() => [] as OrgPosition[]),
      listOrgDepartments(orgId).catch(() => [] as OrgDepartment[])
    ]).then(([roleRows, positionRows, departmentRows]) => {
      if (cancelled) return
      setRoles(roleRows)
      setPositions(positionRows)
      setDepartments(departmentRows)
    })
    return () => {
      cancelled = true
    }
  }, [orgId, authority, directoryRevision])

  // Authority above reads the owner rows; only the presented rows carry optimistic changes.
  const collection = `org/${orgId}/members`
  const optimistic = useOptimisticRows(collection, rows, rowId)
  const needle = query.trim().toLocaleLowerCase()
  const visible = optimistic.rows.filter((row) => {
    if (!needle) return true
    return [
      row.name,
      row.phone ?? '',
      row.email,
      departmentNames.get(row.department_id) ?? '',
      row.position_name ?? '',
      row.applicantId ? t('org.members.pending') : ''
    ]
      .join(' ')
      .toLocaleLowerCase()
      .includes(needle)
  })
  const pagination = useCollectionPage(visible, `members:${orgId}`, 20, JSON.stringify([listScope, needle]))
  const disableable = pagination.visible.filter((row) => canStatusRow(row) && row.status === 'active')
  const selectable = canUpdateStatus && pagination.visible.some(canStatusRow)
  const selection = useCollectionSelection(disableable, JSON.stringify([orgId, generation, listScope, needle]))
  const selectionOpen = selection.ids.length > 0
  const selectionHold = useRef<readonly string[]>([])
  const selectionPresence = useRestingDialog(selectionOpen)
  if (selectionOpen) selectionHold.current = selection.ids
  const shownSelection = selectionOpen ? selection.ids : selectionHold.current
  const liveStatusTarget = statusTarget ? rows.find((row) => row.id === statusTarget.id) : undefined
  const statusOpen =
    !!statusTarget && !!liveStatusTarget && statusChoices(liveStatusTarget).some((c) => c.next === statusTarget.next)

  function statusChoices(row: DirectoryRow): { next: MemberStatus; key: string; danger: boolean }[] {
    if (!canStatusRow(row)) return []
    if (row.status === 'active')
      return [
        { next: 'disabled', key: 'org.status.disable', danger: false },
        { next: 'resigned', key: 'org.status.resign', danger: true }
      ]
    if (row.status === 'disabled')
      return [
        { next: 'active', key: 'org.status.restore', danger: false },
        { next: 'resigned', key: 'org.status.resign', danger: true }
      ]
    return []
  }

  /** Sends one status command per row; each failure restores only its own row and stays selected. */
  async function disableSelected() {
    if (!current() || activeRequest.current) return false
    const targets = disableable.filter((row) => selection.ids.includes(row.id))
    if (targets.length === 0) return false
    activeRequest.current = true
    setPendingId('batch')
    const handles: ReturnType<typeof beginOptimistic>[] = []
    let failed = 0
    try {
      for (const row of targets) {
        if (!current()) return false
        const handle = beginOptimistic<DirectoryRow>(collection, {
          patch: { [row.id]: (item) => ({ ...item, status: 'disabled' }) }
        })
        handles.push(handle)
        try {
          await setOrgMemberStatus(orgId, row.id, 'disabled')
          handle.confirm()
        } catch {
          handle.rollback()
          failed += 1
        }
      }
      if (!current()) return false
      await onRefresh()
      if (!current()) return false
      if (failed === 0) toast.success(t('org.status.batchDone', { count: targets.length }))
      else toast.error(t('org.status.batchPartial', { ok: targets.length - failed, failed }))
      return true
    } finally {
      for (const handle of handles) handle.release()
      if (current()) {
        activeRequest.current = false
        setPendingId(null)
      }
    }
  }

  /** Runs a write; `change` shows in the table at once and is restored if the owner refuses it. */
  async function run(
    targetId: string,
    action: () => Promise<unknown>,
    successKey: string,
    change?: OptimisticChange<DirectoryRow>,
    errorKey: (err: unknown) => string = memberErrorKey
  ) {
    if (!current() || activeRequest.current) return false
    activeRequest.current = true
    setPendingId(targetId)
    const optimistic = change ? beginOptimistic(collection, change) : null
    try {
      await action()
      optimistic?.confirm()
      if (!current()) return false
      await onRefresh()
      if (!current()) return false
      toast.success(t(successKey))
      return true
    } catch (err) {
      optimistic?.rollback()
      if (current()) {
        const key = errorKey(err)
        toast.error(t(key === 'org.error.generic' ? memberErrorKey(err) : key))
      }
      return false
    } finally {
      optimistic?.release()
      if (current()) {
        activeRequest.current = false
        setPendingId(null)
      }
    }
  }

  async function refreshAll() {
    if (refreshingRef.current || !current()) return
    refreshingRef.current = true
    setRefreshing(true)
    try {
      await onRefresh()
      if (!current()) return
      const [roleRows, positionRows, departmentRows] = await Promise.all([
        listOrgRoles(orgId).catch(() => [] as OrgRole[]),
        listOrgPositions(orgId).catch(() => [] as OrgPosition[]),
        listOrgDepartments(orgId).catch(() => [] as OrgDepartment[])
      ])
      if (!current()) return
      setRoles(roleRows)
      setPositions(positionRows)
      setDepartments(departmentRows)
    } catch (err) {
      if (current()) toast.error(t(memberErrorKey(err)))
    } finally {
      if (current()) {
        refreshingRef.current = false
        setRefreshing(false)
      }
    }
  }

  const rowActions = (row: DirectoryRow): { inline: ItemActionIcon[]; menu: ItemActionMenuEntry[] } | null => {
    const busy = pendingId !== null
    if (row.applicantId) {
      if (!canApprove && !canReject) return null
      const applicantId = row.applicantId
      return {
        inline: canApprove
          ? [
              {
                key: 'approve',
                label: t('org.invite.approve'),
                disabled: busy,
                icon: <Check className="size-3.5 text-current" strokeWidth={1.7} />,
                onClick: () => setApproveTarget(row)
              }
            ]
          : [],
        menu: canReject
          ? [
              {
                type: 'item',
                key: 'reject',
                label: t('org.invite.reject'),
                disabled: busy,
                tone: 'danger',
                onClick: () =>
                  void run(row.id, () => rejectJoinRequest(orgId, applicantId), 'org.invite.rejected', {
                    remove: [row.id]
                  })
              }
            ]
          : []
      }
    }
    const choices = statusChoices(row)
    if (!canEditRow(row) && !canRemoveRow(row) && !canAuditRole && choices.length === 0) return null
    const statusEntry = (choice: (typeof choices)[number]): ItemActionMenuEntry => ({
      type: 'item',
      key: `status:${choice.next}`,
      label: t(choice.key),
      icon:
        choice.next === 'active' ? (
          <RotateCcw className="size-3.5 text-current" strokeWidth={1.7} />
        ) : choice.next === 'resigned' ? (
          <UserX className="size-3.5 text-current" strokeWidth={1.7} />
        ) : (
          <Ban className="size-3.5 text-current" strokeWidth={1.7} />
        ),
      disabled: busy,
      tone: choice.danger ? 'danger' : undefined,
      onClick: () => setStatusTarget({ id: row.id, next: choice.next })
    })
    return {
      inline: [
        ...(canEditRow(row)
          ? [
              {
                key: 'edit',
                label: t('common.edit'),
                disabled: busy,
                icon: <Pencil className="size-3.5 text-current" strokeWidth={1.7} />,
                onClick: () => {
                  setEditTarget(row)
                  setRoleId(roles.find((item) => item.code === row.role_code)?.id ?? '')
                  setPositionChoice(row.position_id ?? '')
                  setDepartmentChoice(row.department_id)
                }
              }
            ]
          : []),
        ...(canAuditRole
          ? [
              {
                key: 'history',
                label: t('org.members.history.title'),
                disabled: busy,
                icon: <History className="size-3.5 text-current" strokeWidth={1.7} />,
                onClick: () => {
                  if (current()) setHistoryTarget({ id: row.id, authority: historyAuthority })
                }
              }
            ]
          : [])
      ],
      menu: [
        ...choices.filter((choice) => !choice.danger).map(statusEntry),
        ...choices.filter((choice) => choice.danger).map(statusEntry),
        ...(canRemoveRow(row)
          ? [
              {
                type: 'item' as const,
                key: 'remove',
                label: t('org.members.remove'),
                icon: <UserMinus className="size-3.5 text-current" strokeWidth={1.7} />,
                disabled: busy,
                tone: 'danger' as const,
                onClick: () => setRemoveTarget(row)
              }
            ]
          : [])
      ]
    }
  }

  return (
    <div data-ui="org.members" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        {toolbar}
        <button
          type="button"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          aria-busy={refreshing || undefined}
          disabled={refreshing || pendingId !== null}
          onClick={() => void refreshAll()}
          className={`${ghostIconButtonClassName} ml-auto`}>
          <RefreshCw
            className={cn('size-4 text-current', refreshing && 'animate-spin')}
            style={refreshing ? { animationDuration: '0.7s' } : undefined}
            strokeWidth={1.7}
            aria-hidden
          />
        </button>
      </div>
      <div className="relative mt-3 flex min-h-0 flex-1 flex-col">
        <div className={cn('min-h-0 flex-1 overflow-auto', selectionPresence.present ? 'pb-20' : 'pb-6')}>
          {visible.length === 0 ? (
            <p className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">
              {sourceCount > 0 ? t('org.members.noMatch') : t('org.members.empty')}
            </p>
          ) : (
            <table
              data-ui="org.members-table"
              aria-label={t('org.nav.members')}
              className="w-full min-w-[960px] table-fixed border-collapse text-left">
              <colgroup>
                {selectable ? <col style={{ width: 40 }} /> : null}
                <col style={{ width: '26%' }} />
                <col style={{ width: '13%' }} />
                <col style={{ width: '10%' }} />
                <col style={{ width: '10%' }} />
                <col style={{ width: '10%' }} />
                <col style={{ width: '12%' }} />
                <col style={{ width: '8%' }} />
                <col style={{ width: '11%' }} />
              </colgroup>
              <thead className="sticky top-0 z-[1] bg-white">
                <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                  {selectable ? (
                    <th scope="col" className="h-9 pl-2.5 font-normal">
                      <Checkbox
                        ref={selection.headerRef}
                        aria-label={t('collection.selectPage')}
                        checked={selection.all}
                        disabled={disableable.length === 0 || pendingId !== null}
                        onCheckedChange={(checked) => selection.togglePage(checked === true)}
                      />
                    </th>
                  ) : null}
                  {['member', 'phone', 'email', 'department', 'position', 'role', 'status', 'joined'].map(
                    (column, index, columns) => (
                      <th
                        key={column}
                        scope="col"
                        className={cn(
                          'h-9 font-normal',
                          index === 0 && 'pl-2.5',
                          index === columns.length - 1 && 'pr-2.5'
                        )}>
                        {t(`org.members.col.${column}`)}
                      </th>
                    )
                  )}
                </tr>
              </thead>
              <tbody>
                {pagination.visible.map((row, index) => {
                  const motion = optimistic.motion(row.id)
                  const status = row.applicantId ? null : statusKey(row.status)
                  const actions = rowActions(row)
                  const actionCount =
                    (actions?.inline.length ?? 0) + (actions?.menu.some((entry) => entry.type === 'item') ? 1 : 0)
                  const displayName = memberLabel(row) || DASH
                  return (
                    <tr
                      key={row.id}
                      data-motion={motion}
                      aria-busy={motion === 'pending' || undefined}
                      style={rowMotionStyle(index, motion)}
                      className={cn(
                        'group hover:bg-[#f7f7f7]',
                        selection.ids.includes(row.id) && 'bg-[#f2f3f5]',
                        rowMotionClassName(motion)
                      )}>
                      {selectable ? (
                        <td className="h-12 pl-2.5">
                          {disableable.some((item) => item.id === row.id) ? (
                            <Checkbox
                              aria-label={t('collection.selectRow', {
                                name: displayName,
                                id: row.phone?.trim() || row.id
                              })}
                              checked={selection.ids.includes(row.id)}
                              disabled={pendingId !== null}
                              onCheckedChange={(checked) => selection.toggle(row.id, checked === true)}
                            />
                          ) : null}
                        </td>
                      ) : null}
                      <td className="relative h-12 overflow-hidden pl-2.5">
                        <span className={cn('flex min-w-0 items-center gap-2.5', actionCount > 2 ? 'pr-20' : 'pr-14')}>
                          <span className="flex size-8 shrink-0 items-center justify-center rounded-[8px] bg-[#f2f3f5] text-[13px] font-medium text-[#3f3f3f]">
                            {personInitial(displayName)}
                          </span>
                          <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[#1a1a1a]">
                            {displayName}
                          </span>
                        </span>
                        {actions ? (
                          <ItemActionBar
                            inline={actions.inline}
                            menu={actions.menu}
                            buttonClassName={rowActionClassName}
                            className="bg-white pr-1 group-hover:bg-[#f7f7f7]"
                          />
                        ) : null}
                      </td>
                      <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#3f3f3f]">
                        {row.phone?.trim() || DASH}
                      </td>
                      <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#3f3f3f]">
                        {row.email.trim() || DASH}
                      </td>
                      <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#3f3f3f]">
                        {departmentNames.get(row.department_id) || DASH}
                      </td>
                      <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#3f3f3f]">
                        {row.position_name?.trim() || DASH}
                      </td>
                      <td className="h-12 overflow-hidden whitespace-nowrap">
                        {row.applicantId ? (
                          <span className="inline-flex h-5 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] text-[#8a8a8a]">
                            {t('org.members.pending')}
                          </span>
                        ) : (
                          <span
                            className={cn(
                              'inline-flex h-5 items-center rounded-[4px] px-1.5 text-[11px]',
                              row.creator || row.admin ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f]'
                            )}>
                            {roleLabel(row, t)}
                          </span>
                        )}
                      </td>
                      <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#8a8a8a]">
                        {row.applicantId ? t('org.members.pending') : status ? t(status) : row.status}
                      </td>
                      <td className="h-12 overflow-hidden pr-2.5 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                        {row.joined_at.slice(0, 10) || DASH}
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          )}
        </div>
        {selectionPresence.present ? (
          <div className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center">
            <section
              aria-label={t('collection.selectedActions')}
              data-ui="org.members.selection"
              data-state={selectionPresence.state}
              className={`a3s-popup-rise flex max-w-full flex-wrap items-center gap-1 rounded-[12px] border border-[#ececec] bg-white py-1.5 pr-1.5 pl-4 shadow-[0_12px_32px_rgba(0,0,0,0.12)] ${
                selectionPresence.state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
              }`}>
              <span role="status" className="mr-3 text-[13px] whitespace-nowrap text-[#171717]">
                {t('collection.selected', { count: shownSelection.length })}
                <span className="ml-2 text-[12px] text-[#3f3f3f]">{t('collection.currentPageOnly')}</span>
              </span>
              <Button size="sm" variant="ghost" disabled={pendingId !== null} onClick={() => setBatchConfirm(true)}>
                <Ban aria-hidden className="size-4" />
                {t('org.status.disable')}
              </Button>
              <Button size="sm" variant="ghost" disabled={pendingId !== null} onClick={selection.clear}>
                {t('collection.clearSelection')}
              </Button>
            </section>
          </div>
        ) : null}
      </div>
      {visible.length > 0 ? (
        <div className="shrink-0 pb-4">
          <CollectionPagination {...pagination} busy={refreshing || pendingId !== null} />
        </div>
      ) : null}

      {liveHistoryTarget ? (
        <OrgMemberRoleHistory
          key={historyAuthority}
          orgId={orgId}
          userId={liveHistoryTarget.id}
          memberName={memberLabel(liveHistoryTarget) || DASH}
          authorityKey={historyAuthority}
          departmentNames={departmentNames}
          onClose={() => setHistoryTarget(null)}
        />
      ) : null}
      <ConfirmDialog
        key={`approve:${generation}`}
        operationKey={`${orgId}:${generation}:${approveTarget?.applicantId}`}
        failureText={t('common.save_failed')}
        open={canApprove && approveTarget !== null}
        onOpenChange={(open) => {
          if (!open) setApproveTarget(null)
        }}
        title={t('org.members.approveConfirm', { name: approveTarget ? memberLabel(approveTarget) : '' })}
        description={t('org.members.approveConfirmBody')}
        confirmText={t('org.invite.approve')}
        cancelText={t('common.cancel')}
        onConfirm={async () => {
          const target = approveTarget
          if (!canApprove || !target?.applicantId) return
          const applicantId = target.applicantId
          void run(
            target.id,
            () => approveJoinRequest(orgId, applicantId),
            'org.invite.approved',
            { patch: { [target.id]: (row) => ({ ...row, applicantId: undefined, status: 'active' }) } },
            adminErrorKey
          )
        }}
      />

      <ConfirmDialog
        key={`status:${generation}`}
        operationKey={`${orgId}:${generation}:${statusTarget?.id}:${statusTarget?.next}`}
        failureText={t('common.save_failed')}
        open={statusOpen}
        onOpenChange={(open) => {
          if (!open) setStatusTarget(null)
        }}
        destructive={statusTarget?.next !== 'active'}
        title={t(`org.status.confirm.${statusTarget?.next ?? 'disabled'}.title`, {
          name: liveStatusTarget ? memberLabel(liveStatusTarget) : ''
        })}
        content={
          <ul className="m-0 grid list-disc gap-1 pl-5 text-[13px] leading-5 text-[#3f3f3f]">
            {(statusTarget?.next === 'active' ? ['access'] : ['access', 'kept', 'after']).map((line) => (
              <li key={line}>
                {t(`org.status.confirm.${statusTarget?.next ?? 'disabled'}.${line}`, { team: org.name })}
              </li>
            ))}
          </ul>
        }
        confirmText={t(
          statusTarget?.next === 'active'
            ? 'org.status.restore'
            : statusTarget?.next === 'resigned'
              ? 'org.status.resign'
              : 'org.status.disable'
        )}
        cancelText={t('common.cancel')}
        onConfirm={async () => {
          const target = liveStatusTarget
          const next = statusTarget?.next
          if (!target || !next || !statusChoices(target).some((choice) => choice.next === next) || !current())
            return false
          void run(
            target.id,
            () => setOrgMemberStatus(orgId, target.id, next),
            `org.status.done.${next}`,
            { patch: { [target.id]: (row) => ({ ...row, status: next }) } },
            adminErrorKey
          )
          return undefined
        }}
      />

      <ConfirmDialog
        key={`batch:${generation}`}
        operationKey={`${orgId}:${generation}:batch:${selection.ids.join(',')}`}
        failureText={t('common.save_failed')}
        open={batchConfirm && selection.ids.length > 0}
        onOpenChange={(open) => {
          if (!open) setBatchConfirm(false)
        }}
        destructive
        title={t('org.status.confirm.batch.title', { count: selection.ids.length })}
        content={
          <ul className="m-0 grid list-disc gap-1 pl-5 text-[13px] leading-5 text-[#3f3f3f]">
            {['access', 'kept', 'after'].map((line) => (
              <li key={line}>{t(`org.status.confirm.disabled.${line}`, { team: org.name })}</li>
            ))}
          </ul>
        }
        confirmText={t('org.status.disable')}
        cancelText={t('common.cancel')}
        onConfirm={async () => {
          void disableSelected()
          return undefined
        }}
      />

      <ConfirmDialog
        key={`remove:${generation}`}
        operationKey={`${orgId}:${generation}:${liveRemoveTarget?.id}`}
        failureText={t('common.delete_failed')}
        open={!!liveRemoveTarget && canRemoveRow(liveRemoveTarget)}
        onOpenChange={(open) => {
          if (!open) setRemoveTarget(null)
        }}
        destructive
        title={t('org.members.remove')}
        description={t('org.members.removeConfirm')}
        confirmText={t('org.members.remove')}
        cancelText={t('common.cancel')}
        onConfirm={async () => {
          const target = liveRemoveTarget
          if (!target || !canRemoveRow(target) || !current()) return false
          void run(target.id, () => removeOrgMember(orgId, target.id), 'org.members.removed', { remove: [target.id] })
          return undefined
        }}
      />

      <ConfirmDialog
        key={`edit:${generation}`}
        operationKey={`${orgId}:${generation}:${liveEditTarget?.id}`}
        failureText={t('common.save_failed')}
        open={canEdit && editTarget !== null}
        onOpenChange={(open) => {
          if (!open) setEditTarget(null)
        }}
        title={t('org.members.editTitle', { name: editTarget ? memberLabel(editTarget) : '' })}
        showCloseButton
        closeLabel={t('common.close')}
        confirmText={t('common.save')}
        cancelText={t('common.cancel')}
        confirmLoading={!!editTarget && pendingId === editTarget.id}
        confirmDisabled={!canEdit || (!!editTarget && ((editRole && !roleId) || (editDepartment && !departmentChoice)))}
        onConfirm={async () => {
          const target = liveEditTarget
          if (!canEdit || !target || !current()) return false
          const originalRoleId = roles.find((item) => item.code === target.role_code)?.id ?? ''
          const roleChanged = editRole && roleId !== originalRoleId
          if (roleChanged && !assignableRoleIds(target).includes(roleId)) return false
          const positionChanged = editPosition && (positionChoice || '') !== (target.position_id ?? '')
          const departmentChanged = editDepartment && !!departmentChoice && departmentChoice !== target.department_id
          if (departmentChanged && !assignableDepartmentIds(target).includes(departmentChoice)) return false
          if (!roleChanged && !positionChanged && !departmentChanged) return
          const successKey = roleChanged
            ? 'org.roles.assigned'
            : positionChanged
              ? 'org.positions.assigned'
              : 'org.members.deptAssigned'
          const role = roleChanged ? roles.find((item) => item.id === roleId) : undefined
          const position = positions.find((item) => item.id === positionChoice)
          const nextPosition = positionChoice || null
          const nextDepartment = departmentChoice
          void run(
            target.id,
            async () => {
              if (departmentChanged) await setOrgMemberDepartment(orgId, target.id, departmentChoice)
              if (!current()) return
              if (roleChanged) await setOrgMemberRole(orgId, target.id, roleId)
              if (!current()) return
              if (positionChanged) await setOrgMemberPosition(orgId, target.id, nextPosition)
            },
            successKey,
            {
              patch: {
                [target.id]: (row) => ({
                  ...row,
                  ...(departmentChanged ? { department_id: nextDepartment } : {}),
                  ...(role ? { role_code: role.code, role_name: role.name } : {}),
                  ...(positionChanged ? { position_id: nextPosition, position_name: position?.name ?? null } : {})
                })
              }
            }
          )
          return undefined
        }}
        contentClassName="sm:max-w-sm"
        content={
          <div className="grid gap-3">
            <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              <span id="org-member-department">{t('org.members.col.department')}</span>
              <TreeSelect
                key={`${editTarget?.id ?? ''}:${departments.map((row) => `${row.id}:${row.parent_id ?? ''}`).join(',')}`}
                aria-labelledby="org-member-department"
                treeData={departmentTreeOptions(
                  departments,
                  editTarget?.department_id ?? '',
                  editTarget ? (departmentNames.get(editTarget.department_id) ?? '') : '',
                  new Set(liveEditTarget ? assignableDepartmentIds(liveEditTarget) : [])
                )}
                value={departmentChoice}
                disabled={!editDepartment}
                defaultExpandAll
                searchable
                searchPlaceholder={t('org.members.deptSearch')}
                emptyText={t('org.members.deptNoMatch')}
                expandLabel={t('common.expand')}
                collapseLabel={t('common.collapse')}
                placeholder={t('org.members.col.department')}
                className={orgComboboxClassName}
                onChange={(value) => setDepartmentChoice(value)}
              />
            </div>
            {editRole && liveEditTarget ? (
              <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
                <span id="org-member-role">{t('org.members.col.role')}</span>
                <Combobox
                  aria-labelledby="org-member-role"
                  options={roles
                    .filter(
                      (item) =>
                        item.code !== 'owner' &&
                        (assignableRoleIds(liveEditTarget).includes(item.id) || item.code === liveEditTarget.role_code)
                    )
                    .map((item) => ({
                      value: item.id,
                      label: item.name,
                      disabled: !assignableRoleIds(liveEditTarget).includes(item.id)
                    }))}
                  value={roleId}
                  searchable
                  searchPlaceholder={t('org.roles.search')}
                  emptyText={t('org.roles.noMatch')}
                  filterOption={filterComboboxByLabel}
                  searchDebounceMs={MEMBER_SELECT_DEBOUNCE_MS}
                  width="100%"
                  placeholder={t('org.members.setRole')}
                  className={orgComboboxClassName}
                  onChange={(value) => setRoleId(typeof value === 'string' ? value : '')}
                />
              </div>
            ) : null}
            <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              <span id="org-member-position">{t('org.members.col.position')}</span>
              <Combobox
                aria-labelledby="org-member-position"
                options={[
                  { value: NO_POSITION, label: t('org.positions.unset') },
                  ...positions.map((item) => ({ value: item.id, label: item.name }))
                ]}
                value={positionChoice || NO_POSITION}
                disabled={!editPosition}
                searchable
                searchPlaceholder={t('org.positions.search')}
                emptyText={t('org.positions.noMatch')}
                filterOption={filterComboboxByLabel}
                searchDebounceMs={MEMBER_SELECT_DEBOUNCE_MS}
                width="100%"
                className={orgComboboxClassName}
                onChange={(value) => setPositionChoice(typeof value === 'string' && value !== NO_POSITION ? value : '')}
              />
            </div>
          </div>
        }
      />
    </div>
  )
}
