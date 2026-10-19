import { memberRequest, MemberOrgError } from '@renderer/data/memberOrgs'

type Method = 'GET' | 'POST' | 'PUT'

async function repoRequest<T>(path: string, init: { method: Method; body?: string } = { method: 'GET' }): Promise<T> {
  return (await memberRequest(path, init, fetch)) as T
}

const seg = encodeURIComponent

function query(params: Record<string, string | number | undefined | null>): string {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== '') search.set(key, String(value))
  }
  const text = search.toString()
  return text ? `?${text}` : ''
}

const repoPath = (namespace: string, name: string, tail = '') =>
  `/api/repositories/${seg(namespace)}/${seg(name)}${tail ? `/${tail}` : ''}`

// ── 命名空间与权限 ───────────────────────────────────────────────

export type RepositoryPermissions = {
  read: boolean
  write: boolean
  review: boolean
  release: boolean
  admin: boolean
}

export type TeamNamespace = { namespace: string | null; permissions: RepositoryPermissions }

export function getTeamNamespace(orgId: string) {
  return repoRequest<TeamNamespace>(`/api/repository-namespaces/${seg(orgId)}`)
}

// ── 仓库 ─────────────────────────────────────────────────────────

export const VISIBILITIES = ['team', 'private'] as const
export type Visibility = (typeof VISIBILITIES)[number]

export type Repository = {
  namespace: string
  name: string
  clone_url_path: string
  storage_key: string
  visibility: Visibility
  default_branch: string
  archived: boolean
  created_at: string
}

export type RepositoryDetail = Repository & {
  head: string
  refs: { name: string; oid: string }[]
  permissions: RepositoryPermissions
}

export function listRepositories(namespace: string) {
  return repoRequest<Repository[]>(`/api/repositories${query({ namespace })}`)
}

export function getRepository(namespace: string, name: string) {
  return repoRequest<RepositoryDetail>(repoPath(namespace, name))
}

/** 与服务端 `identity_part_ok` 同形：小写字母开头，小写字母、数字和连字符，不以连字符结尾。 */
export function isRepositoryName(value: string) {
  return value.length <= 64 && /^[a-z](?:[a-z0-9-]*[a-z0-9])?$/.test(value)
}

export function createRepository(input: { namespace: string; name: string; visibility: Visibility }) {
  return repoRequest<Repository>('/api/repositories', {
    method: 'POST',
    body: JSON.stringify({ ...input, seed_readme: true })
  })
}

export type Protection = {
  pattern: string
  no_force_push: boolean
  no_delete: boolean
  require_change_request: boolean
  require_checks: string[]
  require_approvals: number
}

export function listProtections(namespace: string, name: string) {
  return repoRequest<Protection[]>(repoPath(namespace, name, 'protections'))
}

export const MAX_APPROVALS = 10
const MAX_CHECKS = 32
const CHECK_NAME = /^[A-Za-z0-9._/-]{1,64}$/

/** 拆出必需检查：逗号或换行分隔，去空。 */
export function parseCheckNames(value: string) {
  return value
    .split(/[,\n]/)
    .map((name) => name.trim())
    .filter(Boolean)
}

