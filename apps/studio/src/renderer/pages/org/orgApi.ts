import { memberRequest } from '@renderer/data/memberOrgs'

/** 组织架构中的一个部门。字段与 a3s os 的 OrgDepartmentDto 对齐。 */
export type OrgDepartment = {
  id: string
  parent_id: string | null
  name: string
  sort_order: number
}

export type OrgMemberRow = {
  id: string
  member_id?: string | null
  name: string
  email: string
  phone: string | null
  /** Present when the member saved a photo. Empty means the messages page draws nickname or phone text. */
  avatar?: string
  status: string
  admin: boolean
  creator: boolean
  department_id: string
  joined_at: string
  position_id: string | null
  position_name: string | null
  role_code: string
  role_name: string
  role_built_in: boolean
  can_remove?: boolean
  can_update_status?: boolean
  can_assign_position?: boolean
  can_assign_department?: boolean
  assignable_department_ids?: string[]
  can_assign_role?: boolean
  assignable_role_ids?: string[]
}

export type OrgAccessRule = {
  action: string
  subject: string
}

export type OrgDataScope = {
  kind: 'self' | 'department' | 'department_tree' | 'custom' | 'custom_tree' | 'all'
  department_ids: string[]
}

export type OrgRole = {
  id: string
  policy_revision?: string
  code: string
  name: string
  description: string
  built_in: boolean
  sort_order: number
  member_count: number
  rules?: OrgAccessRule[]
  data_scope?: OrgDataScope
  can_update?: boolean
  can_delete?: boolean
}

export type OrgPosition = {
  id: string
  name: string
  sort_order: number
}

export type OrgRolePolicySnapshot = {
  id: string
  name: string
  description: string
  rules: OrgAccessRule[]
  data_scope_kind: OrgDataScope['kind']
  scope_department_ids: string[]
}

export type OrgRoleAudit = {
  id: string
  actor_member_id: string
  action: 'role_created' | 'role_updated' | 'role_deleted'
  occurred_at: string
  before: OrgRolePolicySnapshot | null
  after: OrgRolePolicySnapshot | null
}

export function listRolePolicyAudits(orgId: string, roleId: string) {
  return orgRequest<OrgRoleAudit[]>(orgPath(orgId, `roles/${encodeURIComponent(roleId)}/audit`), { method: 'GET' })
}

export type OrgRoleAssignmentSnapshot = {
  department_id: string
  role: OrgRolePolicySnapshot
  administrator: boolean
}

export type OrgRoleAssignmentAudit = {
  id: string
  actor_member_id: string
  actor_user_id: string
  target_member_id: string
  target_user_id: string
  authority: 'assign_role' | 'update_member'
  admin_change: 'keep' | 'grant' | 'revoke'
  occurred_at: string
  before: OrgRoleAssignmentSnapshot
  after: OrgRoleAssignmentSnapshot
}

export function listRoleAssignmentAudits(orgId: string, userId: string) {
  return orgRequest<OrgRoleAssignmentAudit[]>(orgPath(orgId, `members/${encodeURIComponent(userId)}/role-audit`), {
    method: 'GET'
  })
}

export type OrgJoinInvite = {
  org_id: string
  revision?: string
  code: string
  enabled: boolean
  expires_at: string | null
  rotated_at: string
}

export type PendingApplicant = {
  member_id: string
  name: string
}

type MemberMethod = 'GET' | 'POST' | 'PUT' | 'DELETE'

async function orgRequest<T>(path: string, init: { method: MemberMethod; body?: string }): Promise<T> {
  return (await memberRequest(path, init, fetch)) as T
}

const orgPath = (orgId: string, tail = ''): string =>
  `/api/member/orgs/${encodeURIComponent(orgId)}${tail ? `/${tail}` : ''}`

// ── 部门 ─────────────────────────────────────────────────────────

export function listOrgDepartments(orgId: string) {
  return orgRequest<OrgDepartment[]>(orgPath(orgId, 'departments'), { method: 'GET' })
}

export function createOrgDepartment(orgId: string, input: { parent_id: string; name: string }) {
  return orgRequest<OrgDepartment>(orgPath(orgId, 'departments'), {
    method: 'POST',
    body: JSON.stringify(input)
  })
}

