import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MemberOrgError } from '@renderer/data/memberOrgs'

import type { OrgMemberRow } from '../orgApi'
import { OrgChangeRequestDetail } from '../OrgChangeRequestDetail'
import { OrgRepositoriesPanel } from '../OrgRepositoriesPanel'
import type * as RepoApi from '../repositoryApi'
import {
  cloneUrl,
  isRepositoryName,
  parseCheckNames,
  parseRepoView,
  protectionProblem,
  refusalKey,
  refusalMessage,
  repoErrorKey,
  type ChangeRequestDetail,
  type Protection,
  type RepositoryDetail,
  type RepositoryPermissions
} from '../repositoryApi'
import { memberLabel, relativeTime } from '../repositoryUi'

const api = vi.hoisted(() => ({
  getTeamNamespace: vi.fn(),
  listRepositories: vi.fn(),
  getRepository: vi.fn(),
  createRepository: vi.fn(),
  listBranches: vi.fn(),
  listTags: vi.fn(),
  getTree: vi.fn(),
  getBlob: vi.fn(),
  listCommits: vi.fn(),
  getChangeRequest: vi.fn(),
  compareRefs: vi.fn(),
  actOnChangeRequest: vi.fn(),
  reviewChangeRequest: vi.fn(),
  commentOnChangeRequest: vi.fn()
}))
const profile = vi.hoisted(() => ({ current: { displayName: 'me', phone: '', id: 'm-reviewer' } }))
const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars && Object.keys(vars).length ? `${key} ${JSON.stringify(vars)}` : key,
    i18n: { language: 'zh-CN' }
  })
}))
vi.mock('@renderer/services/toast', () => ({ toast }))
vi.mock('@renderer/components/account/memberProfile', () => ({ useMemberProfile: () => profile.current }))
vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('../repositoryApi', async (importOriginal) => ({
  ...(await importOriginal<typeof RepoApi>()),
  ...api
}))

const ALL: RepositoryPermissions = { read: true, write: true, review: true, release: true, admin: true }
const READ: RepositoryPermissions = { read: true, write: false, review: false, release: false, admin: false }

function repo(patch: Partial<RepositoryDetail> = {}): RepositoryDetail {
  return {
    namespace: 'acme',
    name: 'web',
    clone_url_path: '/git/acme/web.git',
    storage_key: 'k',
    visibility: 'team',
    default_branch: 'main',
    archived: false,
    created_at: '2026-10-13T00:00:00Z',
    head: 'refs/heads/main',
    refs: [],
    permissions: ALL,
    ...patch
  }
}

function request(patch: Partial<ChangeRequestDetail> = {}): ChangeRequestDetail {
  return {
    number: 7,
    title: 'Add login',
    body: 'Adds the login page.',
    source: 'feature',
    source_ref: 'refs/heads/feature',
    target: 'main',
    target_ref: 'refs/heads/main',
    author_member: 'm-author',
    state: 'open',
    merge: null,
    created_at: '2026-10-13T00:00:00Z',
    updated_at: '2026-10-13T00:00:00Z',
    source_oid: 'a'.repeat(40),
    target_oid: 'b'.repeat(40),
    gate: { required_approvals: 1, approvals: [], required_checks: [], passed_checks: [], refusal: null },
    checks: [],
    reviews: [],
    comments: [],
    ...patch
  }
}

const members = [{ member_id: 'm-author', name: '张三' }] as OrgMemberRow[]

function renderFresh(node: React.ReactNode) {
  return render(<SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{node}</SWRConfig>)
}

beforeEach(() => {
  for (const fn of Object.values(api)) fn.mockReset()
  toast.success.mockReset()
  toast.error.mockReset()
  localStorage.clear()
  profile.current = { displayName: 'me', phone: '', id: 'm-reviewer' }
})

