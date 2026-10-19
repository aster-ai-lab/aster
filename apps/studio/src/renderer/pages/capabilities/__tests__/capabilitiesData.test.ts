import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  CAPABILITY_CATEGORIES,
  capabilityCategoryLabelKey,
  capabilitySections,
  installCapability,
  listCapabilities,
  listCustomConnectors,
  prepareBuiltinSkill,
  removeCustomConnector,
  saveCustomConnector,
  searchCapabilities,
  uninstallCapability,
  uploadSkillPackage,
  type CapabilityEntry
} from '../capabilitiesData'

vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:4000', token: 'shell-token' })
}))

function entry(slug: string, patch: Partial<CapabilityEntry> = {}): CapabilityEntry {
  return {
    slug,
    kind: 'connector',
    category: 'other',
    name: slug,
    description: '',
    color: '#171717',
    featured: false,
    builtin: false,
    installed: false,
    config: [],
    surfaces: [],
    ...patch
  }
}

const CATALOG: CapabilityEntry[] = [
  entry('github', { name: 'GitHub', category: 'code', featured: true, description: 'Manage repositories' }),
  entry('notion', { name: 'Notion', category: 'productivity', featured: true }),
  entry('slack', { name: 'Slack', category: 'office' }),
  entry('deep-research', { kind: 'skill', name: 'Deep Research', featured: true }),
  entry('ppt-creation', { kind: 'skill', name: 'PPT Creation', category: 'office' })
]

function respond(body: unknown, status = 200) {
  return vi.fn<typeof fetch>(async () => new Response(JSON.stringify(body), { status }))
}

