import { memberRequest } from '@renderer/data/memberOrgs'

type MemberMethod = 'GET' | 'POST' | 'PUT' | 'DELETE'

async function adminRequest<T>(path: string, init: { method: MemberMethod; body?: string }): Promise<T> {
  return (await memberRequest(path, init, fetch)) as T
}

const orgPath = (orgId: string, tail: string): string => `/api/member/orgs/${encodeURIComponent(orgId)}/${tail}`

// ── 成员状态 ─────────────────────────────────────────────────────

export const MEMBER_STATUSES = ['active', 'invited', 'disabled', 'resigned'] as const
export type MemberStatus = (typeof MEMBER_STATUSES)[number]

export function isMemberStatus(value: unknown): value is MemberStatus {
  return typeof value === 'string' && (MEMBER_STATUSES as readonly string[]).includes(value)
}

export function setOrgMemberStatus(orgId: string, userId: string, status: MemberStatus) {
  return adminRequest<{ ok: boolean }>(orgPath(orgId, `members/${encodeURIComponent(userId)}/status`), {
    method: 'PUT',
    body: JSON.stringify({ status })
  })
}

// ── 手机号邀请 ───────────────────────────────────────────────────

export type MemberInvite = {
  id: string
  revision: string
  org_id: string
  phone: string
  role_code: string | null
  code: string
  status: 'pending' | 'accepted' | 'revoked'
  created_at: string
  accepted_at: string | null
}

/** 中国大陆手机号：可带 +86，11 位，1 开头。最终以服务端为准。 */
export function isInvitePhone(value: string) {
  return /^(\+?86)?1\d{10}$/.test(value)
}

export function listMemberInvites(orgId: string) {
  return adminRequest<MemberInvite[]>(`/api/member/invites?org_id=${encodeURIComponent(orgId)}`, { method: 'GET' })
}

export function createMemberInvite(orgId: string, phone: string, roleCode?: string) {
  return adminRequest<MemberInvite>('/api/member/invites', {
    method: 'POST',
    body: JSON.stringify({ org_id: orgId, phone, ...(roleCode ? { role_code: roleCode } : {}) })
  })
}

export function revokeMemberInvite(invite: Pick<MemberInvite, 'id' | 'revision'>) {
  return adminRequest<{ ok: boolean }>(`/api/member/invites/${encodeURIComponent(invite.id)}`, {
    method: 'DELETE',
    body: JSON.stringify({ expected_revision: invite.revision })
  })
}

// ── 套餐与服务 ───────────────────────────────────────────────────

export const SELF_SERVE_PLANS = ['free', 'builder', 'team'] as const
export type PlanId = (typeof SELF_SERVE_PLANS)[number] | 'enterprise'

/** Owner seat bounds and defaults (member `SubscriptionPlan`). */
export const PLAN_SEATS: Record<PlanId, { min: number; max: number; default: number }> = {
  free: { min: 1, max: 1, default: 1 },
  builder: { min: 1, max: 500, default: 5 },
  team: { min: 5, max: 500, default: 10 },
  enterprise: { min: 1, max: 100000, default: 25 }
}

export function isPlanId(value: unknown): value is PlanId {
  return typeof value === 'string' && value in PLAN_SEATS
}

export type OrgSubscription = {
  org_id: string
  plan_id: string
  status: 'active' | 'cancelled' | string
  seats: number
  period_start: string
  period_end: string | null
  self_serve: boolean
}

export type EntitlementRoute = {
  hostname: string
  path_prefix: string
  state: string
  project_name: string
  environment_name: string
  workload_name: string | null
  workload_health: string | null
}

export function fetchOrgSubscription(orgId: string) {
  return adminRequest<OrgSubscription>(orgPath(orgId, 'subscription'), { method: 'GET' })
}

export function updateOrgSubscription(orgId: string, planId: PlanId, seats: number) {
  return adminRequest<OrgSubscription>(orgPath(orgId, 'subscription'), {
    method: 'PUT',
    body: JSON.stringify({ plan_id: planId, seats })
  })
}