/** 与服务端同一组上限；返回第一条不合格规则的文案键，全部合格返回 null。 */
export function protectionProblem(protections: Protection[]): string | null {
  const seen = new Set<string>()
  for (const rule of protections) {
    const pattern = rule.pattern.trim()
    const subtree = pattern.endsWith('/*') ? pattern.slice(0, -2) : pattern
    if (!subtree.startsWith('refs/') || subtree.length <= 'refs/'.length || /[\s~^:?*[\\]|\.\.|\/\//.test(subtree)) {
      return 'org.repos.protect.badPattern'
    }
    if (seen.has(pattern)) return 'org.repos.protect.duplicate'
    seen.add(pattern)
    if (
      !Number.isInteger(rule.require_approvals) ||
      rule.require_approvals < 0 ||
      rule.require_approvals > MAX_APPROVALS
    ) {
      return 'org.repos.protect.badApprovals'
    }
    if (rule.require_checks.length > MAX_CHECKS || rule.require_checks.some((name) => !CHECK_NAME.test(name))) {
      return 'org.repos.protect.badChecks'
    }
  }
  return null
}

export function setProtections(namespace: string, name: string, protections: Protection[]) {
  return repoRequest<Protection[]>(repoPath(namespace, name, 'protections'), {
    method: 'PUT',
    body: JSON.stringify({ protections })
  })
}

// ── 浏览 ─────────────────────────────────────────────────────────

export type Branch = { name: string; ref: string; oid: string; default: boolean }
export type Tag = { name: string; ref: string; oid: string; commit: string | null }
export type TreeEntry = {
  name: string
  path: string
  type: 'tree' | 'blob' | 'symlink' | 'submodule'
  mode: string
  oid: string
}
export type Tree = { commit: string; path: string; entries: TreeEntry[] }
export type Blob = {
  commit: string
  path: string
  oid: string
  size: number
  symlink: boolean
  content: string | null
  omitted: 'binary' | 'too_large' | null
}
export type Signature = { name: string; email: string; at: string; offset_minutes: number }
export type Commit = {
  oid: string
  tree: string
  parents: string[]
  author: Signature
  committer: Signature
  subject: string
  message: string
}
export type CommitPage = { commits: Commit[]; next: string | null }
export type DiffLine = {
  kind: 'context' | 'added' | 'removed'
  old_line: number | null
  new_line: number | null
  text: string
}
export type Hunk = { old_start: number; old_lines: number; new_start: number; new_lines: number; lines: DiffLine[] }
export type FileDiff = {
  path: string
  status: 'added' | 'deleted' | 'modified'
  old_mode: string | null
  old_oid: string | null
  new_mode: string | null
  new_oid: string | null
  diff: 'text' | 'binary' | 'too_large' | 'submodule'
  hunks: Hunk[]
}
export type Changes = { files: FileDiff[]; truncated: boolean }
export type CommitDetail = { commit: Commit; changes: Changes }
export type Comparison = {
  base: string
  head: string
  merge_base: string | null
  commits: Commit[]
  commits_truncated: boolean
  changes: Changes
}

export function listBranches(namespace: string, name: string) {
  return repoRequest<Branch[]>(repoPath(namespace, name, 'branches'))
}

export function listTags(namespace: string, name: string) {
  return repoRequest<Tag[]>(repoPath(namespace, name, 'tags'))
}

export function getTree(namespace: string, name: string, ref: string, path: string) {
  return repoRequest<Tree>(`${repoPath(namespace, name, 'tree')}${query({ ref, path })}`)
}

export function getBlob(namespace: string, name: string, ref: string, path: string) {
  return repoRequest<Blob>(`${repoPath(namespace, name, 'blob')}${query({ ref, path })}`)
}

export const COMMIT_PAGE = 20

export function listCommits(namespace: string, name: string, ref: string, after?: string | null) {
  return repoRequest<CommitPage>(
    `${repoPath(namespace, name, 'commits')}${query({ ref, after: after ?? undefined, limit: COMMIT_PAGE })}`
  )
}

export function getCommit(namespace: string, name: string, oid: string) {
  return repoRequest<CommitDetail>(repoPath(namespace, name, `commits/${seg(oid)}`))
}

export function compareRefs(namespace: string, name: string, base: string, head: string) {
  return repoRequest<Comparison>(`${repoPath(namespace, name, 'compare')}${query({ base, head })}`)
}

// ── 合并请求 ─────────────────────────────────────────────────────

export const CHANGE_REQUEST_STATES = ['open', 'merged', 'closed'] as const
export type ChangeRequestState = (typeof CHANGE_REQUEST_STATES)[number]
export type ReviewVerdict = 'approved' | 'changes_requested'

export type ChangeRequest = {
  number: number
  title: string
  body: string
  source: string
  source_ref: string
  target: string
  target_ref: string
  author_member: string
  state: ChangeRequestState
  merge: { strategy: string; merged_oid: string; merged_by_member: string } | null
  created_at: string
  updated_at: string
}

export type Check = { name: string; state: string; run_id: string; job_id: string; updated_at: string }
export type Review = {
  id: number
  reviewer_member: string
  verdict: ReviewVerdict
  reviewed_oid: string
  created_at: string
}
export type Comment = {
  id: number
  author_member: string
  path: string | null
  line: number | null
  body: string
  created_at: string
}
export type MergeGate = {
  required_approvals: number
  approvals: string[]
  required_checks: string[]
  passed_checks: string[]
  refusal: string | null
}
export type ChangeRequestDetail = ChangeRequest & {
  source_oid: string | null
  target_oid: string | null
  gate: MergeGate
  checks: Check[]
  reviews: Review[]
  comments: Comment[]
}

export function listChangeRequests(namespace: string, name: string, state?: ChangeRequestState) {
  return repoRequest<ChangeRequest[]>(`${repoPath(namespace, name, 'change-requests')}${query({ state })}`)
}

export function getChangeRequest(namespace: string, name: string, number: number) {
  return repoRequest<ChangeRequestDetail>(repoPath(namespace, name, `change-requests/${number}`))
}

export function openChangeRequest(
  namespace: string,
  name: string,
  input: { title: string; body: string; source: string; target: string }
) {
  return repoRequest<ChangeRequest>(repoPath(namespace, name, 'change-requests'), {
    method: 'POST',
    body: JSON.stringify(input)
  })
}

export function commentOnChangeRequest(namespace: string, name: string, number: number, body: string) {
  return repoRequest<Comment>(repoPath(namespace, name, `change-requests/${number}/comments`), {
    method: 'POST',
    body: JSON.stringify({ body })
  })
}

export function reviewChangeRequest(
  namespace: string,
  name: string,
  number: number,
  verdict: ReviewVerdict,
  body: string
) {
  return repoRequest<Review>(repoPath(namespace, name, `change-requests/${number}/reviews`), {
    method: 'POST',
    body: JSON.stringify(body.trim() ? { verdict, body } : { verdict })
  })
}

export type ChangeRequestAction = 'merge' | 'close' | 'reopen'

export function actOnChangeRequest(namespace: string, name: string, number: number, action: ChangeRequestAction) {
  return repoRequest<ChangeRequest>(repoPath(namespace, name, `change-requests/${number}/${action}`), {
    method: 'POST',
    body: action === 'merge' ? JSON.stringify({ strategy: 'merge_commit' }) : undefined
  })
}

// ── 服务端答复 → 文案键 ──────────────────────────────────────────

/** 拒绝和不存在同一答复：仓库、合并请求都显示「找不到或无权查看」。 */
export function isNotFound(err: unknown) {
  return err instanceof MemberOrgError && err.status === 404
}

const REPO_ERRORS: Record<string, string> = {
  'repository already exists': 'org.repos.exists',
  'repository namespace not found': 'org.repos.notFound',
  'change request is not open': 'org.repos.cr.changed',
  'change request is not closed': 'org.repos.cr.changed',
  'the source branch no longer exists': 'org.repos.cr.sourceGone',
  'the target branch no longer exists': 'org.repos.cr.targetGone',
  'required checks have not passed': 'org.repos.cr.checksPending',
  'the branches share no history': 'org.repos.cr.noHistory'
}

export function repoErrorKey(err: unknown): string {
  if (isNotFound(err)) return 'org.repos.notFound'
  const message = err instanceof Error ? err.message.trim() : ''
  const known = REPO_ERRORS[message]
  if (known) return known
  if (err instanceof MemberOrgError && err.status === 409) return 'org.repos.cr.changed'
  return 'org.error.generic'
}

/** 合并条件的拒绝原因原样展示时，已知的固定答复换成文案；其余（含审阅数）用服务端原文。 */
export function refusalKey(refusal: string): string | null {
  return REPO_ERRORS[refusal.trim()] ?? null
}

export type RefusalMessage = { key: string; values?: Record<string, number> } | { raw: string }

/** 拒绝原因的展示：固定答复用文案；审阅数不足由合并条件的结构化字段给出；其余用服务端原文。 */
export function refusalMessage(gate: MergeGate): RefusalMessage | null {
  if (gate.refusal === null) return null
  const key = refusalKey(gate.refusal)
  if (key) return { key }
  if (gate.approvals.length < gate.required_approvals) {
    return {
      key: 'org.repos.cr.needsApprovals',
      values: { required: gate.required_approvals, count: gate.approvals.length }
    }
  }
  return { raw: gate.refusal }
}

// ── 视图状态（刷新可恢复）────────────────────────────────────────

export const REPO_TABS = ['code', 'commits', 'refs', 'requests'] as const
export type RepoTab = (typeof REPO_TABS)[number]

export type RepoView =
  | { kind: 'list' }
  | { kind: 'repo'; name: string; tab: RepoTab; ref: string | null; path: string; blob: boolean; commit: string | null }
  | { kind: 'request'; name: string; number: number }

export const LIST_VIEW: RepoView = { kind: 'list' }

const VIEW_KEY = 'org.repos.view.v1'

export function readRepoView(orgId: string | null): RepoView {
  if (!orgId) return LIST_VIEW
  try {
    const parsed = JSON.parse(localStorage.getItem(`${VIEW_KEY}:${orgId}`) ?? 'null') as unknown
    return parseRepoView(parsed)
  } catch {
    return LIST_VIEW
  }
}

export function writeRepoView(orgId: string | null, view: RepoView) {
  if (!orgId) return
  try {
    localStorage.setItem(`${VIEW_KEY}:${orgId}`, JSON.stringify(view))
  } catch {
    // Private browsing can reject storage. The current view still stays on screen.
  }
}

export function parseRepoView(value: unknown): RepoView {
  if (!value || typeof value !== 'object') return LIST_VIEW
  const row = value as Record<string, unknown>
  const name = typeof row.name === 'string' && isRepositoryName(row.name) ? row.name : null
  if (row.kind === 'request' && name && Number.isInteger(row.number) && (row.number as number) > 0) {
    return { kind: 'request', name, number: row.number as number }
  }
  if (row.kind === 'repo' && name) {
    const tab = (REPO_TABS as readonly unknown[]).includes(row.tab) ? (row.tab as RepoTab) : 'code'
    return {
      kind: 'repo',
      name,
      tab,
      ref: typeof row.ref === 'string' && row.ref ? row.ref : null,
      path: typeof row.path === 'string' ? row.path.replace(/^\/+|\/+$/g, '') : '',
      blob: row.blob === true,
      commit: typeof row.commit === 'string' && /^[0-9a-f]{40}$/.test(row.commit) ? row.commit : null
    }
  }
  return LIST_VIEW
}

export const shortOid = (oid: string) => oid.slice(0, 7)

/** HTTPS 克隆地址：主机取当前部署的公开地址，路径来自服务端答复。 */
export function cloneUrl(origin: string, repository: Pick<Repository, 'clone_url_path'>) {
  return `${origin.replace(/\/+$/, '')}${repository.clone_url_path}`
}
