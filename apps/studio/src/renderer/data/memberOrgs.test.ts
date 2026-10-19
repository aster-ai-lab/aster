import { waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  createMemberOrg,
  isSessionDenied,
  joinCodeFromInput,
  joinMemberOrg,
  requestJoinMemberOrg,
  searchMemberOrgs,
  listMemberOrgs,
  bindCenterMember,
  canSeePlatformAdmin,
  canManagePlatformDirectory,
  canViewOrgPositions,
  canViewOrgRoles,
  logoutMemberAccount,
  memberAccountFromSession,
  MemberOrgError,
  orgAllows,
  readCenterId,
  readMemberAccount,
  reconcileCenter,
  releaseCenterMember,
  updateMemberAccount,
  writeCenterId
} from './memberOrgs'

const runtime = {
  apiBaseUrl: 'http://127.0.0.1:9',
  token: 'ab'.repeat(32)
}

function installRuntime() {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: runtime,
    configurable: true
  })
}

afterEach(() => {
  releaseCenterMember()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  globalThis.localStorage.removeItem('a3s.org.center')
  globalThis.localStorage.removeItem('a3s.org.center.by-member')
  globalThis.localStorage.removeItem('kense.org.center')
  globalThis.localStorage.removeItem('kense.org.center.by-member')
  globalThis.document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
  globalThis.document.cookie = 'kense.org.center=; Path=/; Max-Age=0'
  vi.unstubAllGlobals()
})