export function renameOrgDepartment(orgId: string, departmentId: string, name: string) {
  return orgRequest<OrgDepartment>(orgPath(orgId, `departments/${encodeURIComponent(departmentId)}`), {
    method: 'PUT',
    body: JSON.stringify({ name })
  })
}

export function deleteOrgDepartment(orgId: string, departmentId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `departments/${encodeURIComponent(departmentId)}`), {
    method: 'DELETE'
  })
}

// ── 成员 / 角色 / 职位 ───────────────────────────────────────────

export function listOrgMembers(orgId: string) {
  return orgRequest<OrgMemberRow[]>(orgPath(orgId, 'members'), { method: 'GET' })
}

export function listOrgRoles(orgId: string) {
  return orgRequest<OrgRole[]>(orgPath(orgId, 'roles'), { method: 'GET' })
}

export function createOrgRole(
  orgId: string,
  input: { name: string; description?: string; rules?: OrgAccessRule[]; data_scope: OrgDataScope }
) {
  return orgRequest<OrgRole>(orgPath(orgId, 'roles'), {
    method: 'POST',
    body: JSON.stringify(input)
  })
}

export function updateOrgRole(
  orgId: string,
  roleId: string,
  input: {
    name: string
    description?: string
    rules?: OrgAccessRule[]
    data_scope?: OrgDataScope
    expected_revision: string
  }
) {
  return orgRequest<OrgRole>(orgPath(orgId, `roles/${encodeURIComponent(roleId)}`), {
    method: 'PUT',
    body: JSON.stringify(input)
  })
}

export function deleteOrgRole(orgId: string, roleId: string, expectedRevision: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `roles/${encodeURIComponent(roleId)}`), {
    method: 'DELETE',
    body: JSON.stringify({ expected_revision: expectedRevision })
  })
}

export function listOrgPositions(orgId: string) {
  return orgRequest<OrgPosition[]>(orgPath(orgId, 'positions'), { method: 'GET' })
}

export function createOrgPosition(orgId: string, input: { name: string }) {
  return orgRequest<OrgPosition>(orgPath(orgId, 'positions'), {
    method: 'POST',
    body: JSON.stringify(input)
  })
}

export function renameOrgPosition(orgId: string, positionId: string, name: string) {
  return orgRequest<OrgPosition>(orgPath(orgId, `positions/${encodeURIComponent(positionId)}`), {
    method: 'PUT',
    body: JSON.stringify({ name })
  })
}

export function deleteOrgPosition(orgId: string, positionId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `positions/${encodeURIComponent(positionId)}`), {
    method: 'DELETE'
  })
}

export function setOrgMemberRole(orgId: string, userId: string, roleId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `members/${encodeURIComponent(userId)}/role`), {
    method: 'PUT',
    body: JSON.stringify({ role_id: roleId })
  })
}

export function setOrgMemberPosition(orgId: string, userId: string, positionId: string | null) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `members/${encodeURIComponent(userId)}/position`), {
    method: 'PUT',
    body: JSON.stringify({ position_id: positionId })
  })
}

export function setOrgMemberDepartment(orgId: string, userId: string, departmentId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `members/${encodeURIComponent(userId)}/department`), {
    method: 'PUT',
    body: JSON.stringify({ department_id: departmentId })
  })
}

export function removeOrgMember(orgId: string, userId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `members/${encodeURIComponent(userId)}`), {
    method: 'DELETE'
  })
}

// ── 加入申请 / 持久加入码 ────────────────────────────────────────

export function listJoinRequests(orgId: string) {
  return orgRequest<PendingApplicant[]>(orgPath(orgId, 'join-requests'), { method: 'GET' })
}

export function approveJoinRequest(orgId: string, memberId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `join-requests/${encodeURIComponent(memberId)}/approval`), {
    method: 'PUT'
  })
}

export function rejectJoinRequest(orgId: string, memberId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, `join-requests/${encodeURIComponent(memberId)}/rejection`), {
    method: 'PUT'
  })
}

export function fetchJoinInvite(orgId: string) {
  return orgRequest<OrgJoinInvite>(orgPath(orgId, 'join-invite'), { method: 'GET' })
}

export function isInvitationRevision(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
}

