import type { Page, Route } from '@playwright/test'

/** Signed member the home pin key and the AIX catalogue both wait for. */
export const UI_MEMBER_ID = '11111111-1111-4111-8111-111111111111'

const UI_HOME = '/tmp/a3s-workbench-office'
const UI_STAMP = '2026-10-07T00:00:00.000Z'
const UI_REVISION = `sha256:${'ab'.repeat(32)}`

export interface WorkbenchUiProject {
  id: string
  name: string
  path: string
}

export interface WorkbenchUiSession {
  id: string
  name: string
  pinned?: boolean
  place: 'inbox' | 'project'
  projectId?: string
  updatedAt?: string
}

export interface WorkbenchUiTask {
  id: string
  name: string
  prompt: string
}

export interface WorkbenchUiInstall {
  /** Default true. The no-agent automation path sets this false. */
  withAgent?: boolean
  projects?: WorkbenchUiProject[]
  sessions?: WorkbenchUiSession[]
  tasks?: WorkbenchUiTask[]
  connectors?: { slug: string; name: string }[]
  skills?: { slug: string; name: string }[]
  /** Keep the live composer out of the host so the fallback input can take a file. */
  holdComposer?: boolean
}

export const UI_PROJECT: WorkbenchUiProject = {
  id: 'project-a3s',
  name: 'A3S',
  path: `${UI_HOME}/.a3s/projects/A3S`
}

type UiAgent = {
  id: string
  type: string
  name: string
  model: string
  createdAt: string
  updatedAt: string
  orderKey: string
}

type StoredSession = {
  id: string
  agentId: string
  name: string
  isNameManuallyEdited: boolean
  workspaceId: string
  workspace: Record<string, unknown>
  orderKey: string
  lastActivityAt: string
  createdAt: string
  updatedAt: string
}

type StoredTask = Record<string, unknown>

export interface WorkbenchUiBootstrap {
  runtime: { apiBaseUrl: string; token: string; homePath: string }
  memberId: string
  projects: Record<string, unknown>[]
  holdComposer: boolean
}

function projectRow(project: WorkbenchUiProject) {
  return {
    id: project.id,
    name: project.name,
    path: project.path,
    type: 'user',
    orderKey: 'a0',
    createdAt: UI_STAMP,
    updatedAt: UI_STAMP
  }
}

export function workbenchUiBootstrap(options: WorkbenchUiInstall): WorkbenchUiBootstrap {
  return {
    runtime: {
      apiBaseUrl: 'http://127.0.0.1:4318',
      token: 'ab'.repeat(32),
      homePath: UI_HOME
    },
    memberId: UI_MEMBER_ID,
    projects: (options.projects ?? [UI_PROJECT]).map(projectRow),
    holdComposer: options.holdComposer === true
  }
}

function inboxWorkspace() {
  return {
    id: 'workspace-inbox',
    name: '收件箱',
    path: `${UI_HOME}/.a3s/sessions`,
    type: 'system',
    orderKey: 'a0',
    createdAt: UI_STAMP,
    updatedAt: UI_STAMP
  }
}

function sessionRecord(seed: WorkbenchUiSession, projects: WorkbenchUiProject[], agentId: string): StoredSession {
  const project =
    seed.place === 'project' ? (projects.find((item) => item.id === seed.projectId) ?? projects[0] ?? null) : null
  const workspace = project ? projectRow(project) : inboxWorkspace()
  const updatedAt = seed.updatedAt ?? UI_STAMP
  return {
    id: seed.id,
    agentId,
    name: seed.name,
    isNameManuallyEdited: true,
    workspaceId: workspace.id,
    workspace,
    orderKey: 'a0',
    lastActivityAt: updatedAt,
    createdAt: UI_STAMP,
    updatedAt
  }
}

function taskRecord(seed: WorkbenchUiTask, agentId: string): StoredTask {
  return {
    id: seed.id,
    agentId,
    name: seed.name,
    prompt: seed.prompt,
    trigger: { kind: 'cron', expr: '0 9 * * *' },
    timeoutMinutes: 30,
    workspace: { type: 'system' },
    reuseSession: false,
    reuseSessionId: null,
    enabled: true,
    status: 'active',
    createdAt: UI_STAMP,
    updatedAt: UI_STAMP
  }
}

function listedAgent(agent: UiAgent) {
  return { ...agent, modelName: '工作模型' }
}

function taskListItem(task: StoredTask) {
  return { ...task, runSummary: null }
}

function capabilityItem(kind: 'connector' | 'skill', slug: string, name: string) {
  return {
    slug,
    kind,
    name,
    description: kind === 'connector' ? '测试连接器' : '测试技能',
    category: 'other',
    installed: false
  }
}