export function listOrgEntitlements(orgId: string) {
  return adminRequest<EntitlementRoute[]>(orgPath(orgId, 'entitlements'), { method: 'GET' })
}

export type SeatSummary = { used: number; total: number; planId: string }

/** 已用席位：在职成员，与服务端接受邀请、通过申请时的口径一致。待接受邀请、停用、离职都不占。 */
export function usedSeats(members: readonly { status: string }[]) {
  return members.filter((row) => row.status === 'active').length
}

// ── 企业认证 ─────────────────────────────────────────────────────

export type VerificationStatus = 'unverified' | 'pending' | 'verified' | 'rejected'

export type OrgVerification = {
  status: VerificationStatus
  enterprise_name: string
  credit_code: string
  legal_person: string
  reject_reason: string
}

export type PersonalVerification = {
  status: VerificationStatus
  real_name: string
  id_last4: string
  reject_reason: string
}

const VERIFICATION_STATUSES: ReadonlySet<string> = new Set(['unverified', 'pending', 'verified', 'rejected'])

function verificationOf<T extends { status: VerificationStatus }>(value: unknown, fields: readonly string[]): T {
  const row = value as Record<string, unknown> | null
  if (
    !row ||
    typeof row !== 'object' ||
    typeof row.status !== 'string' ||
    !VERIFICATION_STATUSES.has(row.status) ||
    fields.some((field) => typeof row[field] !== 'string')
  ) {
    throw new Error('verification response is malformed')
  }
  return row as T
}

const ORG_VERIFICATION_FIELDS = ['enterprise_name', 'credit_code', 'legal_person', 'reject_reason'] as const
const PERSONAL_VERIFICATION_FIELDS = ['real_name', 'id_last4', 'reject_reason'] as const

export async function fetchOrgVerification(orgId: string) {
  const row = await adminRequest<unknown>(orgPath(orgId, 'verification'), { method: 'GET' })
  return verificationOf<OrgVerification>(row, ORG_VERIFICATION_FIELDS)
}

export async function fetchPersonalVerification() {
  const row = await adminRequest<unknown>('/api/member/verification', { method: 'GET' })
  return verificationOf<PersonalVerification>(row, PERSONAL_VERIFICATION_FIELDS)
}

export async function submitOrgVerification(
  orgId: string,
  input: { enterprise_name: string; credit_code: string; legal_person: string }
) {
  const row = await adminRequest<unknown>(orgPath(orgId, 'verification'), {
    method: 'POST',
    body: JSON.stringify(input)
  })
  return verificationOf<OrgVerification>(row, ORG_VERIFICATION_FIELDS)
}

export async function submitPersonalVerification(input: { real_name: string; id_number: string }) {
  const row = await adminRequest<unknown>('/api/member/verification', {
    method: 'POST',
    body: JSON.stringify(input)
  })
  return verificationOf<PersonalVerification>(row, PERSONAL_VERIFICATION_FIELDS)
}

const CREDIT_CHARSET = '0123456789ABCDEFGHJKLMNPQRTUWXY'
const CREDIT_WEIGHTS = [1, 3, 9, 27, 19, 26, 16, 17, 20, 29, 25, 13, 8, 24, 10, 30, 28]

/** 统一社会信用代码（GB 32100），与服务端 `normalize_credit_code` 同一校验位算法。 */
export function isCreditCode(value: string) {
  const code = value.trim().toUpperCase()
  if (code.length !== 18) return false
  let sum = 0
  for (let index = 0; index < 17; index += 1) {
    const position = CREDIT_CHARSET.indexOf(code[index])
    if (position < 0) return false
    sum += position * CREDIT_WEIGHTS[index]
  }
  return code[17] === CREDIT_CHARSET[(31 - (sum % 31)) % 31]
}

// ── 个人访问令牌 ─────────────────────────────────────────────────

export const TOKEN_SCOPES = ['repository:read', 'repository:write'] as const
export type TokenScope = (typeof TOKEN_SCOPES)[number]
export const TOKEN_EXPIRY_DAYS = [7, 30, 60, 90] as const

