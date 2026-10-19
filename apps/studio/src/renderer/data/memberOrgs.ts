import { readStudioRuntime } from '@shared/studioRuntime'

/** 未选团队时停留在个人中心。与用户中心同一个键，两边源不同，互不覆盖。 */
export const PERSONAL_CENTER = 'personal'

const STORAGE_KEY = 'a3s.org.center'
const CENTER_BY_MEMBER = 'a3s.org.center.by-member'
const CENTER_EVENT = 'a3s-org-center'
const CENTER_COOKIE = 'a3s.org.center'
const LEGACY_STORAGE_KEY = 'kense.org.center'
const LEGACY_CENTER_BY_MEMBER = 'kense.org.center.by-member'
const LEGACY_CENTER_COOKIE = 'kense.org.center'
const MEMBER_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

/** Member whose team the shared slot currently belongs to. Empty after logout. */
let centerMemberId = ''

/** Exact account binding maintained by login/logout; never infer it from a team name. */
export function readCenterMemberId(): string {
  return centerMemberId
}

export type MemberDepartment = {
  id: string
  parent_id: string | null
  name: string
  sort_order: number
}

export type MemberOrg = {
  id: string
  name: string
  logo: string
  membership?: 'admin' | 'member' | 'pending'
  isOfficial?: boolean
  creator?: boolean
  industry?: string
  scale?: string
  memberCount?: number
  canReadWork?: boolean
  canCreateWork?: boolean
  canReviewJoin?: boolean
  canInviteMembers?: boolean
  canUpdateOrg?: boolean
  canTransferOrg?: boolean
  canDeleteOrg?: boolean
  canLeaveOrg?: boolean
  canReadSchedule?: boolean
  canCreateSchedule?: boolean
  canUpdateSchedule?: boolean
  canDeleteSchedule?: boolean
  canReadMailRecipients?: boolean
  canReadMail?: boolean
  canCreateMail?: boolean
  canSendMail?: boolean
  canUpdateMail?: boolean
  canDeleteMail?: boolean
  dataScope?: { kind: string; departmentIds: string[] }
  rules?: { action: string; subject: string }[]
}

/** Same check as the OS `require_access`: the creator passes, otherwise a CASL rule must match. */
export function orgAllows(org: MemberOrg | undefined, action: string, subject: string): boolean {
  if (!org || org.membership === 'pending') return false
  if (org.creator) return true
  return (org.rules ?? []).some(
    (rule) =>
      (rule.action === 'manage' || rule.action === action) && (rule.subject === 'all' || rule.subject === subject)
  )
}

export function orgAllowsInvite(org: MemberOrg | undefined): boolean {
  return org?.canInviteMembers === true && org.dataScope?.kind === 'all' && orgAllows(org, 'invite', 'Member')
}

export function orgAllowsConfigureInvite(org: MemberOrg | undefined): boolean {
  return orgAllowsInvite(org) && orgAllows(org, 'configure', 'JoinInvite')
}

export function orgAllowsJoin(org: MemberOrg | undefined, action: 'approve' | 'reject'): boolean {
  return org?.canReviewJoin === true && orgAllows(org, action, 'JoinRequest')
}

/** 职位页需要角色上的新建、修改、删除或分配。通讯录基线里的 read 不能打开这一页。 */
const POSITION_PAGE_ACTIONS = ['create', 'update', 'delete', 'assign'] as const

/** 角色页需要分配、新建、修改、删除或审计权限。通讯录基线里的 read 不能打开这一页。 */
const ROLE_PAGE_ACTIONS = ['assign', 'create', 'update', 'delete', 'audit'] as const

export function canViewOrgPositions(org: MemberOrg | undefined): boolean {
  return POSITION_PAGE_ACTIONS.some((action) => orgAllows(org, action, 'Position'))
}

export function canViewOrgRoles(org: MemberOrg | undefined): boolean {
  return ROLE_PAGE_ACTIONS.some((action) => orgAllows(org, action, 'Role'))
}

/** Directory administration opens the full platform navigation. */
export function canManagePlatformDirectory(org: MemberOrg | undefined): boolean {
  if (!org?.isOfficial || org.membership === 'pending') return false
  return Boolean(org.creator) || canViewOrgPositions(org) || canViewOrgRoles(org) || orgAllows(org, 'invite', 'Member')
}