export function configureJoinInvite(
  orgId: string,
  input: { expected_revision: string; enabled?: boolean; rotate?: boolean }
) {
  if (!isInvitationRevision(input.expected_revision)) throw new Error('Invitation revision is unavailable')
  return orgRequest<OrgJoinInvite>(orgPath(orgId, 'join-invite'), {
    method: 'PUT',
    body: JSON.stringify(input)
  })
}

// ── 团队设置 ────────────────────────────────────────────────────

export type OrgProfileInput = { name?: string; industry?: string; scale?: string; logo?: string }

export type OrgProfile = {
  id: string
  name: string
  industry: string
  scale: string
  logo: string
}

export type TransferCandidate = {
  id: string
  name: string
  phone: string | null
  /** 头像 data URL。空串或缺失时，下拉用成员目录里的头像，再没有则显示昵称首字。 */
  avatar?: string
}

export function updateOrgProfile(orgId: string, input: OrgProfileInput) {
  return orgRequest<OrgProfile>(orgPath(orgId), { method: 'PUT', body: JSON.stringify(input) })
}

export function dissolveOrg(orgId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId), { method: 'DELETE' })
}

export function leaveOrg(orgId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, 'membership'), { method: 'DELETE' })
}

export function listTransferCandidates(orgId: string) {
  return orgRequest<TransferCandidate[]>(orgPath(orgId, 'transfer-candidates'), { method: 'GET' })
}

export function transferOrg(orgId: string, userId: string) {
  return orgRequest<{ ok: boolean }>(orgPath(orgId, 'ownership-transfers'), {
    method: 'POST',
    body: JSON.stringify({ user_id: userId })
  })
}

/**
 * a3s os 侧站点 origin，用于拼加入链接（网页版取 window.location.origin，
 * studio 的页面 origin 是 shell loopback，不能直接用）。
 */
export async function readOrgOrigin(): Promise<string> {
  const row = await orgRequest<{ orgOrigin?: string }>('/api/bootstrap', { method: 'GET' })
  return typeof row.orgOrigin === 'string' ? row.orgOrigin : ''
}

// ── 后端错误 → 文案键（与网页版同一组正则）──────────────────────

export function departmentErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message : ''
  if (/root department/i.test(message)) return 'org.members.deptRoot'
  if (/child departments/i.test(message)) return 'org.members.deptHasChildren'
  if (/still has users/i.test(message)) return 'org.members.deptHasMembers'
  if (/department name/i.test(message)) return 'org.members.deptNameInvalid'
  return 'org.error.generic'
}

export function memberErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message : ''
  if (/at least one admin/i.test(message)) return 'org.members.lastAdmin'
  if (/cannot remove yourself/i.test(message)) return 'org.members.selfRemove'
  if (/creator role cannot be changed|only the creator can hold/i.test(message)) return 'org.roles.ownerLocked'
  if (/department not found|primary department must belong/i.test(message)) return 'org.members.deptMissing'
  return 'org.error.generic'
}

export function positionErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message : ''
  if (/position name already exists/i.test(message)) return 'org.positions.duplicate'
  if (/position name is required/i.test(message)) return 'org.positions.nameInvalid'
  if (/position still has members/i.test(message)) return 'org.positions.hasMembers'
  return 'org.error.generic'
}

export function settingsErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message : ''
  if (/org name is required/i.test(message)) return 'org.settings.nameRequired'
  if (/creator must transfer/i.test(message)) return 'org.settings.creatorMustTransfer'
  if (/at least one admin/i.test(message)) return 'org.members.lastAdmin'
  if (/transfer target/i.test(message)) return 'org.settings.transferInvalid'
  if (/cannot be dissolved/i.test(message)) return 'org.settings.dissolveBlocked'
  if (/missing permission/i.test(message)) return 'org.settings.forbidden'
  return 'org.error.generic'
}

export function roleErrorKey(err: unknown): string {
  const message = err instanceof Error ? err.message : ''
  if (/role policy changed/i.test(message)) return 'org.roles.changed'
  if (/role name already exists/i.test(message)) return 'org.roles.duplicate'
  if (/role name is invalid|role name is required/i.test(message)) return 'org.roles.nameInvalid'
  if (/role description is too long/i.test(message)) return 'org.roles.descriptionTooLong'
  if (/built-in roles cannot/i.test(message)) return 'org.roles.builtInLocked'
  if (/reassign members in this role/i.test(message)) return 'org.roles.hasMembers'
  return 'org.error.generic'
}