describe('capabilities OS client', () => {
  beforeEach(() => vi.useRealTimers())
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('reads one page through the shell with the shell bearer', async () => {
    const fetchImpl = respond({
      code: 0,
      message: 'ok',
      data: {
        items: [
          {
            slug: 'notion',
            kind: 'connector',
            category: 'productivity',
            name: 'Notion',
            description: 'Pages and databases',
            color: '#191919',
            featured: true,
            installed: true,
            iconUrl: '/v1/api/org/capabilities/notion/icon?v=abc',
            version: '  ',
            prompts: ['', '  生成一份客户汇报 PPT  ', '生成一份客户汇报 PPT', 3]
          },
          { slug: 'broken', kind: 'plugin', name: 'Legacy plugin' },
          {
            slug: 'odd',
            kind: 'skill',
            category: 'unknown',
            name: 'Odd',
            color: 'red',
            iconUrl: 'https://evil.example/icon.png'
          }
        ],
        total: 40,
        page: 2,
        pageSize: 24,
        categories: [
          { id: 'productivity', count: 12 },
          { id: 'code', count: 3 },
          { id: '  ', count: 9 },
          { id: 'office', count: 0 },
          { id: 'featured', count: 4 },
          { id: 'modelscope-tools', count: 2 },
          'nope'
        ]
      }
    })

    const page = await listCapabilities(
      { kind: 'connector', q: '  notion ', category: 'productivity', featured: true, page: 2, pageSize: 24 },
      fetchImpl
    )

    expect(fetchImpl).toHaveBeenCalledWith(
      'http://127.0.0.1:4000/api/capabilities?kind=connector&q=notion&category=productivity&featured=true&page=2&pageSize=24',
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({ authorization: 'Bearer shell-token' })
      })
    )
    expect(page).toEqual({
      items: [
        entry('notion', {
          name: 'Notion',
          category: 'productivity',
          description: 'Pages and databases',
          color: '#191919',
          featured: true,
          installed: true,
          iconUrl: '/api/capabilities/notion/icon?v=abc',
          prompts: ['生成一份客户汇报 PPT']
        }),
        entry('odd', { kind: 'skill', name: 'Odd' })
      ],
      total: 40,
      page: 2,
      pageSize: 24,
      categories: [
        { id: 'productivity', count: 12 },
        { id: 'code', count: 3 },
        { id: 'modelscope-tools', count: 2 }
      ]
    })
    expect(capabilityCategoryLabelKey('productivity')).toBe('capabilities.category.productivity')
    expect(capabilityCategoryLabelKey('modelscope-tools')).toBeUndefined()

    await listCapabilities({ kind: 'connector', installed: true }, fetchImpl)
    expect(fetchImpl).toHaveBeenLastCalledWith(
      'http://127.0.0.1:4000/api/capabilities?kind=connector&installed=true&page=1&pageSize=24',
      expect.anything()
    )
  })

  it('keeps a plugin package with a digest and a package id', async () => {
    const fetchImpl = respond({
      code: 0,
      message: 'ok',
      data: {
        items: [
          { slug: 'broken', kind: 'plugin', name: 'Legacy plugin' },
          {
            slug: 'half',
            kind: 'plugin',
            name: 'Half',
            install: { type: 'plugin', digest: 'sha256:ab', size: 4, packageId: 'use/a3s/review' }
          },
          {
            slug: 'a3s-review',
            kind: 'plugin',
            category: 'code',
            name: '审查',
            description: '审查 Word 文档',
            color: '#6E5CE6',
            featured: true,
            installed: false,
            iconUrl: '/v1/api/org/capabilities/a3s-review/icon?v=cd6293d906b3',
            coverUrl: '/v1/api/org/capabilities/a3s-review/cover?v=573ad5be561a',
            install: {
              type: 'plugin',
              digest: `sha256:${'ab'.repeat(32)}`,
              size: 120,
              packageId: 'use/a3s/review'
            },
            surfaces: ['skill', 'ui', 'nope']
          }
        ],
        total: 3,
        page: 1,
        pageSize: 24,
        categories: [{ id: 'code', count: 1 }]
      }
    })
    const page = await listCapabilities({ kind: 'plugin' }, fetchImpl)
    expect(page.items.map((item) => item.slug)).toEqual(['a3s-review'])
    expect(page.items[0]?.applet).toBeUndefined()
    expect(page.items[0]).toMatchObject({
      kind: 'plugin',
      name: '审查',
      category: 'code',
      featured: true,
      description: '审查 Word 文档',
      iconUrl: '/api/capabilities/a3s-review/icon?v=cd6293d906b3',
      coverUrl: '/api/capabilities/a3s-review/cover?v=573ad5be561a',
      surfaces: ['skill', 'ui']
    })
  })

  it('keeps AIX in the native plugin install flow and refuses foreign identity namespaces', async () => {
    const base = {
      slug: 'proof-views',
      kind: 'plugin',
      name: 'Views',
      installed: true,
      surfaces: ['skill', 'ui'],
      version: '1.0.0',
      install: {
        type: 'plugin',
        packageType: 'aix',
        available: true,
        digest: `sha256:${'ab'.repeat(32)}`,
        size: 120,
        packageId: 'use/proof/views'
      }
    }
    const page = await listCapabilities(
      { kind: 'plugin' },
      respond({
        code: 0,
        data: {
          items: [base, { ...base, slug: 'foreign', install: { ...base.install, packageId: 'vsx/proof/views' } }],
          total: 2,
          page: 1,
          pageSize: 24,
          categories: []
        }
      })
    )
    expect(page.items).toHaveLength(1)
    expect(page.items[0]).toMatchObject({
      kind: 'plugin',
      packageType: 'aix',
      installed: true,
      available: true,
      surfaces: ['skill', 'ui']
    })
  })

  it('rejects the retired VSIX package type from the capability catalog', async () => {
    const base = {
      slug: `vsix-${'a'.repeat(48)}`,
      kind: 'plugin',
      name: 'Sample',
      installed: true,
      surfaces: ['ui', 'tool'],
      applet: { permissions: ['user'] },
      install: {
        type: 'plugin',
        packageType: 'vsix',
        available: false,
        digest: `sha256:${'ab'.repeat(32)}`,
        size: 120,
        packageId: 'vsx/sample/diagnostic'
      }
    }
    const page = await listCapabilities({ kind: 'plugin' }, respond({ code: 0, data: { items: [base] } }))
    expect(page.items).toEqual([])
  })

  it('keeps a declared applet window and drops an unknown permission', async () => {
    const fetchImpl = respond({
      code: 0,
      data: {
        items: [
          {
            slug: 'a3s-leave',
            kind: 'plugin',
            name: '请假',
            install: {
              type: 'plugin',
              digest: `sha256:${'ab'.repeat(32)}`,
              size: 12,
              packageId: 'use/a3s/leave'
            },
            surfaces: ['ui'],
            applet: {
              window: { defaultWidth: 480, defaultHeight: 720, maxWidth: 840, maxHeight: 960 },
              permissions: ['knowledge', 'user']
            }
          },
          {
            slug: 'a3s-approval',
            kind: 'plugin',
            name: '审批',
            install: {
              type: 'plugin',
              digest: `sha256:${'cd'.repeat(32)}`,
              size: 12,
              packageId: 'use/a3s/approval'
            },
            surfaces: ['ui'],
            applet: { permissions: ['phone'] }
          }
        ],
        total: 2,
        page: 1,
        pageSize: 24,
        categories: []
      }
    })
    const page = await listCapabilities({ kind: 'plugin' }, fetchImpl)
    expect(page.items.find((item) => item.slug === 'a3s-leave')?.applet).toEqual({
      window: { defaultWidth: 480, defaultHeight: 720, maxWidth: 840, maxHeight: 960 },
      permissions: ['user', 'knowledge']
    })
    expect(page.items.find((item) => item.slug === 'a3s-approval')?.applet).toBeUndefined()
  })

  it('keeps a connector install spec and its config fields', async () => {
    const fetchImpl = respond({
      code: 0,
      data: {
        items: [
          {
            slug: 'github',
            kind: 'connector',
            category: 'code',
            name: 'GitHub',
            description: 'Repos',
            color: '#111111',
            featured: false,
            installed: true,
            install: { type: 'mcp', transport: 'stdio', command: 'npx', args: ['-y', 'server', 1] },
            config: [
              { key: 'GITHUB_PERSONAL_ACCESS_TOKEN', description: 'PAT', required: true, secret: true },
              { key: '9bad', description: 'nope', required: true, secret: true },
              { key: 'OPTIONAL', description: 4, required: 'yes', secret: 1 }
            ]
          }
        ],
        total: 1,
        page: 1,
        pageSize: 24,
        categories: []
      }
    })

    const page = await listCapabilities({ kind: 'connector' }, fetchImpl)
    expect(page.items[0]?.install).toEqual({ transport: 'stdio', command: 'npx', args: ['-y', 'server'] })
    expect(page.items[0]?.config).toEqual([
      { key: 'GITHUB_PERSONAL_ACCESS_TOKEN', description: 'PAT', required: true, secret: true },
      { key: 'OPTIONAL', description: '', required: false, secret: false }
    ])
  })

  it('rejects a full-catalog array', async () => {
    await expect(
      listCapabilities({ kind: 'skill' }, respond({ code: 0, message: 'ok', data: [] }))
    ).rejects.toMatchObject({ name: 'MemberOrgError' })
  })

  it('forgets a remembered applet allow when that plugin is installed or removed', async () => {
    localStorage.setItem('a3s.applet.permission-grant.v1', JSON.stringify({ leave: 'user', travel: 'user' }))
    localStorage.setItem('a3s.applet.profile-grant.v1', JSON.stringify(['leave', 'travel']))
    const fetchImpl = respond({ code: 0, message: 'ok', data: { slug: 'a3s-leave', installed: true } })

    await installCapability('a3s-leave', fetchImpl)

    expect(JSON.parse(localStorage.getItem('a3s.applet.permission-grant.v1') ?? '{}')).toEqual({ travel: 'user' })
    expect(JSON.parse(localStorage.getItem('a3s.applet.profile-grant.v1') ?? '[]')).toEqual(['travel'])

    localStorage.setItem('a3s.applet.permission-grant.v1', JSON.stringify({ leave: 'user' }))
    localStorage.setItem('a3s.applet.profile-grant.v1', JSON.stringify(['leave']))
    await uninstallCapability('a3s-leave', fetchImpl)

    expect(JSON.parse(localStorage.getItem('a3s.applet.permission-grant.v1') ?? '{}')).toEqual({})
    expect(JSON.parse(localStorage.getItem('a3s.applet.profile-grant.v1') ?? '[]')).toEqual([])
    localStorage.removeItem('a3s.applet.permission-grant.v1')
    localStorage.removeItem('a3s.applet.profile-grant.v1')
  })

  it('puts and deletes one installation', async () => {
    const fetchImpl = respond({ code: 0, message: 'ok', data: { slug: 'notion', installed: true } })

    await installCapability('notion', fetchImpl)
    await uninstallCapability('notion', fetchImpl)

    expect(fetchImpl.mock.calls.map(([url, init]) => [url, init?.method])).toEqual([
      ['http://127.0.0.1:4000/api/capabilities/installations/notion', 'PUT'],
      ['http://127.0.0.1:4000/api/capabilities/installations/notion', 'DELETE']
    ])
  })

  it('prepares a built-in skill and posts one package to the shell', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input)
      if (url.endsWith('/api/skills/builtin/skill-creator')) {
        return new Response(JSON.stringify({ name: 'skill-creator' }), { status: 200 })
      }
      return new Response(JSON.stringify({ error: 'invalid_name' }), { status: 400, headers: init?.headers })
    })
    vi.stubGlobal('fetch', fetchImpl)

    await prepareBuiltinSkill('skill-creator')
    const file = new File([new Uint8Array([0x50, 0x4b])], 'demo.skill', { type: 'application/zip' })
    await expect(uploadSkillPackage(file)).rejects.toMatchObject({ message: 'invalid_name', status: 400 })
    expect(String(fetchImpl.mock.calls[0]?.[0])).toBe('http://127.0.0.1:4000/api/skills/builtin/skill-creator')
    expect(String(fetchImpl.mock.calls[1]?.[0])).toBe('http://127.0.0.1:4000/api/skills/upload')
    const upload = fetchImpl.mock.calls[1]?.[1]
    expect(upload?.method).toBe('POST')
    expect(new Headers(upload?.headers).get('x-skill-filename')).toBe('demo.skill')
  })

  it('saves a custom connector on this device and reads the streamable http transport back', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (_input, init) => {
      if (init?.method === 'PUT') return new Response(JSON.stringify({ ok: true }), { status: 200 })
      if (init?.method === 'DELETE') return new Response(JSON.stringify({ ok: true }), { status: 200 })
      return new Response(
        JSON.stringify({
          connectors: [
            {
              name: 'search',
              transport: 'streamable_http',
              url: 'https://search.example/mcp',
              values: { X_API_KEY: 'tok_test' }
            }
          ]
        }),
        { status: 200 }
      )
    })
    vi.stubGlobal('fetch', fetchImpl)

    await saveCustomConnector({
      name: 'search',
      transport: 'streamable_http',
      url: 'https://search.example/mcp',
      values: { X_API_KEY: 'tok_test' }
    })
    expect(await listCustomConnectors()).toEqual([
      {
        name: 'search',
        transport: 'streamable_http',
        url: 'https://search.example/mcp',
        values: { X_API_KEY: 'tok_test' }
      }
    ])
    await removeCustomConnector('search')
    expect(String(fetchImpl.mock.calls[0]?.[0])).toBe('http://127.0.0.1:4000/api/capabilities/custom')
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('PUT')
    expect(new Headers(fetchImpl.mock.calls[0]?.[1]?.headers).get('authorization')).toBe('Bearer shell-token')
    expect(fetchImpl.mock.calls[1]?.[1]).not.toHaveProperty('body')
    expect(String(fetchImpl.mock.calls[2]?.[0])).toBe('http://127.0.0.1:4000/api/capabilities/custom/search')
    expect(fetchImpl.mock.calls[2]?.[1]?.method).toBe('DELETE')

    fetchImpl.mockResolvedValueOnce(new Response(JSON.stringify({ error: 'connector_exists' }), { status: 400 }))
    await expect(
      saveCustomConnector({ name: 'github', transport: 'stdio', command: 'npx', args: [], values: {} })
    ).rejects.toMatchObject({ message: 'connector_exists', status: 400 })
  })

  it('carries the HTTP status when the member is signed out', async () => {
    await expect(
      listCapabilities({ kind: 'connector' }, respond({ error: 'unauthorized' }, 401))
    ).rejects.toMatchObject({
      name: 'MemberOrgError',
      status: 401
    })
  })
})

