import {
  ConfirmDialog,
  Dialog,
  DialogContent,
  DialogTitle,
  EmptyState,
  Input,
  SearchInput,
  SearchSelect,
  SegmentedControl
} from '@a3s-lab/ui'
import {
  Activity,
  Blocks,
  Briefcase,
  Building2,
  ChevronRight,
  CirclePlay,
  Clock3,
  BookOpen,
  CalendarDays,
  CreditCard,
  FolderGit2,
  KeyRound,
  LayoutDashboard,
  LogIn,
  Pencil,
  Plus,
  ScrollText,
  Settings,
  Shield,
  Trash2,
  Users
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { textLogo } from '@renderer/components/account/entityImage'
import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { PluginPage } from '@renderer/components/plugins/PluginPage'
import {
  canViewOrgPositions,
  canViewOrgRoles,
  isSessionDenied,
  listMemberOrgs,
  orgAllows,
  orgAllowsJoin,
  orgAllowsInvite,
  logoutMemberAccount,
  PERSONAL_CENTER,
  readCenterId,
  returnToLogin,
  subscribeCenter,
  type MemberOrg
} from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import type { PluginRenderProps } from '@renderer/types/workbenchPlugin'
import { cn } from '@renderer/utils/style'
import { notifyWorkInboxChanged } from '@renderer/workInbox/client'

import { listMemberInvites, usedSeats, fetchOrgSubscription, type SeatSummary } from './orgAdminApi'
import {
  createOrgDepartment,
  deleteOrgDepartment,
  departmentErrorKey,
  listJoinRequests,
  listOrgDepartments,
  listOrgMembers,
  renameOrgDepartment,
  type OrgDepartment,
  type PendingApplicant
} from './orgApi'
import { OrgAuditPanel } from './OrgAuditPanel'
import { OrgCapabilitiesPanel } from './OrgCapabilitiesPanel'
import { OrgDirectory, OrgPositionPanel, OrgRolePanel, orgSearchClassName } from './OrgCatalog'
import { OrgInviteTools } from './OrgInviteDialog'
import { OrgInvitesTable } from './OrgInvitesTable'
import { OrgMemberTable, type DirectoryRow } from './OrgMemberTable'
import { OrgPlanPanel } from './OrgPlanPanel'
import { OrgRepositoriesPanel } from './OrgRepositoriesPanel'
import { OrgRunsPanel } from './OrgRunsPanel'
import { OrgSettingsPanel } from './OrgSettingsPanel'
import { OrgTokensPanel } from './OrgTokensPanel'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName, rowActionClassName } from './orgUi'
import { OrgUsagePanel } from './OrgUsagePanel'
import { OrgWorkbench } from './OrgWorkbench'

type DeptDraft = { mode: 'create'; parentId: string } | { mode: 'rename'; id: string; name: string }
type MemberSegment = 'members' | 'pending' | 'invites'
type StatusFilter = 'active' | 'disabled' | 'resigned' | 'all'
const STATUS_FILTERS: StatusFilter[] = ['active', 'disabled', 'resigned', 'all']
type OrgSection =
  | 'workbench'
  | 'knowledge'
  | 'schedule'
  | 'runs'
  | 'members'
  | 'positions'
  | 'roles'
  | 'repositories'
  | 'tokens'
  | 'usage'
  | 'plan'
  | 'capabilities'
  | 'audit'
  | 'settings'

const SECTIONS: { id: OrgSection; icon: typeof Users }[] = [
  { id: 'workbench', icon: LayoutDashboard },
  { id: 'knowledge', icon: BookOpen },
  { id: 'schedule', icon: CalendarDays },
  { id: 'runs', icon: CirclePlay },
  { id: 'members', icon: Users },
  { id: 'positions', icon: Briefcase },
  { id: 'roles', icon: Shield },
  { id: 'repositories', icon: FolderGit2 },
  { id: 'tokens', icon: KeyRound },
  { id: 'usage', icon: Activity },
  { id: 'plan', icon: CreditCard },
  { id: 'capabilities', icon: Blocks },
  { id: 'audit', icon: ScrollText },
  { id: 'settings', icon: Settings }
]

const NAV_GROUPS: { id: 'work' | 'agents' | 'organization' | 'dev' | 'admin'; ids: OrgSection[] }[] = [
  { id: 'work', ids: ['workbench', 'knowledge', 'schedule'] },
  { id: 'agents', ids: ['runs'] },
  { id: 'organization', ids: ['members', 'positions', 'roles'] },
  { id: 'dev', ids: ['repositories', 'tokens'] },
  { id: 'admin', ids: ['usage', 'plan', 'capabilities', 'audit', 'settings'] }
]