export function canSeePlatformAdmin(org: MemberOrg | undefined): boolean {
  if (!org?.isOfficial || org.membership === 'pending') return false
  return (
    canManagePlatformDirectory(org) ||
    orgAllows(org, 'read', 'PlatformSupplier') ||
    (org.dataScope?.kind === 'all' &&
      ['create', 'update', 'delete'].some((action) => orgAllows(org, action, 'PlatformSupplier')))
  )
}

export class MemberOrgError extends Error {
  /** HTTP status from the shell or the OS; absent when the request never left the renderer. */
  readonly status?: number

  constructor(message: string, status?: number) {
    super(message)
    this.name = 'MemberOrgError'
    this.status = status
  }
}

function readNamedCookie(name: string): string {
  if (typeof globalThis.document === 'undefined') return ''
  const prefix = `${name}=`
  const raw = globalThis.document.cookie.split('; ').find((item) => item.startsWith(prefix))
  if (!raw) return ''
  try {
    return decodeURIComponent(raw.slice(prefix.length)).trim()
  } catch {
    return ''
  }
}

function expireCookie(name: string) {
  if (typeof globalThis.document === 'undefined') return
  globalThis.document.cookie = `${name}=; Path=/; Max-Age=0; SameSite=Lax`
}

function readCenterCookie(): string {
  return readNamedCookie(CENTER_COOKIE)
}

/** 空字符串表示这个页面来源还没有选过团队。个人中心是显式写入的值。 */
export function readStoredCenter(): string {
  try {
    const stored = globalThis.localStorage.getItem(STORAGE_KEY)?.trim() ?? ''
    if (stored) return stored
    const legacy = globalThis.localStorage.getItem(LEGACY_STORAGE_KEY)?.trim() ?? ''
    if (legacy) {
      globalThis.localStorage.setItem(STORAGE_KEY, legacy)
      globalThis.localStorage.removeItem(LEGACY_STORAGE_KEY)
      expireCookie(LEGACY_CENTER_COOKIE)
      return legacy
    }
  } catch {
    /* private mode */
  }
  const cookie = readCenterCookie()
  if (cookie) return cookie
  const legacyCookie = readNamedCookie(LEGACY_CENTER_COOKIE)
  if (!legacyCookie) return ''
  storeCenterLocally(legacyCookie)
  return legacyCookie
}

export function readCenterId(): string {
  const stored = readStoredCenter()
  if (!stored) return PERSONAL_CENTER
  try {
    if (globalThis.localStorage.getItem(STORAGE_KEY) !== stored) globalThis.localStorage.setItem(STORAGE_KEY, stored)
  } catch {
    /* private mode */
  }
  return stored
}

function memberCenters(): Record<string, string> {
  try {
    let raw = globalThis.localStorage.getItem(CENTER_BY_MEMBER)
    if (!raw) {
      raw = globalThis.localStorage.getItem(LEGACY_CENTER_BY_MEMBER)
      if (raw) {
        globalThis.localStorage.setItem(CENTER_BY_MEMBER, raw)
        globalThis.localStorage.removeItem(LEGACY_CENTER_BY_MEMBER)
      }
    }
    if (!raw) return {}
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    const out: Record<string, string> = {}
    for (const [key, value] of Object.entries(parsed)) {
      if (MEMBER_ID.test(key) && typeof value === 'string' && value.trim()) out[key] = value.trim()
    }
    return out
  } catch {
    return {}
  }
}

function rememberMemberCenter(id: string) {
  if (!centerMemberId) return
  try {
    const map = memberCenters()
    map[centerMemberId] = id
    globalThis.localStorage.setItem(CENTER_BY_MEMBER, JSON.stringify(map))
  } catch {
    /* private mode */
  }
}

function storeCenterLocally(id: string) {
  try {
    globalThis.localStorage.setItem(STORAGE_KEY, id)
  } catch {
    /* private mode */
  }
  if (typeof globalThis.document !== 'undefined') {
    globalThis.document.cookie = `${CENTER_COOKIE}=${encodeURIComponent(id)}; Path=/; Max-Age=31536000; SameSite=Lax`
    expireCookie(LEGACY_CENTER_COOKIE)
  }
  if (typeof globalThis.window !== 'undefined') globalThis.window.dispatchEvent(new globalThis.Event(CENTER_EVENT))
}

export function writeCenterId(id: string) {
  storeCenterLocally(id)
  rememberMemberCenter(id)
  publishCenter(id)
}

/**
 * Remember a team inside this window only. A file-editor popout has its own
 * storage and must not POST the selection back to the shell.
 */
export function adoptEditorCenter(id: string): void {
  const trimmed = id.trim()
  if (trimmed === PERSONAL_CENTER || trimmed.length > 64 || !/^[0-9a-zA-Z_-]+$/u.test(trimmed)) return
  storeCenterLocally(trimmed)
}