export type AccessToken = {
  id: string
  name: string
  scopes: TokenScope[]
  created_at: string
  expires_at: string
  revoked_at: string | null
  last_used_at: string | null
}

export type IssuedAccessToken = AccessToken & { token: string }

export function listAccessTokens() {
  return adminRequest<AccessToken[]>('/api/member/access-tokens', { method: 'GET' })
}

export function createAccessToken(input: { name: string; scopes: TokenScope[]; expires_in_days: number }) {
  return adminRequest<IssuedAccessToken>('/api/member/access-tokens', {
    method: 'POST',
    body: JSON.stringify(input)
  })
}

export function revokeAccessToken(id: string) {
  return adminRequest<{ ok: boolean }>(`/api/member/access-tokens/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export type TokenState = 'active' | 'expired' | 'revoked'

export function tokenState(token: Pick<AccessToken, 'revoked_at' | 'expires_at'>, now = Date.now()): TokenState {
  if (token.revoked_at) return 'revoked'
  return Date.parse(token.expires_at) <= now ? 'expired' : 'active'
}

// ── 审计日志 ─────────────────────────────────────────────────────

export const AUDIT_KINDS = ['member', 'role', 'invitation', 'organization', 'repository'] as const
export type AuditKind = (typeof AUDIT_KINDS)[number]
export const AUDIT_WINDOWS = [7, 30, 90] as const
export type AuditWindow = (typeof AUDIT_WINDOWS)[number]
export const AUDIT_PAGE_SIZE = 50

export type AuditActor = { kind: 'member' | 'operator'; member_id: string | null; name: string | null }

export type AuditTarget = {
  kind: 'member' | 'applicant' | 'role' | 'invitation' | 'organization' | 'repository'
  id: string | null
  name: string | null
  phone: string | null
  repository: string | null
  change_request: number | null
}

export type AuditValue = {
  kind: 'status' | 'plan' | 'name' | 'department' | 'position' | 'capabilities'
  code: string | null
  seats: number | null
  /** 团队能力：允许清单中的条目数。 */
  count?: number | null
  id: string | null
  label: string | null
}

export type AuditEntry = {
  id: string
  occurred_at: string
  action: string
  kind: AuditKind
  actor: AuditActor
  target: AuditTarget
  from: AuditValue | null
  to: AuditValue | null
}

export type AuditCursor = { before_at: string; before_id: string }
export type AuditPage = { entries: AuditEntry[]; next: AuditCursor | null }
export type AuditFilter = { kind: AuditKind | null; actor: string | null; days: AuditWindow }

export function listTeamAudit(orgId: string, filter: AuditFilter, cursor: AuditCursor | null) {
  const query = new URLSearchParams({ days: String(filter.days), limit: String(AUDIT_PAGE_SIZE) })
  if (filter.kind) query.set('kind', filter.kind)
  if (filter.actor) query.set('actor', filter.actor)
  if (cursor) {
    query.set('before_at', cursor.before_at)
    query.set('before_id', cursor.before_id)
  }
  return adminRequest<AuditPage>(orgPath(orgId, `audit?${query}`), { method: 'GET' })
}

/** CSV 的列固定，导出内容与当前已加载的行一致。 */
export const AUDIT_CSV_COLUMNS = [
  'occurred_at',
  'actor_kind',
  'actor_member_id',
  'actor_name',
  'action',
  'kind',
  'target_kind',
  'target_id',
  'target_name',
  'target_phone',
  'target_repository',
  'change_request',
  'from',
  'to'
] as const

function auditValueText(value: AuditValue | null): string {
  if (!value) return ''
  switch (value.kind) {
    case 'plan':
      return `${value.code ?? ''}:${value.seats ?? ''}`
    case 'status':
      return value.code ?? ''
    case 'capabilities':
      return `${value.code ?? ''}:${value.count ?? ''}`
    default:
      return value.label ?? value.id ?? ''
  }
}

/** 引号包住每个字段；以公式字符开头的值加单引号，避免表格软件把它当公式执行。 */
function csvField(raw: string): string {
  const value = /^[=+\-@\t\r]/.test(raw) ? `'${raw}` : raw
  return `"${value.replace(/"/g, '""')}"`
}

export function auditCsv(entries: readonly AuditEntry[]): string {
  const rows = entries.map((entry) =>
    [
      entry.occurred_at,
      entry.actor.kind,
      entry.actor.member_id ?? '',
      entry.actor.name ?? '',
      entry.action,
      entry.kind,
      entry.target.kind,
      entry.target.id ?? '',
      entry.target.name ?? '',
      entry.target.phone ?? '',
      entry.target.repository ?? '',
      entry.target.change_request === null ? '' : String(entry.target.change_request),
      auditValueText(entry.from),
      auditValueText(entry.to)
    ]
      .map(csvField)
      .join(',')
  )
  return [AUDIT_CSV_COLUMNS.join(','), ...rows].join('\r\n') + '\r\n'
}

// ── 运行 ─────────────────────────────────────────────────────────

export const RUN_PAGE_SIZE = 20
/** Cloud 智能体执行的状态，闭集。其余值按原文显示。 */
export const RUN_STATUSES = [
  'pending',
  'running',
  'awaiting_approval',
  'cancelling',
  'succeeded',
  'failed',
  'cancelled'
] as const
export type RunStatus = (typeof RUN_STATUSES)[number]
const STOPPABLE: ReadonlySet<string> = new Set(['pending', 'running', 'awaiting_approval'])

export type TeamRun = {
  execution_id: string
  conversation_id: string
  agent_asset_id: string
  started_by: { member_id: string; name: string | null }
  started_at: string
  /** null：Cloud 中已无此运行。 */
  status: string | null
}

export type RunCursor = { before_at: string; before_id: string }
export type RunPage = { runs: TeamRun[]; next: RunCursor | null }
export type RunStop = { execution_id: string; status: string; replayed: boolean }

export function isRunStatus(value: unknown): value is RunStatus {
  return typeof value === 'string' && (RUN_STATUSES as readonly string[]).includes(value)
}

export function runStoppable(status: string | null): boolean {
  return status !== null && STOPPABLE.has(status)
}

export function listTeamRuns(orgId: string, cursor: RunCursor | null) {
  const query = new URLSearchParams({ limit: String(RUN_PAGE_SIZE) })
  if (cursor) {
    query.set('before_at', cursor.before_at)
    query.set('before_id', cursor.before_id)
  }
  return adminRequest<RunPage>(orgPath(orgId, `runs?${query}`), { method: 'GET' })
}

export function teamRunLogs(orgId: string, executionId: string, cursor: string | null) {
  const query = cursor ? `?${new URLSearchParams({ cursor })}` : ''
  return adminRequest<unknown>(orgPath(orgId, `runs/${encodeURIComponent(executionId)}/logs${query}`), {
    method: 'GET'
  })
}

/** 同一个 requestId 重试不会重复停止。 */
export function stopTeamRun(orgId: string, executionId: string, requestId: string) {
  return adminRequest<RunStop>(orgPath(orgId, `runs/${encodeURIComponent(executionId)}/stop`), {
    method: 'POST',
    body: JSON.stringify({ request_id: requestId })
  })
}

export type RunLogLine = { key: string; kind: string; text: string }
export type RunLogPage = { lines: RunLogLine[]; next: string | null }

function textOf(value: unknown): string {
  if (typeof value === 'string') return value
  if (value && typeof value === 'object') {
    const row = value as Record<string, unknown>
    for (const field of ['text', 'reason', 'message']) {
      if (typeof row[field] === 'string' && row[field]) return row[field]
    }
  }
  return value === undefined || value === null ? '' : JSON.stringify(value)
}

/** Cloud 轨迹页 `{records, nextCursor}`，或包在 `data` 里的同一对象。 */
export function runLogPage(value: unknown): RunLogPage {
  const root = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  const page = root.data && typeof root.data === 'object' ? (root.data as Record<string, unknown>) : root
  const records = Array.isArray(page.records) ? page.records : []
  const lines = records.map((record, index): RunLogLine => {
    const row = record && typeof record === 'object' ? (record as Record<string, unknown>) : {}
    const sequence = typeof row.sequence === 'number' ? row.sequence : index
    return {
      key: String(sequence),
      kind: typeof row.kind === 'string' ? row.kind : '',
      text: textOf(row.content)
    }
  })
  const next = typeof page.nextCursor === 'string' && page.nextCursor ? page.nextCursor : null
  return { lines, next }
}

// ── 服务端答复 → 文案键（逐字匹配所有者契约里的固定答复）─────────

const ADMIN_ERRORS: Record<string, string> = {
  'cannot change your own status': 'org.status.selfChange',
  'a resigned member must be invited again': 'org.status.resignedFinal',
  'the enterprise must keep at least one admin': 'org.members.lastAdmin',
  'a pending invite for this phone already exists': 'org.invites.duplicate',
  'invitation changed; reload before revoking': 'org.invites.changed',
  'invite is no longer pending': 'org.invites.changed',
  'credit code is already used': 'org.verification.creditUsed',
  'valid Chinese mobile number is required': 'org.invites.phoneInvalid',
  'self-serve plan change is disabled; contact us': 'org.plan.selfServeOff',
  'every seat is taken; add seats or disable a member first': 'org.seats.full',
  'seats cannot be fewer than active members; disable members first': 'org.seats.belowActive',
  'enterprise plan is provisioned by sales; contact us': 'org.plan.enterpriseSales',
  'personal verification is required': 'org.verification.personalRequired',
  'only the creator can submit enterprise verification': 'org.verification.creatorOnly',
  'verification is incomplete': 'org.verification.incomplete',
  'credit code is invalid': 'org.verification.creditInvalid',
  'enterprise name is required': 'org.verification.enterpriseNameInvalid',
  'legal person is required': 'org.verification.legalPersonInvalid',
  'real name is required': 'org.verification.realNameInvalid',
  'id number is invalid': 'org.verification.idInvalid',
  'verification is already pending': 'org.verification.alreadyPending',
  'verification is already complete': 'org.verification.alreadyVerified'
}

export function adminErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message.trim() : ''
  return ADMIN_ERRORS[message] ?? 'org.error.generic'
}