/** 新上线的菜单项。点开过一次后不再显示「新」。 */
const NEW_SECTIONS: readonly OrgSection[] = ['repositories', 'tokens', 'plan', 'audit', 'runs', 'capabilities']
const SECTION_KEY = 'org.section.v1'
const SEEN_KEY = 'org.nav.seen.v1'
const STATUS_FILTER_KEY = 'org.members.status.v1'

function isOrgSection(value: unknown): value is OrgSection {
  return typeof value === 'string' && SECTIONS.some((item) => item.id === value)
}

function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value)
  } catch {
    // Private browsing can reject storage. The current choice still stays on screen.
  }
}

function readSection(orgId: string | null): OrgSection | null {
  const raw = orgId ? readStored(`${SECTION_KEY}:${orgId}`) : null
  return isOrgSection(raw) ? raw : null
}

function readSeen(): Set<OrgSection> {
  try {
    const parsed = JSON.parse(readStored(SEEN_KEY) ?? '[]') as unknown
    return new Set(Array.isArray(parsed) ? parsed.filter(isOrgSection) : [])
  } catch {
    return new Set()
  }
}

function readStatusFilter(orgId: string | null): StatusFilter {
  const raw = orgId ? readStored(`${STATUS_FILTER_KEY}:${orgId}`) : null
  return (STATUS_FILTERS as (string | null)[]).includes(raw) ? (raw as StatusFilter) : 'active'
}