describe('repository helpers', () => {
  it('accepts only server-legal repository names', () => {
    expect(isRepositoryName('web-app2')).toBe(true)
    expect(isRepositoryName('Web')).toBe(false)
    expect(isRepositoryName('web-')).toBe(false)
    expect(isRepositoryName('2web')).toBe(false)
    expect(isRepositoryName(`a${'b'.repeat(64)}`)).toBe(false)
  })

  it('restores only well-formed views', () => {
    expect(parseRepoView({ kind: 'request', name: 'web', number: 3 })).toEqual({
      kind: 'request',
      name: 'web',
      number: 3
    })
    expect(parseRepoView({ kind: 'request', name: 'web', number: 0 })).toEqual({ kind: 'list' })
    expect(parseRepoView({ kind: 'repo', name: '../x', tab: 'code' })).toEqual({ kind: 'list' })
    expect(parseRepoView({ kind: 'repo', name: 'web', tab: 'bogus', path: '/src/', commit: 'nope' })).toEqual({
      kind: 'repo',
      name: 'web',
      tab: 'code',
      ref: null,
      path: 'src',
      blob: false,
      commit: null
    })
    expect(parseRepoView('garbage')).toEqual({ kind: 'list' })
  })

  it('maps server answers to copy, with denied and missing alike', () => {
    expect(repoErrorKey(new MemberOrgError('repository not found', 404))).toBe('org.repos.notFound')
    expect(repoErrorKey(new MemberOrgError('repository already exists', 409))).toBe('org.repos.exists')
    expect(repoErrorKey(new MemberOrgError('main changed during the merge; retry', 409))).toBe('org.repos.cr.changed')
    expect(repoErrorKey(new Error('boom'))).toBe('org.error.generic')
    expect(refusalKey('required checks have not passed')).toBe('org.repos.cr.checksPending')
    expect(refusalKey('change request needs 1 approval(s) on its latest commit; it has 0')).toBeNull()
    const gate = { required_approvals: 2, approvals: ['m-1'], required_checks: [], passed_checks: [] }
    expect(refusalMessage({ ...gate, refusal: null })).toBeNull()
    expect(refusalMessage({ ...gate, refusal: 'required checks have not passed' })).toEqual({
      key: 'org.repos.cr.checksPending'
    })
    expect(refusalMessage({ ...gate, refusal: 'needs approvals' })).toEqual({
      key: 'org.repos.cr.needsApprovals',
      values: { required: 2, count: 1 }
    })
    expect(refusalMessage({ ...gate, approvals: ['m-1', 'm-2'], refusal: 'something new' })).toEqual({
      raw: 'something new'
    })
  })

  it('checks protections against the server bounds', () => {
    const rule = (patch: Partial<Protection>): Protection => ({
      pattern: 'refs/heads/main',
      no_force_push: true,
      no_delete: true,
      require_change_request: false,
      require_checks: [],
      require_approvals: 0,
      ...patch
    })
    expect(protectionProblem([rule({}), rule({ pattern: 'refs/tags/*' })])).toBeNull()
    expect(protectionProblem([rule({ pattern: 'main' })])).toBe('org.repos.protect.badPattern')
    expect(protectionProblem([rule({ pattern: 'refs/' })])).toBe('org.repos.protect.badPattern')
    expect(protectionProblem([rule({}), rule({})])).toBe('org.repos.protect.duplicate')
    expect(protectionProblem([rule({ require_approvals: 11 })])).toBe('org.repos.protect.badApprovals')
    expect(protectionProblem([rule({ require_checks: ['has space'] })])).toBe('org.repos.protect.badChecks')
    expect(parseCheckNames(' build, test\nlint ,, ')).toEqual(['build', 'test', 'lint'])
  })

  it('builds clone URLs, relative times and member labels', () => {
    expect(cloneUrl('https://os.example.com/', repo())).toBe('https://os.example.com/git/acme/web.git')
    const now = Date.parse('2026-10-13T01:00:00Z')
    expect(relativeTime('2026-10-13T00:50:00Z', 'en', now)).toBe('10 minutes ago')
    expect(relativeTime('2026-10-13T00:59:50Z', 'en', now)).toBe('this minute')
    expect(memberLabel(members, 'm-author')).toBe('张三')
    expect(memberLabel(members, '0123456789abcdef')).toBe('01234567')
  })
})