const HOME_CATALOGUE = {
  protocolVersion: 1,
  revision: '',
  packages: [
    {
      slug: 'home-writing',
      packageId: 'use/a3s/home-writing',
      generation: 1,
      revision: UI_REVISION,
      containers: [],
      views: [],
      home: [
        {
          id: 'notice',
          shelf: 'write',
          title: '通知',
          description: '发布需要周知或执行的事项。',
          order: 0,
          action: 'writing',
          writingType: 'notice'
        }
      ]
    }
  ]
}

const ORG_MODELS = {
  success: true,
  data: {
    items: [
      {
        id: 'gpt-work',
        name: '工作模型',
        protocols: ['openai'],
        capabilities: ['tool_call'],
        available: true
      }
    ],
    defaultModel: 'gpt-work'
  }
}

function pageOf(items: unknown[]) {
  return { items, nextCursor: null, total: items.length }
}

function applyWorkspace(session: StoredSession, body: Record<string, unknown>) {
  const workspace = body.workspace
  if (body.type === 'user' && workspace && typeof workspace === 'object') {
    const row = workspace as { id?: unknown }
    session.workspace = workspace as Record<string, unknown>
    if (typeof row.id === 'string') session.workspaceId = row.id
  } else if (body.type === 'system') {
    const inbox = inboxWorkspace()
    if (typeof body.path === 'string' && body.path) inbox.path = body.path
    session.workspace = inbox
    session.workspaceId = inbox.id
  }
  session.updatedAt = new Date().toISOString()
}