/** Restores this member's team. A member with no saved team keeps the current slot once, then owns it. */
export function bindCenterMember(memberId: string) {
  const id = memberId.trim()
  if (!MEMBER_ID.test(id)) return
  centerMemberId = id
  const map = memberCenters()
  const known = Object.prototype.hasOwnProperty.call(map, id)
  const next = known ? map[id] : readCenterId()
  if (!known) rememberMemberCenter(next)
  if (readStoredCenter() !== next) writeCenterId(next)
  else publishCenter(next)
}

/** Drops the in-memory binding. Saved teams stay on disk for the next sign-in. */
export function releaseCenterMember() {
  centerMemberId = ''
}

/** Tells the shell which team is selected, so an applet window can read it. */
export function publishStoredCenter(): void {
  publishCenter(readCenterId())
}

function publishCenter(id: string) {
  let runtime: ReturnType<typeof readStudioRuntime>
  try {
    runtime = readStudioRuntime()
  } catch {
    return
  }
  if (!runtime || typeof globalThis.fetch !== 'function') return
  void globalThis
    .fetch(`${runtime.apiBaseUrl}/api/member-center`, {
      method: 'POST',
      cache: 'no-store',
      headers: {
        accept: 'application/json',
        authorization: `Bearer ${runtime.token}`,
        'content-type': 'application/json'
      },
      body: JSON.stringify({ id })
    })
    .catch(() => undefined)
}

export function subscribeCenter(listener: () => void) {
  if (typeof globalThis.window === 'undefined') return () => undefined
  const onStorage = (event: globalThis.StorageEvent) => {
    if (event.key === STORAGE_KEY) listener()
  }
  globalThis.window.addEventListener(CENTER_EVENT, listener)
  globalThis.window.addEventListener('storage', onStorage)
  return () => {
    globalThis.window.removeEventListener(CENTER_EVENT, listener)
    globalThis.window.removeEventListener('storage', onStorage)
  }
}

/** 已退出或不再加入的团队回到个人中心。申请中的成员资格不能选。 */
export function reconcileCenter(centerId: string, orgs: readonly MemberOrg[]): string {
  const joined = orgs.filter((org) => org.membership !== 'pending')
  if (centerId === PERSONAL_CENTER || joined.some((org) => org.id === centerId)) return centerId
  return PERSONAL_CENTER
}

export async function listMemberOrgs(fetchImpl: typeof globalThis.fetch = globalThis.fetch): Promise<MemberOrg[]> {
  const data = await memberRequest('/api/member-orgs', { method: 'GET' }, fetchImpl)
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.flatMap((row) => {
    const org = asOrg(row)
    return org ? [org] : []
  })
}