describe('OrgRepositoriesPanel', () => {
  it('explains a team without a code namespace', async () => {
    api.getTeamNamespace.mockResolvedValue({ namespace: null, permissions: READ })
    renderFresh(<OrgRepositoriesPanel orgId="org-1" members={[]} onOpenTokens={vi.fn()} />)
    expect(await screen.findByText('org.repos.noNamespaceTitle')).toBeInTheDocument()
    expect(api.listRepositories).not.toHaveBeenCalled()
  })

  it('offers create only to repository admins', async () => {
    api.getTeamNamespace.mockResolvedValue({ namespace: 'acme', permissions: READ })
    api.listRepositories.mockResolvedValue([repo()])
    const { unmount } = renderFresh(<OrgRepositoriesPanel orgId="org-1" members={[]} onOpenTokens={vi.fn()} />)
    expect(await screen.findByText('web')).toBeInTheDocument()
    expect(screen.queryByText('org.repos.create')).not.toBeInTheDocument()
    unmount()

    api.getTeamNamespace.mockResolvedValue({ namespace: 'acme', permissions: ALL })
    renderFresh(<OrgRepositoriesPanel orgId="org-1" members={[]} onOpenTokens={vi.fn()} />)
    expect(await screen.findByText('org.repos.create')).toBeInTheDocument()
  })

  it('opens a repository and remembers the view', async () => {
    api.getTeamNamespace.mockResolvedValue({ namespace: 'acme', permissions: READ })
    api.listRepositories.mockResolvedValue([repo()])
    api.getRepository.mockResolvedValue(repo({ permissions: READ }))
    api.listBranches.mockResolvedValue([{ name: 'main', ref: 'refs/heads/main', oid: 'c'.repeat(40), default: true }])
    api.listCommits.mockResolvedValue({ commits: [], next: null })
    api.getTree.mockResolvedValue({ commit: 'c'.repeat(40), path: '', entries: [] })
    renderFresh(<OrgRepositoriesPanel orgId="org-1" members={[]} onOpenTokens={vi.fn()} />)
    fireEvent.click(await screen.findByText('web'))
    expect(await screen.findByRole('tab', { name: 'org.repos.tab.code' })).toHaveAttribute('aria-selected', 'true')
    expect(JSON.parse(localStorage.getItem('org.repos.view.v1:org-1') ?? 'null')).toMatchObject({
      kind: 'repo',
      name: 'web'
    })
  })

  it('answers a missing repository like a denied one', async () => {
    localStorage.setItem('org.repos.view.v1:org-1', JSON.stringify({ kind: 'repo', name: 'gone', tab: 'code' }))
    api.getTeamNamespace.mockResolvedValue({ namespace: 'acme', permissions: READ })
    api.getRepository.mockRejectedValue(new MemberOrgError('repository not found', 404))
    renderFresh(<OrgRepositoriesPanel orgId="org-1" members={[]} onOpenTokens={vi.fn()} />)
    expect(await screen.findByText('org.repos.notFound')).toBeInTheDocument()
  })
})

describe('OrgChangeRequestDetail', () => {
  function show(detail: ChangeRequestDetail, permissions: RepositoryPermissions = ALL) {
    api.getRepository.mockResolvedValue(repo({ permissions }))
    api.getChangeRequest.mockResolvedValue(detail)
    return renderFresh(
      <OrgChangeRequestDetail
        namespace="acme"
        name="web"
        number={detail.number}
        members={members}
        onBack={vi.fn()}
        onList={vi.fn()}
      />
    )
  }

  it('locks merge while the gate refuses and explains why', async () => {
    const refusal = 'change request needs 1 approval(s) on its latest commit; it has 0'
    show(request({ gate: { required_approvals: 1, approvals: [], required_checks: [], passed_checks: [], refusal } }))
    const merge = await screen.findByRole('button', { name: /org.repos.cr.merge/ })
    expect(merge).toBeDisabled()
    expect(screen.getByText('org.repos.cr.needsApprovals {"required":1,"count":0}')).toBeInTheDocument()
    expect(screen.queryByText(refusal)).not.toBeInTheDocument()
  })

  it('merges once the gate opens', async () => {
    api.actOnChangeRequest.mockResolvedValue(request({ state: 'merged' }))
    show(
      request({
        gate: {
          required_approvals: 1,
          approvals: ['m-reviewer'],
          required_checks: [],
          passed_checks: [],
          refusal: null
        }
      })
    )
    const merge = await screen.findByRole('button', { name: /org.repos.cr.merge/ })
    expect(merge).toBeEnabled()
    fireEvent.click(merge)
    await waitFor(() => expect(api.actOnChangeRequest).toHaveBeenCalledWith('acme', 'web', 7, 'merge'))
    expect(toast.success).toHaveBeenCalledWith('org.repos.cr.done.merge')
  })

  it('hides merge from readers and approval from the author', async () => {
    show(request(), READ)
    expect(await screen.findByText('Add login')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /org.repos.cr.merge/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /org.repos.cr.approve/ })).not.toBeInTheDocument()
  })

  it('lets a reviewer approve but not the author', async () => {
    api.reviewChangeRequest.mockResolvedValue({})
    const { unmount } = show(request())
    fireEvent.click(await screen.findByRole('button', { name: /org.repos.cr.approve/ }))
    await waitFor(() => expect(api.reviewChangeRequest).toHaveBeenCalledWith('acme', 'web', 7, 'approved', ''))
    unmount()

    profile.current = { displayName: 'me', phone: '', id: 'm-author' }
    show(request())
    expect(await screen.findByText('org.repos.cr.authorCannotApprove')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /org.repos.cr.approve/ })).not.toBeInTheDocument()
  })
})