/** Stub the work-home reads. Unknown `/api` calls fall through; they are not aborted. */
export async function installWorkbenchUi(page: Page, options: WorkbenchUiInstall, agent: UiAgent) {
  const projects = options.projects ?? [UI_PROJECT]
  const withAgent = options.withAgent !== false
  const sessions = (options.sessions ?? []).map((seed) => sessionRecord(seed, projects, agent.id))
  const tasks = (options.tasks ?? []).map((seed) => taskRecord(seed, agent.id))
  const connectors = (options.connectors ?? [{ slug: 'mail', name: '邮箱' }]).map((item) =>
    capabilityItem('connector', item.slug, item.name)
  )
  const skills = (options.skills ?? []).map((item) => capabilityItem('skill', item.slug, item.name))
  const pins = (options.sessions ?? [])
    .filter((session) => session.pinned)
    .map((session) => ({
      id: `pin-${session.id}`,
      entityType: 'session',
      entityId: session.id,
      orderKey: 'a0',
      createdAt: UI_STAMP,
      updatedAt: UI_STAMP
    }))

  const fulfill = (route: Route, body: unknown, status = 200) =>
    route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) })

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url())
    let path = url.pathname
    try {
      path = decodeURIComponent(path)
    } catch {
      // Keep the raw pathname when it is not valid percent-encoding.
    }
    const method = route.request().method().toUpperCase()
    if (path === '/api/agent-plane' && method === 'GET') return fulfill(route, { baseUrl: 'http://127.0.0.1:4318' })
    if (path === '/api/os-gateway/credential' && method === 'GET') {
      return fulfill(route, { origin: 'http://127.0.0.1:4318', token: 'session-token' })
    }
    if (path === '/api/member/me' || path === '/api/member-session/profile') {
      return fulfill(route, { id: UI_MEMBER_ID, display_name: '成员', phone: '1' })
    }
    if (path === '/api/member-orgs' && method === 'GET') return fulfill(route, [])
    if (path === '/api/aix/views' && method === 'GET') return fulfill(route, HOME_CATALOGUE)
    if (path === '/api/local-skills' && method === 'GET') return fulfill(route, { skills: [], roots: [] })
    if (path === '/api/agent-events') return route.fulfill({ status: 204, body: '' })
    if (path === '/api/workspace-entries' && method === 'POST') return fulfill(route, {})
    if (path === '/api/agent-channels/sync' && method === 'POST') return fulfill(route, {})
    if (path.startsWith('/api/composer-files/') && method === 'POST') {
      const name = decodeURIComponent(path.slice('/api/composer-files/'.length))
      return fulfill(route, { path: `${UI_HOME}/.a3s/${name}`, name, size: 5 })
    }
    if (path === '/api/capabilities/custom' && method === 'GET') return fulfill(route, { connectors: [] })
    if (path === '/api/capabilities/custom' && method === 'PUT') return fulfill(route, { connectors: [] })
    if (path === '/api/capabilities' && method === 'GET') {
      const kind = url.searchParams.get('kind')
      const items = kind === 'skill' ? skills : kind === 'connector' ? connectors : []
      return fulfill(route, { items, total: items.length, page: 1, pageSize: 24, categories: [] })
    }
    if (path === '/api/agents' && method === 'GET') {
      const items = withAgent ? [listedAgent(agent)] : []
      return fulfill(route, { items, total: items.length, page: 1 })
    }
    if (path === `/api/agents/${agent.id}` && method === 'GET') return fulfill(route, listedAgent(agent))
    if (path === '/api/pins' && method === 'GET') return fulfill(route, pins)
    if (path === '/api/agent-sessions' && method === 'GET') return fulfill(route, pageOf(sessions))
    if (path === '/api/agent-sessions/latest' && method === 'GET') return fulfill(route, { session: null, topic: null })
    if (path === '/api/agent-sessions/archive' && method === 'POST') {
      const body = route.request().postDataJSON() as { sessionIds?: unknown }
      const ids = new Set(Array.isArray(body?.sessionIds) ? body.sessionIds.filter((id) => typeof id === 'string') : [])
      for (let index = sessions.length - 1; index >= 0; index -= 1) {
        if (ids.has(sessions[index].id)) sessions.splice(index, 1)
      }
      return fulfill(route, {})
    }
    const workspacePath = /^\/api\/agent-sessions\/([^/]+)\/workspace$/.exec(path)
    if (workspacePath && method === 'PUT') {
      const session = sessions.find((item) => item.id === workspacePath[1])
      if (!session) return fulfill(route, { message: 'missing session' }, 404)
      const body = (route.request().postDataJSON() ?? {}) as Record<string, unknown>
      applyWorkspace(session, body)
      return fulfill(route, session)
    }
    const messagesPath = /^\/api\/agent-sessions\/([^/]+)\/messages$/.exec(path)
    if (messagesPath && method === 'GET') return fulfill(route, pageOf([]))
    const sessionPath = /^\/api\/agent-sessions\/([^/]+)$/.exec(path)
    if (sessionPath && method === 'GET') {
      const session = sessions.find((item) => item.id === sessionPath[1])
      return session ? fulfill(route, session) : fulfill(route, { message: 'missing session' }, 404)
    }
    if (sessionPath && method === 'PATCH') {
      const session = sessions.find((item) => item.id === sessionPath[1])
      if (!session) return fulfill(route, { message: 'missing session' }, 404)
      const body = (route.request().postDataJSON() ?? {}) as { name?: unknown; isNameManuallyEdited?: unknown }
      if (typeof body.name === 'string') session.name = body.name
      if (body.isNameManuallyEdited === true) session.isNameManuallyEdited = true
      session.updatedAt = new Date().toISOString()
      return fulfill(route, session)
    }
    if (path === '/api/agent-tasks' && method === 'GET') {
      const items = tasks.map(taskListItem)
      return fulfill(route, { items, total: items.length, page: 1 })
    }
    if (path === '/api/agent-tasks' && method === 'POST') {
      const body = (route.request().postDataJSON() ?? {}) as { agentId?: unknown; name?: unknown; prompt?: unknown }
      const created = taskRecord(
        {
          id: `task-created-${tasks.length + 1}`,
          name: typeof body.name === 'string' ? body.name : '',
          prompt: typeof body.prompt === 'string' ? body.prompt : ''
        },
        typeof body.agentId === 'string' ? body.agentId : agent.id
      )
      tasks.push(created)
      return fulfill(route, created, 201)
    }
    const taskLogs = /^\/api\/agent-tasks\/([^/]+)\/logs$/.exec(path)
    if (taskLogs && method === 'GET') return fulfill(route, { items: [], total: 0, page: 1 })
    const taskPath = /^\/api\/agent-tasks\/([^/]+)$/.exec(path)
    if (taskPath && method === 'GET') {
      const task = tasks.find((item) => item.id === taskPath[1])
      return task ? fulfill(route, task) : fulfill(route, { message: 'missing task' }, 404)
    }
    const agentTasks = /^\/api\/agents\/([^/]+)\/tasks$/.exec(path)
    if (agentTasks && method === 'GET') {
      const items = tasks.filter((item) => item.agentId === agentTasks[1])
      return fulfill(route, { items, total: items.length, page: 1 })
    }
    return route.fallback()
  })

  await page.route('**/v1/api/org/models', async (route) => {
    if (route.request().method().toUpperCase() !== 'GET') {
      await route.fallback()
      return
    }
    await fulfill(route, ORG_MODELS)
  })
}