function navClass(selected: boolean) {
  return cn(
    'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
    'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

function rootDepartmentId(rows: OrgDepartment[]): string | null {
  return rows.find((row) => row.parent_id === null)?.id ?? rows[0]?.id ?? null
}

function subtreeIds(rows: OrgDepartment[], selectedId: string): Set<string> {
  const children = new Map<string, string[]>()
  for (const row of rows) {
    if (!row.parent_id) continue
    const list = children.get(row.parent_id) ?? []
    list.push(row.id)
    children.set(row.parent_id, list)
  }
  const ids = new Set<string>()
  const walk = (id: string) => {
    if (ids.has(id)) return
    ids.add(id)
    for (const child of children.get(id) ?? []) walk(child)
  }
  walk(selectedId)
  return ids
}

/** 团队侧栏承载工作台、知识、日程和组织管理。邮件在任务窗口里。用量只给创建人。 */
export default function OrgPage({ initialSection }: Partial<PluginRenderProps> & { initialSection?: OrgSection } = {}) {
  const { t } = useTranslation()
  const [centerId, setCenterId] = useState(readCenterId)
  useEffect(() => subscribeCenter(() => setCenterId(readCenterId())), [])
  const {
    data: orgs,
    error: orgsError,
    isLoading: orgsLoading,
    mutate: mutateOrgs
  } = useSWR('studio/member-orgs', () => listMemberOrgs())
  useEffect(() => {
    void mutateOrgs()
  }, [centerId, mutateOrgs])
  const profileName = useMemberProfile()?.displayName ?? ''

  const orgId = centerId === PERSONAL_CENTER ? null : centerId
  const currentOrg: MemberOrg | undefined = orgsError ? undefined : orgs?.find((org) => org.id === orgId)
  const canReview = orgAllowsJoin(currentOrg, 'approve') || orgAllowsJoin(currentOrg, 'reject')
  const canInvite = orgAllowsInvite(currentOrg)
  const canViewPositions = canViewOrgPositions(currentOrg)
  const canViewRoles = canViewOrgRoles(currentOrg)
  const canViewUsage = !!currentOrg?.creator
  const canViewAudit = orgAllows(currentOrg, 'read', 'Audit')
  const wholeTeam = !!currentOrg?.creator || currentOrg?.dataScope?.kind === 'all'
  const canViewRuns = wholeTeam && orgAllows(currentOrg, 'read', 'Run')
  const canStopRuns = wholeTeam && orgAllows(currentOrg, 'stop', 'Run')
  const canManageCapabilities = orgAllows(currentOrg, 'manage', 'Capability')
  const sections = SECTIONS.filter(
    ({ id }) =>
      (id !== 'positions' || canViewPositions) &&
      (id !== 'roles' || canViewRoles) &&
      (id !== 'usage' || canViewUsage) &&
      (id !== 'audit' || canViewAudit) &&
      (id !== 'runs' || canViewRuns) &&
      (id !== 'capabilities' || canManageCapabilities)
  )

  const directoryPolicy = JSON.stringify([
    orgId,
    currentOrg?.creator,
    currentOrg?.membership,
    currentOrg?.rules,
    currentOrg?.dataScope,
    canReview,
    currentOrg?.canLeaveOrg
  ])
  const directoryPolicyRef = useRef(directoryPolicy)
  const directoryEpoch = useRef(0)
  if (directoryPolicyRef.current !== directoryPolicy) {
    directoryPolicyRef.current = directoryPolicy
    directoryEpoch.current += 1
  }
  const policyEpoch = directoryEpoch.current
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const directoryKey =
    orgId && currentOrg && currentOrg.membership !== 'pending'
      ? (`studio/org-directory:${orgId}:${directoryEpoch.current}:${directoryPolicy}` as const)
      : null
  const {
    data: catalog,
    error,
    isLoading,
    mutate
  } = useSWR(
    directoryKey,
    () =>
      Promise.all([
        listOrgMembers(orgId!),
        listOrgDepartments(orgId!),
        canReview ? listJoinRequests(orgId!) : Promise.resolve([] as PendingApplicant[])
      ]),
    { dedupingInterval: 0 }
  )

  const data = error ? undefined : catalog
  const ready = data !== undefined
  const departmentAuthorityKey = JSON.stringify([directoryKey, ready, !!error])
  const departmentAuthority = useRef({ key: departmentAuthorityKey, generation: 0 })
  if (departmentAuthority.current.key !== departmentAuthorityKey) {
    departmentAuthority.current = {
      key: departmentAuthorityKey,
      generation: departmentAuthority.current.generation + 1
    }
  }
  const departmentGeneration = departmentAuthority.current.generation
  const isCurrent = (version: number) => mounted.current && departmentAuthority.current.generation === version
  const [rows, departments, applicants] = data ?? [undefined, undefined, undefined]
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [collapsedIds, setCollapsedIds] = useState<ReadonlySet<string>>(() => new Set())
  const [segment, setSegment] = useState<MemberSegment>('members')
  const [statusFilter, setStatusFilterState] = useState<StatusFilter>(() => readStatusFilter(orgId))
  const [memberQuery, setMemberQuery] = useState('')
  const [section, setSectionState] = useState<OrgSection>(() => initialSection ?? readSection(orgId) ?? 'workbench')
  const [seen, setSeen] = useState(readSeen)
  const sectionOrg = useRef(orgId)
  useEffect(() => {
    if (sectionOrg.current === orgId || !orgId) return
    sectionOrg.current = orgId
    setSectionState((current) => readSection(orgId) ?? initialSection ?? current)
    setStatusFilterState(readStatusFilter(orgId))
    setSegment('members')
  }, [orgId, initialSection])
  const setSection = (next: OrgSection) => {
    setSectionState(next)
    if (orgId) writeStored(`${SECTION_KEY}:${orgId}`, next)
    if (NEW_SECTIONS.includes(next) && !seen.has(next)) {
      const updated = new Set(seen).add(next)
      setSeen(updated)
      writeStored(SEEN_KEY, JSON.stringify([...updated]))
    }
  }
  const setStatusFilter = (next: StatusFilter) => {
    setStatusFilterState(next)
    if (orgId) writeStored(`${STATUS_FILTER_KEY}:${orgId}`, next)
  }
  const [directoryRevision, setDirectoryRevision] = useState(0)
  useEffect(() => {
    if (
      (section === 'positions' && !canViewPositions) ||
      (section === 'roles' && !canViewRoles) ||
      (section === 'usage' && !canViewUsage) ||
      (section === 'audit' && !canViewAudit) ||
      (section === 'runs' && !canViewRuns) ||
      (section === 'capabilities' && !canManageCapabilities)
    ) {
      setSectionState('members')
    }
  }, [canManageCapabilities, canViewAudit, canViewPositions, canViewRoles, canViewRuns, canViewUsage, section])
  useEffect(() => {
    if (!directoryKey || section !== 'members') return
    const refresh = () => {
      if (document.visibilityState === 'hidden') return
      void mutate()
    }
    document.addEventListener('visibilitychange', refresh)
    window.addEventListener('focus', refresh)
    return () => {
      document.removeEventListener('visibilitychange', refresh)
      window.removeEventListener('focus', refresh)
    }
  }, [directoryKey, mutate, section])
  useEffect(() => {
    if (!directoryKey || !profileName) return
    void mutate()
  }, [directoryKey, mutate, profileName])
  const effectiveSegment: MemberSegment =
    (segment === 'pending' && !canReview) || (segment === 'invites' && !canInvite) ? 'members' : segment
  const invitesKey =
    directoryKey && canInvite ? (`studio/org-invites:${orgId}:${directoryEpoch.current}` as const) : null
  const { data: invites, mutate: mutateInvites } = useSWR(invitesKey, () => listMemberInvites(orgId!), {
    dedupingInterval: 0
  })
  const subscriptionKey = directoryKey ? (`studio/org-subscription:${orgId}` as const) : null
  const { data: subscription } = useSWR(subscriptionKey, () => fetchOrgSubscription(orgId!))
  const seats: SeatSummary | null =
    typeof subscription?.seats === 'number' && rows
      ? { used: usedSeats(rows), total: subscription.seats, planId: subscription.plan_id }
      : null
  const [draft, setDraft] = useState<DeptDraft | null>(null)
  const [deptName, setDeptName] = useState('')
  const [deptError, setDeptError] = useState<string | null>(null)
  const [deptBusy, setDeptBusy] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<OrgDepartment | null>(null)
  useEffect(() => {
    setDraft(null)
    setDeleteTarget(null)
    setDeptName('')
    setDeptError(null)
    setDeptBusy(false)
  }, [departmentGeneration])

  // Keep the tree selection on a department that still exists; fall back to the root.
  useEffect(() => {
    if (!departments) return
    if (selectedId && departments.some((row) => row.id === selectedId)) return
    setSelectedId(rootDepartmentId(departments))
  }, [departments, selectedId])

  const sorted = useMemo(() => {
    const list = [...(departments ?? [])]
    list.sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name, 'zh'))
    return list
  }, [departments])
  const treeRoots = useMemo(() => {
    const visibleIds = new Set(sorted.map((row) => row.id))
    return sorted.filter((row) => !row.parent_id || !visibleIds.has(row.parent_id))
  }, [sorted])
  const childrenByParent = useMemo(() => {
    const map = new Map<string, OrgDepartment[]>()
    for (const row of sorted) {
      if (!row.parent_id) continue
      const list = map.get(row.parent_id) ?? []
      list.push(row)
      map.set(row.parent_id, list)
    }
    return map
  }, [sorted])
  const expandedIds = useMemo(() => {
    const expanded = new Set<string>()
    for (const row of sorted) {
      if (!collapsedIds.has(row.id)) expanded.add(row.id)
    }
    return expanded
  }, [collapsedIds, sorted])

  const toggleNode = (id: string) => {
    setCollapsedIds((previous) => {
      const next = new Set(previous)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const departmentIds = useMemo(
    () => (selectedId && departments ? subtreeIds(departments, selectedId) : null),
    [departments, selectedId]
  )
  const names = useMemo(() => {
    const map = new Map<string, string>()
    for (const row of departments ?? []) map.set(row.id, row.name)
    return map
  }, [departments])

  const scopedRows = useMemo(
    () => (rows ?? []).filter((row) => !departmentIds || departmentIds.has(row.department_id)),
    [rows, departmentIds]
  )
  const scopedCount = scopedRows.filter((row) => statusFilter === 'all' || row.status === statusFilter).length
  const pendingInvites = (invites ?? []).filter((row) => row.status === 'pending').length
  const pendingRows: DirectoryRow[] = useMemo(
    () =>
      (applicants ?? []).map((applicant) => ({
        id: `pending:${applicant.member_id}`,
        name: applicant.name,
        email: '',
        phone: null,
        status: 'pending',
        admin: false,
        creator: false,
        department_id: '',
        joined_at: '',
        position_id: null,
        position_name: null,
        role_code: '',
        role_name: '',
        role_built_in: false,
        applicantId: applicant.member_id
      })),
    [applicants]
  )
  const visible: DirectoryRow[] = useMemo(() => {
    if (effectiveSegment === 'pending') return pendingRows
    return scopedRows.filter((row) => statusFilter === 'all' || row.status === statusFilter)
  }, [scopedRows, pendingRows, effectiveSegment, statusFilter])

  const selected = departments?.find((row) => row.id === selectedId) ?? null

  const reload = async () => {
    await Promise.all([mutate(), mutateOrgs(), invitesKey ? mutateInvites() : undefined])
    notifyWorkInboxChanged()
  }
  const reloadInvites = async () => {
    await mutateInvites()
  }

  async function saveDepartment() {
    if (
      deptBusy ||
      !ready ||
      !isCurrent(departmentGeneration) ||
      !orgId ||
      !draft ||
      !orgAllows(currentOrg, draft.mode === 'create' ? 'create' : 'update', 'Department')
    )
      return
    const name = deptName.trim()
    if (!name) {
      setDeptError(t('org.members.deptNameInvalid'))
      return
    }
    setDeptBusy(true)
    setDeptError(null)
    try {
      if (draft.mode === 'create') await createOrgDepartment(orgId, { parent_id: draft.parentId, name })
      else await renameOrgDepartment(orgId, draft.id, name)
      if (!isCurrent(departmentGeneration)) return
      await mutate()
      if (!isCurrent(departmentGeneration)) return
      toast.success(t(draft.mode === 'create' ? 'org.members.deptCreated' : 'org.members.deptRenamed'))
      setDraft(null)
    } catch (err) {
      if (isCurrent(departmentGeneration)) setDeptError(t(departmentErrorKey(err)))
    } finally {
      if (isCurrent(departmentGeneration)) setDeptBusy(false)
    }
  }

  async function removeDepartment() {
    const target = deleteTarget
    if (
      deptBusy ||
      !ready ||
      !isCurrent(departmentGeneration) ||
      !orgId ||
      !target ||
      !orgAllows(currentOrg, 'delete', 'Department')
    )
      return false
    setDeptBusy(true)
    try {
      await deleteOrgDepartment(orgId, target.id)
      if (!isCurrent(departmentGeneration)) return false
      await mutate()
      if (!isCurrent(departmentGeneration)) return false
      toast.success(t('org.members.deptDeleted'))
    } catch (err) {
      if (isCurrent(departmentGeneration)) toast.error(t(departmentErrorKey(err)))
    } finally {
      if (isCurrent(departmentGeneration)) {
        setDeleteTarget(null)
        setDeptBusy(false)
      }
    }
    return isCurrent(departmentGeneration)
  }

  const openDraft = (next: DeptDraft) => {
    if (deptBusy || !ready || !orgAllows(currentOrg, next.mode === 'create' ? 'create' : 'update', 'Department')) return
    setDraft(next)
    setDeptName(next.mode === 'rename' ? next.name : '')
    setDeptError(null)
  }

  const bumpDirectory = () => {
    setDirectoryRevision((value) => value + 1)
    void reload()
  }

  // 未选团队、申请中或登录失效：不进入管理页。
  const loadingOrgs = !!orgId && !orgs && (orgsLoading || !orgsError)
  const denied = !!orgsError && isSessionDenied(orgsError)
  const directoryFailure = error ? (
    <div className="flex flex-1 flex-col items-center justify-center gap-3">
      <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
      <button type="button" onClick={() => void mutate().catch(() => undefined)} className={quietButtonClassName(30)}>
        {t('common.retry')}
      </button>
    </div>
  ) : null

  let body: ReactNode
  if (!orgId || !currentOrg || currentOrg.membership === 'pending') {
    body = (
      <div className="flex h-full min-h-0 w-full flex-col bg-white">
        {loadingOrgs ? null : denied ? (
          <EmptyState
            icon={LogIn}
            title={t('sidebar.center.session_expired')}
            actionLabel={t('sidebar.center.sign_in_again')}
            onAction={() => void logoutMemberAccount().finally(returnToLogin)}
          />
        ) : orgId && orgsError ? (
          <EmptyState
            icon={Building2}
            title={t('org.error.generic')}
            actionLabel={t('common.retry')}
            onAction={() => void mutateOrgs()}
          />
        ) : currentOrg?.membership === 'pending' ? (
          <EmptyState
            icon={Clock3}
            title={t('org.empty.pendingTitle')}
            description={t('org.empty.pending', { name: currentOrg.name })}
          />
        ) : (
          <EmptyState icon={Building2} title={t('org.empty.noOrgTitle')} description={t('org.empty.noOrg')} />
        )}
      </div>
    )
  } else {
    const teamLogo = currentOrg.logo || textLogo(currentOrg.name)
    const segmentOptions = [
      { value: 'members' as const, label: `${t('org.members.filter.members')} ${scopedCount}` },
      ...(canReview
        ? [{ value: 'pending' as const, label: `${t('org.members.filter.pending')} ${pendingRows.length}` }]
        : []),
      ...(canInvite
        ? [{ value: 'invites' as const, label: `${t('org.members.filter.invites')} ${pendingInvites}` }]
        : [])
    ]
    const memberSegments =
      segmentOptions.length > 1 ? (
        <SegmentedControl
          aria-label={t('org.members.segments')}
          size="sm"
          className="h-[30px]"
          value={effectiveSegment}
          options={segmentOptions}
          onValueChange={(next) => setSegment(next)}
        />
      ) : null

    body = (
      <div className="flex h-full min-h-0 w-full bg-white">
        <div className="flex min-h-0 min-w-0 flex-1">
          <aside data-ui="org.sidebar" className="flex w-[15.25rem] shrink-0 flex-col border-r border-[#efefef]">
            <header data-ui="org.team" className="mx-3 flex items-center gap-2.5 px-2.5 pt-4 pb-3">
              {teamLogo ? (
                <img src={teamLogo} alt="" className="size-8 shrink-0 rounded-[8px] object-cover" />
              ) : (
                <span className="flex size-8 shrink-0 items-center justify-center rounded-[8px] bg-[#f2f3f5] text-[#3f3f3f]">
                  <Building2 className="size-4 text-current" strokeWidth={1.7} aria-hidden />
                </span>
              )}
              <div className="min-w-0">
                <h1
                  id="org-sidebar-title"
                  className="m-0 truncate text-[14px] leading-5 font-medium tracking-[-0.01em] text-foreground">
                  {currentOrg.name}
                </h1>
                <p className="m-0 truncate text-[12px] leading-4 text-[#8a8a8a]">
                  {t('org.members.count', { n: currentOrg.memberCount ?? rows?.length ?? 0 })}
                </p>
              </div>
            </header>
            <nav
              aria-labelledby="org-sidebar-title"
              className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-3 pb-4">
              {NAV_GROUPS.map((group) => {
                const items = group.ids.flatMap((id) => {
                  const entry = sections.find((item) => item.id === id)
                  return entry ? [entry] : []
                })
                if (items.length === 0) return null
                const labelId = `org-nav-${group.id}`
                return (
                  <div key={group.id} role="group" aria-labelledby={labelId} className="grid content-start gap-0.5">
                    <p id={labelId} className="m-0 px-2.5 pb-1 text-[12px] leading-4 text-[#8a8a8a]">
                      {t(`org.nav.group.${group.id}`)}
                    </p>
                    {items.map(({ id, icon: Icon }) => (
                      <button
                        key={id}
                        type="button"
                        data-ui={`org.nav.${id}`}
                        aria-label={t(`org.nav.${id}`)}
                        aria-current={section === id ? 'page' : undefined}
                        className={navClass(section === id)}
                        onClick={() => setSection(id)}>
                        <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} />
                        <span className="min-w-0 flex-1 truncate">{t(`org.nav.${id}`)}</span>
                        {NEW_SECTIONS.includes(id) && !seen.has(id) ? (
                          <span className="rounded-[4px] bg-[#f2f3f5] px-1 text-[11px] leading-4 text-[#3f3f3f]">
                            {t('org.nav.new')}
                          </span>
                        ) : null}
                      </button>
                    ))}
                  </div>
                )
              })}
            </nav>
          </aside>

          <div key={orgId} className="flex min-h-0 min-w-0 flex-1 flex-col">
            {section === 'workbench' ? <OrgWorkbench orgId={orgId} org={currentOrg} /> : null}
            {section === 'knowledge' || section === 'schedule' ? (
              <PluginPage page={`studio.${section}/team-content`} />
            ) : null}
            {section === 'members' ? (
              <OrgDirectory
                title={t('org.nav.members')}
                description={selected?.name ?? (rows ? t('org.members.count', { n: visible.length }) : undefined)}
                actions={
                  !error && canInvite && orgId ? (
                    <OrgInviteTools
                      orgId={orgId}
                      org={currentOrg}
                      seats={seats}
                      onOpenPlan={() => setSection('plan')}
                      onMembershipChanged={reload}
                      onInvited={reloadInvites}
                    />
                  ) : undefined
                }
                treeLabel={t('org.members.tree')}
                detailLabel={t('org.nav.members')}
                tree={
                  departments && departments.length > 0 ? (
                    <div role="tree" aria-label={t('org.members.tree')} data-ui="org.tree">
                      <DeptTreeLevel
                        nodes={treeRoots}
                        depth={0}
                        childrenByParent={childrenByParent}
                        expandedIds={expandedIds}
                        selectedId={selectedId}
                        org={currentOrg}
                        busy={deptBusy}
                        onSelect={setSelectedId}
                        onToggle={toggleNode}
                        openDraft={openDraft}
                        onDelete={(node) => {
                          if (!deptBusy) setDeleteTarget(node)
                        }}
                      />
                    </div>
                  ) : undefined
                }
                detail={
                  error ? (
                    directoryFailure
                  ) : isLoading && !data ? (
                    <div className="relative min-h-40 flex-1">
                      <PageEntry id="org.directory" ready={false} />
                    </div>
                  ) : effectiveSegment === 'invites' ? (
                    <OrgInvitesTable
                      orgId={orgId}
                      org={currentOrg}
                      invites={invites ?? []}
                      segments={memberSegments}
                      onRefresh={reloadInvites}
                    />
                  ) : (
                    <OrgMemberTable
                      orgId={orgId}
                      rows={visible}
                      departmentNames={names}
                      sourceCount={(rows ?? []).length}
                      org={currentOrg}
                      query={memberQuery}
                      listScope={JSON.stringify([selectedId, effectiveSegment, statusFilter])}
                      directoryRevision={directoryRevision}
                      onRefresh={reload}
                      toolbar={
                        <>
                          <SearchInput
                            value={memberQuery}
                            onChange={(event) => setMemberQuery(event.target.value)}
                            onClear={() => setMemberQuery('')}
                            clearLabel={t('common.clear')}
                            aria-label={t('org.members.search')}
                            placeholder={t('org.members.search')}
                            containerClassName={orgSearchClassName}
                          />
                          {memberSegments}
                          {effectiveSegment === 'members' ? (
                            <SearchSelect
                              size="sm"
                              searchable={false}
                              allowClear={false}
                              aria-label={t('org.members.statusFilter')}
                              value={statusFilter}
                              options={STATUS_FILTERS.map((value) => ({
                                value,
                                label: t(`org.members.statusFilter.${value}`)
                              }))}
                              onChange={(next) =>
                                setStatusFilter(
                                  (STATUS_FILTERS as string[]).includes(next) ? (next as StatusFilter) : 'active'
                                )
                              }
                              className="w-[104px]"
                            />
                          ) : null}
                        </>
                      }
                    />
                  )
                }
              />
            ) : null}
            {section === 'positions' && canViewPositions ? (
              <OrgPositionPanel
                orgId={orgId}
                org={currentOrg}
                members={rows ?? []}
                departmentNames={names}
                directoryRevision={directoryRevision}
                directoryFailed={!!error}
                policyEpoch={policyEpoch}
                onChanged={bumpDirectory}
                onRefresh={reload}
              />
            ) : null}
            {section === 'roles' && canViewRoles ? (
              <OrgRolePanel
                orgId={orgId}
                org={currentOrg}
                policyEpoch={policyEpoch}
                canCreate={orgAllows(currentOrg, 'create', 'Role')}
                canEdit={orgAllows(currentOrg, 'update', 'Role')}
                canDelete={orgAllows(currentOrg, 'delete', 'Role')}
                onChanged={bumpDirectory}
              />
            ) : null}
            {section === 'repositories' ? (
              <OrgRepositoriesPanel orgId={orgId} members={rows ?? []} onOpenTokens={() => setSection('tokens')} />
            ) : null}
            {section === 'tokens' ? <OrgTokensPanel /> : null}
            {section === 'plan' ? <OrgPlanPanel orgId={orgId} org={currentOrg} members={rows ?? []} /> : null}
            {section === 'audit' && canViewAudit ? (
              <OrgAuditPanel key={orgId} orgId={orgId} members={rows ?? []} />
            ) : null}
            {section === 'capabilities' && canManageCapabilities ? (
              <OrgCapabilitiesPanel key={orgId} orgId={orgId} />
            ) : null}
            {section === 'runs' && canViewRuns ? (
              <OrgRunsPanel key={orgId} orgId={orgId} canStop={canStopRuns} />
            ) : null}
            {section === 'usage' && canViewUsage ? (
              error ? (
                directoryFailure
              ) : data ? (
                <OrgUsagePanel departments={departments ?? []} members={rows ?? []} />
              ) : (
                <div className="relative min-h-40 flex-1">
                  <PageEntry id="org.usage" ready={false} />
                </div>
              )
            ) : null}
            {section === 'settings' ? (
              <OrgSettingsPanel
                org={currentOrg}
                members={rows ?? []}
                onChanged={async () => {
                  await mutateOrgs()
                }}
              />
            ) : null}
          </div>
        </div>

        <Dialog
          key={`department-draft:${departmentGeneration}`}
          open={
            ready &&
            draft !== null &&
            orgAllows(currentOrg, draft.mode === 'create' ? 'create' : 'update', 'Department')
          }
          onOpenChange={(open) => {
            if (!open && isCurrent(departmentGeneration)) setDraft(null)
          }}>
          <DialogContent aria-describedby={undefined} className="sm:max-w-sm">
            <DialogTitle className="text-[15px] leading-5 font-medium">
              {t(draft?.mode === 'rename' ? 'org.members.deptRenameTitle' : 'org.members.deptCreateTitle')}
            </DialogTitle>
            <form
              className="grid gap-3"
              onSubmit={(event) => {
                event.preventDefault()
                void saveDepartment()
              }}>
              <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-dept-name">
                {t('org.members.deptName')}
                <Input
                  id="org-dept-name"
                  value={deptName}
                  autoFocus
                  disabled={deptBusy}
                  maxLength={64}
                  placeholder={t('org.members.deptNamePlaceholder')}
                  aria-invalid={deptError ? true : undefined}
                  className={orgFieldClassName}
                  onChange={(event) => {
                    setDeptName(event.target.value)
                    setDeptError(null)
                  }}
                />
              </label>
              {deptError ? (
                <p className="m-0 text-[12px] text-[#b42318]" role="alert">
                  {deptError}
                </p>
              ) : null}
              <div className="flex justify-end gap-2">
                <button
                  type="button"
                  disabled={deptBusy}
                  onClick={() => setDraft(null)}
                  className={quietButtonClassName(36)}>
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  disabled={
                    deptBusy ||
                    !ready ||
                    !deptName.trim() ||
                    !draft ||
                    !orgAllows(currentOrg, draft.mode === 'create' ? 'create' : 'update', 'Department')
                  }
                  className={filledButtonClassName(36)}>
                  {t('common.save')}
                </button>
              </div>
            </form>
          </DialogContent>
        </Dialog>

        <ConfirmDialog
          key={`department-delete:${departmentGeneration}`}
          operationKey={`${orgId}:${departmentGeneration}:${deleteTarget?.id}`}
          failureText={t('common.delete_failed')}
          open={ready && deleteTarget !== null && orgAllows(currentOrg, 'delete', 'Department')}
          onOpenChange={(open) => {
            if (!open && isCurrent(departmentGeneration)) setDeleteTarget(null)
          }}
          destructive
          title={t('org.members.deptDeleteTitle')}
          description={t('org.members.deptDeleteDesc')}
          confirmText={t('org.members.deptDelete')}
          cancelText={t('common.cancel')}
          confirmLoading={deptBusy}
          confirmDisabled={deptBusy}
          cancelDisabled={deptBusy}
          onConfirm={removeDepartment}
        />
      </div>
    )
  }

  return (
    <div data-ui="org.page" className="relative flex h-full min-h-0 min-w-0 flex-1 bg-white">
      {body}
      <PageEntry id="org" ready={!loadingOrgs} />
    </div>
  )
}

function DeptTreeLevel({
  nodes,
  depth,
  childrenByParent,
  expandedIds,
  selectedId,
  org,
  busy,
  onSelect,
  onToggle,
  openDraft,
  onDelete
}: {
  nodes: OrgDepartment[]
  depth: number
  childrenByParent: Map<string, OrgDepartment[]>
  expandedIds: ReadonlySet<string>
  selectedId: string | null
  org: MemberOrg
  busy: boolean
  onSelect: (id: string) => void
  onToggle: (id: string) => void
  openDraft: (draft: DeptDraft) => void
  onDelete: (node: OrgDepartment) => void
}) {
  const { t } = useTranslation()
  const canCreate = orgAllows(org, 'create', 'Department')
  const canEdit = orgAllows(org, 'update', 'Department')
  const canDelete = orgAllows(org, 'delete', 'Department')
  return (
    <>
      {nodes.map((node) => {
        const children = childrenByParent.get(node.id) ?? []
        const isExpanded = expandedIds.has(node.id)
        const isSelected = node.id === selectedId
        return (
          <div
            key={node.id}
            role="treeitem"
            aria-selected={isSelected}
            aria-expanded={children.length > 0 ? isExpanded : undefined}>
            <div
              className={cn(
                'group relative flex h-9 items-center rounded-[8px] pr-1 text-[13px] outline-none',
                isSelected
                  ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                  : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
              )}
              style={{ paddingLeft: 8 + depth * 16 }}>
              <button
                type="button"
                onClick={() => onSelect(node.id)}
                className="flex min-w-0 flex-1 items-center gap-2.5 bg-transparent text-start outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                {children.length > 0 ? (
                  <ChevronRight
                    className={cn('size-3.5 shrink-0 text-current transition-transform', isExpanded && 'rotate-90')}
                    strokeWidth={1.7}
                    aria-hidden
                    onClick={(event) => {
                      event.stopPropagation()
                      onToggle(node.id)
                    }}
                  />
                ) : (
                  <span className="size-3.5 shrink-0" aria-hidden />
                )}
                <span className="min-w-0 flex-1 truncate">{node.name}</span>
              </button>
              {canCreate || canEdit || canDelete ? (
                <div
                  className={cn(
                    'flex shrink-0 items-center gap-0.5',
                    // The selected row keeps its actions visible. A background press
                    // selects the row and does not hover it.
                    isSelected ? 'visible' : 'invisible group-focus-within:visible group-hover:visible'
                  )}>
                  {canCreate ? (
                    <button
                      type="button"
                      aria-label={t('org.members.deptAdd')}
                      disabled={busy}
                      title={t('org.members.deptAdd')}
                      onClick={(event) => {
                        event.stopPropagation()
                        openDraft({ mode: 'create', parentId: node.id })
                      }}
                      className={rowActionClassName}>
                      <Plus className="size-3.5 text-current" strokeWidth={1.7} />
                    </button>
                  ) : null}
                  {canEdit ? (
                    <button
                      type="button"
                      aria-label={`${t('org.members.deptRename')} ${node.name}`}
                      disabled={busy}
                      title={t('org.members.deptRename')}
                      onClick={(event) => {
                        event.stopPropagation()
                        openDraft({ mode: 'rename', id: node.id, name: node.name })
                      }}
                      className={rowActionClassName}>
                      <Pencil className="size-3.5 text-current" strokeWidth={1.7} />
                    </button>
                  ) : null}
                  {canDelete && node.parent_id !== null ? (
                    <button
                      type="button"
                      aria-label={`${t('org.members.deptDelete')} ${node.name}`}
                      disabled={busy}
                      title={t('org.members.deptDelete')}
                      onClick={(event) => {
                        event.stopPropagation()
                        onDelete(node)
                      }}
                      className={rowActionClassName}>
                      <Trash2 className="size-3.5 text-current" strokeWidth={1.7} />
                    </button>
                  ) : null}
                </div>
              ) : null}
            </div>
            {children.length > 0 && isExpanded ? (
              <div role="group">
                <DeptTreeLevel
                  nodes={children}
                  depth={depth + 1}
                  childrenByParent={childrenByParent}
                  expandedIds={expandedIds}
                  selectedId={selectedId}
                  org={org}
                  busy={busy}
                  onSelect={onSelect}
                  onToggle={onToggle}
                  openDraft={openDraft}
                  onDelete={onDelete}
                />
              </div>
            ) : null}
          </div>
        )
      })}
    </>
  )
}