describe('searchCapabilities', () => {
  it('matches names and descriptions case-insensitively', () => {
    expect(searchCapabilities(CATALOG, 'GITHUB').map((row) => row.slug)).toEqual(['github'])
    expect(searchCapabilities(CATALOG, 'repositories').map((row) => row.slug)).toEqual(['github'])
  })

  it('returns every entry for an empty or whitespace-only query', () => {
    expect(searchCapabilities(CATALOG, '')).toHaveLength(CATALOG.length)
    expect(searchCapabilities(CATALOG, '   ')).toHaveLength(CATALOG.length)
  })

  it('returns nothing when no entry matches', () => {
    expect(searchCapabilities(CATALOG, 'no-such-capability')).toEqual([])
  })
})

describe('capabilitySections', () => {
  it('places the featured section first and only featured entries in it', () => {
    const sections = capabilitySections(CATALOG)
    expect(sections[0]?.id).toBe('featured')
    expect(sections[0]?.entries.map((row) => row.slug)).toEqual(['github', 'notion', 'deep-research'])
  })

  it('lists non-empty category sections in canonical order', () => {
    const canonical = CAPABILITY_CATEGORIES.map((category) => category.id)
    const categorySections = capabilitySections(CATALOG)
      .slice(1)
      .map((section) => section.id)
    expect(categorySections).toEqual(canonical.filter((id) => CATALOG.some((row) => row.category === id)))
  })

  it('keeps sections scoped to the entries it is given', () => {
    const skillsOnly = CATALOG.filter((row) => row.kind === 'skill')
    const sections = capabilitySections(skillsOnly)
    expect(sections.map((section) => section.id)).toEqual(['featured', 'office', 'other'])
    expect(sections.flatMap((section) => section.entries)).toHaveLength(skillsOnly.length + 1)
  })
})