export async function listMemberDepartments(
  orgId: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<MemberDepartment[]> {
  const data = await memberRequest(`/api/member-departments/${encodeURIComponent(orgId)}`, { method: 'GET' }, fetchImpl)
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.flatMap((row) => {
    const department = asDepartment(row)
    return department ? [department] : []
  })
}

export type MemberAccount = {
  displayName: string
  phone: string
  /** 头像 data URL；空串表示用昵称首字。 */
  avatar?: string
  id?: string
}

export const MEMBER_NAME_MAX = 32

/** Name, phone and avatar from a member row. The session token stays out of this object. */
export function memberAccountFromSession(value: unknown): MemberAccount {
  if (!value || typeof value !== 'object') return { displayName: '', phone: '' }
  const row = value as Record<string, unknown>
  const displayName = typeof row.display_name === 'string' ? row.display_name.trim() : ''
  const phone = typeof row.phone === 'string' && /^\d{1,20}$/.test(row.phone) ? row.phone : ''
  const avatar = typeof row.avatar === 'string' && row.avatar.startsWith('data:image/') ? row.avatar : ''
  const id = typeof row.id === 'string' && MEMBER_ID.test(row.id) ? row.id : ''
  const account = avatar ? { displayName, phone, avatar } : { displayName, phone }
  return id ? { ...account, id } : account
}

/** The OS profile when reachable, otherwise what the saved session remembers. A denied session still throws. */
export async function readMemberAccount(fetchImpl: typeof globalThis.fetch = globalThis.fetch): Promise<MemberAccount> {
  try {
    return memberAccountFromSession(await memberRequest('/api/member/me', { method: 'GET' }, fetchImpl))
  } catch (error) {
    if (isSessionDenied(error)) throw error
    return memberAccountFromSession(await memberRequest('/api/member-session/profile', { method: 'GET' }, fetchImpl))
  }
}

export async function updateMemberAccount(
  profile: { displayName: string; avatar: string },
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<MemberAccount> {
  const data = await memberRequest(
    '/api/member/me',
    { method: 'PUT', body: JSON.stringify({ display_name: profile.displayName.trim(), avatar: profile.avatar }) },
    fetchImpl
  )
  return memberAccountFromSession(data)
}

/** The OS no longer accepts the saved member session; only signing in again helps. */
export function isSessionDenied(error: unknown): boolean {
  return error instanceof MemberOrgError && error.status === 401
}

export async function logoutMemberAccount(fetchImpl: typeof globalThis.fetch = globalThis.fetch): Promise<void> {
  await memberRequest('/api/member-session/logout', { method: 'POST', body: '{}' }, fetchImpl)
  releaseCenterMember()
  storeCenterLocally(PERSONAL_CENTER)
}

/** 回到桌面端登录页；运行时不可用时留在原地。 */
export function returnToLogin() {
  try {
    const runtime = readStudioRuntime()
    if (!runtime) return
    globalThis.window.location.assign(`${runtime.apiBaseUrl}/`)
  } catch {
    /* the caller keeps its current state */
  }
}

export async function createMemberOrg(
  name: string,
  logo = '',
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<MemberOrg> {
  const data = await memberRequest(
    '/api/member-orgs',
    { method: 'POST', body: JSON.stringify(logo ? { name, logo } : { name }) },
    fetchImpl
  )
  const org = asOrg(data)
  if (!org) throw new MemberOrgError('invalid')
  return org
}

const JOIN_CODE = /^[A-Za-z0-9]{1,64}$/

/** A raw join code, or the code at the end of an invite link (`…/join/{code}`). */
export function joinCodeFromInput(raw: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return ''
  const fromLink = codeInJoinPath(trimmed)
  if (fromLink) return fromLink
  return JOIN_CODE.test(trimmed) ? trimmed : ''
}

function codeInJoinPath(value: string): string {
  let path = ''
  try {
    path = new globalThis.URL(value).pathname
  } catch {
    const slash = value.indexOf('/')
    if (slash >= 0) path = value.slice(slash)
  }
  if (!path) return ''
  const parts = path.split('/').filter(Boolean)
  const at = parts.lastIndexOf('join')
  const code = at >= 0 ? parts[at + 1] : ''
  if (!code) return ''
  const bare = safeDecode(code).split('?')[0].split('#')[0]
  return JOIN_CODE.test(bare) ? bare : ''
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

export type DiscoverableOrg = {
  id: string
  name: string
  logo: string
  memberCount: number
  joined: boolean
  requested: boolean
}

/** Teams whose name contains the query. Empty and over-long queries are rejected upstream. */
export async function searchMemberOrgs(
  query: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<DiscoverableOrg[]> {
  const data = await memberRequest(
    `/api/member/discoverable-orgs?q=${encodeURIComponent(query.trim())}`,
    { method: 'GET' },
    fetchImpl
  )
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.flatMap((row) => {
    const org = asDiscoverable(row)
    return org ? [org] : []
  })
}

/** Ask to join. An admin still has to approve; this does not switch the center. */
export async function requestJoinMemberOrg(
  orgId: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<void> {
  await memberRequest(
    `/api/member/orgs/${encodeURIComponent(orgId)}/join-requests`,
    { method: 'POST', body: '{}' },
    fetchImpl
  )
}

/** Join the team behind a persistent invite code. The shell attaches the member session. */
export async function joinMemberOrg(
  code: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<MemberOrg> {
  const data = await memberRequest(
    `/api/member/join-codes/${encodeURIComponent(code)}/memberships`,
    { method: 'POST', body: '{}' },
    fetchImpl
  )
  const row = data && typeof data === 'object' && 'org' in data ? data.org : data
  const org = asOrg(row)
  if (!org) throw new MemberOrgError('invalid')
  return org
}

/** Member-plane call through the Studio shell, which attaches the stored member session. */
export async function memberRequest(
  path: string,
  init: { method: 'GET' | 'POST' | 'PUT' | 'DELETE'; body?: string },
  fetchImpl: typeof globalThis.fetch
): Promise<unknown> {
  let runtime: ReturnType<typeof readStudioRuntime>
  try {
    runtime = readStudioRuntime()
  } catch {
    throw new MemberOrgError('runtime')
  }
  if (!runtime) throw new MemberOrgError('runtime')
  const response = await fetchImpl(`${runtime.apiBaseUrl}${path}`, {
    method: init.method,
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`,
      ...(init.body ? { 'content-type': 'application/json' } : {})
    },
    body: init.body
  })
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text) as unknown
    } catch {
      throw new MemberOrgError(text || `HTTP ${response.status}`, response.status)
    }
  }
  return unwrap(payload, response.status)
}

function unwrap(payload: unknown, status: number): unknown {
  if (status >= 400) {
    const message =
      payload &&
      typeof payload === 'object' &&
      'message' in payload &&
      typeof payload.message === 'string' &&
      payload.message.trim()
        ? payload.message
        : `HTTP ${status}`
    throw new MemberOrgError(message, status)
  }
  if (payload && typeof payload === 'object' && 'code' in payload && 'data' in payload) {
    const record = payload as { code?: unknown; message?: unknown; data?: unknown; success?: unknown }
    const code = typeof record.code === 'number' ? record.code : status
    const ok = (code === 0 || (code >= 200 && code < 300)) && record.success !== false
    if (!ok) {
      const message = typeof record.message === 'string' && record.message ? record.message : `API error ${code}`
      throw new MemberOrgError(message, status >= 400 ? status : code)
    }
    return record.data
  }

  return payload
}

function asDepartment(value: unknown): MemberDepartment | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id || typeof row.name !== 'string' || !row.name.trim()) return null
  return {
    id: row.id,
    parent_id: typeof row.parent_id === 'string' ? row.parent_id : null,
    name: row.name,
    sort_order: typeof row.sort_order === 'number' ? row.sort_order : 0
  }
}

function asDiscoverable(value: unknown): DiscoverableOrg | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id || typeof row.name !== 'string' || !row.name.trim()) return null
  return {
    id: row.id,
    name: row.name,
    logo: typeof row.logo === 'string' ? row.logo : '',
    memberCount: typeof row.member_count === 'number' ? row.member_count : 0,
    joined: row.joined === true,
    requested: row.requested === true
  }
}

function asOrg(value: unknown): MemberOrg | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id || typeof row.name !== 'string' || !row.name.trim()) return null
  const membership =
    row.membership === 'admin' || row.membership === 'member' || row.membership === 'pending'
      ? row.membership
      : undefined
  const rules = Array.isArray(row.rules)
    ? row.rules.flatMap((rule) => {
        if (!rule || typeof rule !== 'object') return []
        const { action, subject } = rule as Record<string, unknown>
        return typeof action === 'string' && typeof subject === 'string' ? [{ action, subject }] : []
      })
    : []
  const scope = row.data_scope as { kind?: unknown; department_ids?: unknown } | null | undefined
  const dataScope =
    scope &&
    typeof scope.kind === 'string' &&
    ['self', 'department', 'department_tree', 'custom', 'custom_tree', 'all'].includes(scope.kind) &&
    Array.isArray(scope.department_ids) &&
    scope.department_ids.every((id): id is string => typeof id === 'string')
      ? { kind: scope.kind, departmentIds: scope.department_ids }
      : undefined
  return {
    id: row.id,
    name: row.name,
    logo: typeof row.logo === 'string' ? row.logo : '',
    membership,
    creator: row.creator === true,
    isOfficial: row.is_official === true,
    industry: typeof row.industry === 'string' ? row.industry : '',
    scale: typeof row.scale === 'string' ? row.scale : '',
    memberCount: typeof row.member_count === 'number' ? row.member_count : 0,
    canReadWork: row.can_read_work === true,
    canCreateWork: row.can_create_work === true,
    canReviewJoin: row.can_review_join === true,
    canInviteMembers: row.can_invite_members === true,
    canUpdateOrg: row.can_update_org === true,
    canTransferOrg: row.can_transfer_org === true,
    canDeleteOrg: row.can_delete_org === true,
    canLeaveOrg: row.can_leave_org === true,
    canReadSchedule: row.can_read_schedule === true,
    canCreateSchedule: row.can_create_schedule === true,
    canUpdateSchedule: row.can_update_schedule === true,
    canDeleteSchedule: row.can_delete_schedule === true,
    canReadMailRecipients: row.can_read_mail_recipients === true,
    canReadMail: row.can_read_mail === true,
    canCreateMail: row.can_create_mail === true,
    canSendMail: row.can_send_mail === true,
    canUpdateMail: row.can_update_mail === true,
    canDeleteMail: row.can_delete_mail === true,
    dataScope,
    rules
  }
}