describe('memberOrgs', () => {
  it('publishes the selected team to the shell', async () => {
    installRuntime()
    const fetchImpl = vi.fn<
      (url: Parameters<typeof globalThis.fetch>[0], init?: globalThis.RequestInit) => Promise<globalThis.Response>
    >(async () => new globalThis.Response('{"ok":true}', { status: 200 }))
    vi.stubGlobal('fetch', fetchImpl)
    writeCenterId('org-1')
    await waitFor(() => expect(fetchImpl).toHaveBeenCalledTimes(1))
    expect(fetchImpl).toHaveBeenCalledWith(
      'http://127.0.0.1:9/api/member-center',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ id: 'org-1' })
      })
    )
    expect(fetchImpl.mock.calls[0][1]?.headers).toMatchObject({ authorization: `Bearer ${runtime.token}` })
  })

  it('moves a saved team off the previous storage key', () => {
    globalThis.localStorage.setItem('kense.org.center', 'org-legacy')
    globalThis.document.cookie = 'kense.org.center=org-legacy; Path=/'
    expect(readCenterId()).toBe('org-legacy')
    expect(globalThis.localStorage.getItem('a3s.org.center')).toBe('org-legacy')
    expect(globalThis.localStorage.getItem('kense.org.center')).toBeNull()
    expect(globalThis.document.cookie).not.toContain('kense.org.center=')
  })

  it('keeps each member on their own team', () => {
    const memberA = '11111111-1111-4111-8111-111111111111'
    const memberB = '22222222-2222-4222-8222-222222222222'
    bindCenterMember(memberA)
    writeCenterId('org-a')
    expect(readCenterId()).toBe('org-a')
    releaseCenterMember()
    globalThis.localStorage.setItem('a3s.org.center', 'personal')
    bindCenterMember(memberB)
    expect(readCenterId()).toBe('personal')
    writeCenterId('org-b')
    bindCenterMember(memberA)
    expect(readCenterId()).toBe('org-a')
    expect(memberAccountFromSession({ id: memberA, display_name: '甲', phone: '13800138000' }).id).toBe(memberA)
  })

  it('checks team permissions the way the OS does', () => {
    const base = { id: 'org-1', name: 'A', logo: '' }
    expect(orgAllows({ ...base, membership: 'member', creator: true }, 'delete', 'Organization')).toBe(true)
    expect(
      orgAllows(
        { ...base, membership: 'admin', rules: [{ action: 'manage', subject: 'all' }] },
        'update',
        'Organization'
      )
    ).toBe(true)
    expect(
      orgAllows(
        { ...base, membership: 'admin', rules: [{ action: 'update', subject: 'Organization' }] },
        'delete',
        'Organization'
      )
    ).toBe(false)
    expect(orgAllows({ ...base, membership: 'member', rules: [] }, 'update', 'Organization')).toBe(false)
    expect(orgAllows({ ...base, membership: 'pending', creator: true }, 'update', 'Organization')).toBe(false)
    expect(orgAllows(undefined, 'update', 'Organization')).toBe(false)
  })

  it('opens the position and role pages only when the role grants that catalog', () => {
    const base = { id: 'org-1', name: 'A', logo: '', membership: 'member' as const }
    const reads = [
      { action: 'read', subject: 'Position' },
      { action: 'read', subject: 'Role' }
    ]
    expect(canViewOrgPositions({ ...base, rules: reads })).toBe(false)
    expect(canViewOrgRoles({ ...base, rules: reads })).toBe(false)
    expect(canViewOrgPositions({ ...base, rules: [{ action: 'create', subject: 'Position' }] })).toBe(true)
    expect(canViewOrgRoles({ ...base, rules: [{ action: 'create', subject: 'Position' }] })).toBe(false)
    expect(canViewOrgPositions({ ...base, rules: [{ action: 'assign', subject: 'Role' }] })).toBe(false)
    expect(canViewOrgRoles({ ...base, rules: [{ action: 'assign', subject: 'Role' }] })).toBe(true)
    expect(canViewOrgPositions({ ...base, creator: true, rules: [] })).toBe(true)
    expect(canViewOrgRoles({ ...base, creator: true, rules: [] })).toBe(true)
    expect(canViewOrgPositions({ ...base, membership: 'pending', creator: true })).toBe(false)
    expect(canViewOrgRoles(undefined)).toBe(false)
  })

  it('shows platform administration only to directory admins of the official team', async () => {
    installRuntime()
    const fetchImpl = vi.fn(
      async () =>
        new globalThis.Response(
          JSON.stringify([
            { id: 'org-official', name: '官方组织', membership: 'admin', creator: true, is_official: true },
            { id: 'org-channel', name: '渠道团队', membership: 'admin', creator: true, is_official: false }
          ]),
          { status: 200 }
        )
    )

    const [official, channel] = await listMemberOrgs(fetchImpl)
    expect(official?.isOfficial).toBe(true)
    expect(canSeePlatformAdmin(official)).toBe(true)
    expect(canSeePlatformAdmin(channel)).toBe(false)
    expect(canSeePlatformAdmin({ ...official, creator: false, rules: [] })).toBe(false)
    expect(canSeePlatformAdmin({ ...official, membership: 'pending' })).toBe(false)
    expect(canSeePlatformAdmin(undefined)).toBe(false)
  })

  it('shows only resource entry for supplier delegates and refuses bounded or non-official write grants', () => {
    const base = {
      id: 'official',
      name: 'Official',
      logo: '',
      isOfficial: true,
      creator: false,
      membership: 'member' as const,
      rules: [{ action: 'read', subject: 'PlatformSupplier' }],
      dataScope: { kind: 'self', departmentIds: [] }
    }
    expect(canSeePlatformAdmin(base)).toBe(true)
    expect(canManagePlatformDirectory(base)).toBe(false)
    const writeOnly = { ...base, rules: [{ action: 'update', subject: 'PlatformSupplier' }] }
    expect(canSeePlatformAdmin(writeOnly)).toBe(false)
    expect(canSeePlatformAdmin({ ...writeOnly, dataScope: { kind: 'all', departmentIds: [] } })).toBe(true)
    expect(canSeePlatformAdmin({ ...base, isOfficial: false })).toBe(false)
    expect(canSeePlatformAdmin({ ...base, membership: 'pending' })).toBe(false)
  })

  it('asks the shell for teams and unwraps the envelope', async () => {
    installRuntime()
    const fetchImpl = vi.fn(
      async () =>
        new globalThis.Response(
          JSON.stringify({
            code: 0,
            message: 'ok',
            data: [
              { id: 'org-1', name: '通友科技', logo: '', membership: 'admin', creator: true },
              { id: '', name: 'skip' }
            ]
          }),
          { status: 200 }
        )
    )

    await expect(listMemberOrgs(fetchImpl)).resolves.toEqual([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        isOfficial: false,
        canReadWork: false,
        canCreateWork: false,
        canReviewJoin: false,
        canInviteMembers: false,
        canUpdateOrg: false,
        canTransferOrg: false,
        canDeleteOrg: false,
        canLeaveOrg: false,
        canReadSchedule: false,
        canCreateSchedule: false,
        canUpdateSchedule: false,
        canDeleteSchedule: false,
        canReadMailRecipients: false,
        canReadMail: false,
        canCreateMail: false,
        canSendMail: false,
        canUpdateMail: false,
        canDeleteMail: false,
        dataScope: undefined,
        industry: '',
        scale: '',
        memberCount: 0,
        rules: []
      }
    ])
    expect(fetchImpl).toHaveBeenCalledWith('http://127.0.0.1:9/api/member-orgs', {
      method: 'GET',
      cache: 'no-store',
      headers: {
        accept: 'application/json',
        authorization: `Bearer ${runtime.token}`
      },
      body: undefined
    })
  })

  it('creates a team with the shell bearer', async () => {
    installRuntime()
    const fetchImpl = vi.fn<
      (url: Parameters<typeof globalThis.fetch>[0], init?: globalThis.RequestInit) => Promise<globalThis.Response>
    >(
      async () =>
        new globalThis.Response(
          JSON.stringify({ code: 0, message: 'ok', data: { id: 'org-2', name: '新团队', creator: true } }),
          {
            status: 201
          }
        )
    )

    await expect(createMemberOrg('新团队', '', fetchImpl)).resolves.toMatchObject({ id: 'org-2', name: '新团队' })
    const init = fetchImpl.mock.calls[0]?.[1] as globalThis.RequestInit
    expect(init.method).toBe('POST')
    expect(init.body).toBe(JSON.stringify({ name: '新团队' }))
    expect((init.headers as Record<string, string>).authorization).toBe(`Bearer ${runtime.token}`)

    await createMemberOrg('新团队', 'data:image/webp;base64,AAAA', fetchImpl)
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(
      JSON.stringify({ name: '新团队', logo: 'data:image/webp;base64,AAAA' })
    )
  })

  it('reads a join code from a raw value or an invite link', () => {
    expect(joinCodeFromInput('  abc123  ')).toBe('abc123')
    expect(joinCodeFromInput('https://example.test/join/abc123')).toBe('abc123')
    expect(joinCodeFromInput('https://example.test/join/abc123/')).toBe('abc123')
    expect(joinCodeFromInput('/join/abc123?from=copy')).toBe('abc123')
    expect(joinCodeFromInput('')).toBe('')
    expect(joinCodeFromInput('not a code')).toBe('')
    expect(joinCodeFromInput('https://example.test/invite/abc123')).toBe('')
  })

  it('joins a team through the shell with the code from the invite link', async () => {
    installRuntime()
    const fetchImpl = vi.fn<
      (url: Parameters<typeof globalThis.fetch>[0], init?: globalThis.RequestInit) => Promise<globalThis.Response>
    >(
      async () =>
        new globalThis.Response(
          JSON.stringify({
            code: 0,
            message: 'ok',
            data: {
              code: 'abc123',
              enabled: true,
              expired: false,
              org: { id: 'org-2', name: '协作组', logo: '', membership: 'member' }
            }
          }),
          { status: 200 }
        )
    )

    await expect(joinMemberOrg('abc123', fetchImpl)).resolves.toMatchObject({ id: 'org-2', name: '协作组' })
    expect(String(fetchImpl.mock.calls[0]?.[0])).toBe('http://127.0.0.1:9/api/member/join-codes/abc123/memberships')
    const init = fetchImpl.mock.calls[0]?.[1] as globalThis.RequestInit
    expect(init.method).toBe('POST')
    expect(init.body).toBe('{}')
    expect((init.headers as Record<string, string>).authorization).toBe(`Bearer ${runtime.token}`)
  })

  it('searches teams by name and posts a join request', async () => {
    installRuntime()
    const fetchImpl = vi.fn<
      (url: Parameters<typeof globalThis.fetch>[0], init?: globalThis.RequestInit) => Promise<globalThis.Response>
    >(async (url) => {
      const path = String(url)
      if (path.includes('discoverable-orgs')) {
        return new globalThis.Response(
          JSON.stringify({
            code: 0,
            message: 'ok',
            data: [
              {
                id: 'org-2',
                name: '通友科技',
                logo: '',
                member_count: 12,
                joined: false,
                requested: false
              },
              { id: '', name: '丢弃' }
            ]
          }),
          { status: 200 }
        )
      }
      return new globalThis.Response(JSON.stringify({ code: 0, message: 'ok', data: { ok: true } }), { status: 201 })
    })

    await expect(searchMemberOrgs(' 通友 ', fetchImpl)).resolves.toEqual([
      { id: 'org-2', name: '通友科技', logo: '', memberCount: 12, joined: false, requested: false }
    ])
    expect(String(fetchImpl.mock.calls[0]?.[0])).toBe(
      'http://127.0.0.1:9/api/member/discoverable-orgs?q=%E9%80%9A%E5%8F%8B'
    )

    await requestJoinMemberOrg('org-2', fetchImpl)
    expect(String(fetchImpl.mock.calls[1]?.[0])).toBe('http://127.0.0.1:9/api/member/orgs/org-2/join-requests')
    const init = fetchImpl.mock.calls[1]?.[1] as globalThis.RequestInit
    expect(init.method).toBe('POST')
    expect(init.body).toBe('{}')
  })

  it('reads and saves the OS profile, and only falls back to the saved session when the OS is unreachable', async () => {
    installRuntime()
    const me = {
      code: 0,
      message: 'ok',
      data: { display_name: '林', phone: '13800138000', avatar: 'data:image/webp;base64,AA' }
    }
    const fetchImpl = vi.fn<
      (url: Parameters<typeof globalThis.fetch>[0], init?: globalThis.RequestInit) => Promise<globalThis.Response>
    >(async () => new globalThis.Response(JSON.stringify(me), { status: 200 }))
    await expect(readMemberAccount(fetchImpl)).resolves.toEqual({
      displayName: '林',
      phone: '13800138000',
      avatar: 'data:image/webp;base64,AA'
    })
    expect(String(fetchImpl.mock.calls[0]?.[0])).toBe('http://127.0.0.1:9/api/member/me')

    await updateMemberAccount({ displayName: ' 新名字 ', avatar: '' }, fetchImpl)
    const init = fetchImpl.mock.calls[1]?.[1] as globalThis.RequestInit
    expect(String(fetchImpl.mock.calls[1]?.[0])).toBe('http://127.0.0.1:9/api/member/me')
    expect(init.method).toBe('PUT')
    expect(init.body).toBe(JSON.stringify({ display_name: '新名字', avatar: '' }))

    const denied = vi.fn(
      async () => new globalThis.Response(JSON.stringify({ error: 'unauthorized' }), { status: 401 })
    )
    const error = await readMemberAccount(denied).catch((reason: unknown) => reason)
    expect(isSessionDenied(error)).toBe(true)
    expect(denied).toHaveBeenCalledTimes(1)
  })

  it('reads the account name and phone without keeping the session token', async () => {
    installRuntime()
    const fetchImpl = vi.fn()
    fetchImpl.mockResolvedValueOnce(new globalThis.Response('upstream down', { status: 502 }))
    fetchImpl.mockImplementation(
      async () =>
        new globalThis.Response(
          JSON.stringify({
            signedIn: true,
            display_name: ' RoyLin ',
            phone: '13800138000'
          }),
          { status: 200 }
        )
    )

    await expect(readMemberAccount(fetchImpl)).resolves.toEqual({ displayName: 'RoyLin', phone: '13800138000' })
    expect(memberAccountFromSession({ token: 'a3s_ms_session.token', phone: 'abc' })).toEqual({
      displayName: '',
      phone: ''
    })
    expect(
      memberAccountFromSession({
        display_name: 'RoyLin',
        phone: '18770221825',
        avatar: 'data:image/svg+xml;base64,QQ'
      })
    ).toEqual({
      displayName: 'RoyLin',
      phone: '18770221825',
      avatar: 'data:image/svg+xml;base64,QQ'
    })
    expect(fetchImpl).toHaveBeenCalledWith(
      'http://127.0.0.1:9/api/member-session/profile',
      expect.objectContaining({ method: 'GET' })
    )

    fetchImpl.mockResolvedValueOnce(new globalThis.Response(JSON.stringify({ token: null }), { status: 200 }))
    await expect(logoutMemberAccount(fetchImpl)).resolves.toBeUndefined()
    const init = fetchImpl.mock.calls[2]?.[1] as globalThis.RequestInit
    expect(String(fetchImpl.mock.calls[2]?.[0])).toBe('http://127.0.0.1:9/api/member-session/logout')
    expect(init.method).toBe('POST')
    expect(JSON.stringify(init)).not.toContain('a3s_ms_')
  })

  it.each([401, 404, 409, 503])(
    'preserves Boot error messages and the actual HTTP %s authority status',
    async (status) => {
      installRuntime()
      const message = 'role policy changed; reload before saving'
      const fetchImpl = vi.fn(
        async () => new globalThis.Response(JSON.stringify({ statusCode: 401, message, error: 'Conflict' }), { status })
      )
      const error = await listMemberOrgs(fetchImpl).catch((reason: unknown) => reason)
      expect(error).toBeInstanceOf(MemberOrgError)
      expect(error).toMatchObject({ message, status })
      expect(isSessionDenied(error)).toBe(status === 401)
    }
  )

  it.each([401, 404, 503])('refuses a success-shaped policy body on HTTP %s', async (status) => {
    installRuntime()
    const fetchImpl = vi.fn(
      async () =>
        new globalThis.Response(
          JSON.stringify({
            code: 0,
            data: [{ id: 'org-1', name: 'team', membership: 'admin', rules: [{ action: 'update', subject: 'Role' }] }]
          }),
          { status }
        )
    )
    const error = await listMemberOrgs(fetchImpl).catch((reason: unknown) => reason)
    expect(error).toBeInstanceOf(MemberOrgError)
    expect(error).toMatchObject({ status })
    expect(isSessionDenied(error)).toBe(status === 401)
  })

  it('fails closed when the shell descriptor or the membership is missing', async () => {
    const fetchImpl = vi.fn()
    await expect(listMemberOrgs(fetchImpl)).rejects.toBeInstanceOf(MemberOrgError)
    expect(fetchImpl).not.toHaveBeenCalled()

    installRuntime()
    const denied = vi.fn(
      async () => new globalThis.Response(JSON.stringify({ error: 'unauthorized' }), { status: 401 })
    )
    await expect(listMemberOrgs(denied)).rejects.toThrow('HTTP 401')
    expect(reconcileCenter('missing', [])).toBe('personal')
    expect(reconcileCenter('org-1', [{ id: 'org-1', name: '通友科技', logo: '', membership: 'pending' }])).toBe(
      'personal'
    )
  })
})