// ── 团队能力 ─────────────────────────────────────────────────────

export type CapabilityAllowanceKind = 'connector' | 'skill' | 'plugin'
export type CapabilityAllowance = { kind: CapabilityAllowanceKind; slug: string }

export type TeamCapabilityPolicy = {
  restricted: boolean
  allowed: CapabilityAllowance[]
  /** 团队第一次保存前为 0。 */
  revision: number
  updated_by: string | null
  updated_at: string | null
}

export const TEAM_CAPABILITY_ALLOWLIST_MAX = 500

export function getCapabilityPolicy(orgId: string) {
  return adminRequest<TeamCapabilityPolicy>(orgPath(orgId, 'capability-policy'), { method: 'GET' })
}

export function saveCapabilityPolicy(
  orgId: string,
  next: { restricted: boolean; allowed: readonly CapabilityAllowance[]; expected_revision: number }
) {
  return adminRequest<TeamCapabilityPolicy>(orgPath(orgId, 'capability-policy'), {
    method: 'PUT',
    body: JSON.stringify({
      restricted: next.restricted,
      allowed: next.allowed,
      expected_revision: next.expected_revision
    })
  })
}

/** 同一个能力只记一次；顺序与服务端一致（类型、标识）。 */
export function allowanceKey(entry: CapabilityAllowance): string {
  return `${entry.kind}:${entry.slug}`
}

export function samePolicy(
  left: { restricted: boolean; allowed: readonly CapabilityAllowance[] },
  right: { restricted: boolean; allowed: readonly CapabilityAllowance[] }
): boolean {
  if (left.restricted !== right.restricted || left.allowed.length !== right.allowed.length) return false
  const keys = new Set(left.allowed.map(allowanceKey))
  return right.allowed.every((entry) => keys.has(allowanceKey(entry)))
}
