import {
  createSidebarShortcutId,
  type SidebarFavorite,
  type SidebarShortcutItem,
  type SidebarShortcutTarget
} from '@shared/data/preference/preferenceTypes'
import { CONVERSATION_ROUTES, conversationRouteUrl } from '@shared/utils/conversationRoute'

/**
 * Context passed to sidebar navigation handlers. Carries per-call state the
 * registry can't know on its own (preferences).
 */
export interface SidebarNavContext {
  defaultPaintingProvider: string
}

/**
 * Apps that hold conversations (chat→topic, agent→session) carry a
 * `conversationRoute`: the conversation-key↔URL mapping. Which
 * conversation a bare entry lands on is resolved by the routes' own `beforeLoad`
 * interceptors, not here. Apps without it (files / notes / …) are
 * plain route entries.
 */
export interface SidebarConversationRoute {
  /** Extract the conversation key (topicId / sessionId) from an existing tab URL. */
  keyFromUrl: (url: string) => string | undefined
  /** Build the tab URL for a conversation key (keeps dispatch app-agnostic). */
  urlForKey: (key: string) => string
}

interface SidebarAppDefinition<Id extends SidebarFavorite = SidebarFavorite> {
  id: Id
  routePrefix: string
  /** Url to open when no tab exists yet (defaults to `routePrefix`). */
  resolveUrl?: (ctx: SidebarNavContext) => string
  /** Highlight the sidebar entry only on the exact base route, not on sub-routes owned by the app. */
  exactRouteFocus?: boolean
  conversationRoute?: SidebarConversationRoute
}

function getConversationSearchParamFromUrl(url: string, name: string): string | undefined {
  try {
    return new URL(url, 'app://x').searchParams.get(name) ?? undefined
  } catch {
    return undefined
  }
}

/**
 * Single source of truth for sidebar applications.
 * Order here is the canonical sidebar order and drives preference defaults.
 */
const SIDEBAR_APP_DEFINITIONS = [
  {
    id: 'agents',
    routePrefix: '/app/agents',
    conversationRoute: {
      keyFromUrl: (url) => getConversationSearchParamFromUrl(url, CONVERSATION_ROUTES.agent.keyParam),
      urlForKey: (key) => conversationRouteUrl({ conversationType: 'agent', conversationId: key })
    }
  },
  {
    id: 'messages',
    routePrefix: '/app/messages'
  },
  {
    id: 'schedule',
    routePrefix: '/app/schedule'
  },
  {
    id: 'mail',
    routePrefix: '/app/mail'
  },
  {
    id: 'org',
    routePrefix: '/app/org'
  },
  {
    id: 'platform',
    routePrefix: '/app/platform'
  },
  {
    id: 'assistants',
    // `routePrefix` must stay a string literal — the knowledge-manifest generator reads it
    // with ts-morph. `conversationRoute` below carries the same path from the shared contract.
    routePrefix: '/app/chat',
    conversationRoute: {
      keyFromUrl: (url) => getConversationSearchParamFromUrl(url, CONVERSATION_ROUTES.assistant.keyParam),
      urlForKey: (key) => conversationRouteUrl({ conversationType: 'assistant', conversationId: key })
    }
  },
  {
    id: 'mini_app',
    routePrefix: '/app/mini-app',
    exactRouteFocus: true
  },
  {
    id: 'knowledge',
    routePrefix: '/app/knowledge'
  },
  {
    id: 'files',
    routePrefix: '/app/files'
  },
  {
    id: 'code_tools',
    routePrefix: '/app/code'
  },
  {
    id: 'notes',
    routePrefix: '/app/notes'
  },
  {
    id: 'capabilities',
    routePrefix: '/app/capabilities'
  },
  {
    id: 'documents',
    routePrefix: '/app/documents'
  },
  {
    id: 'plugins',
    routePrefix: '/app/plugins'
  }
] as const satisfies readonly SidebarAppDefinition[]

export type SidebarAppId = (typeof SIDEBAR_APP_DEFINITIONS)[number]['id']
export type SidebarApp = SidebarAppDefinition<SidebarAppId>

export const SIDEBAR_APPS: readonly SidebarApp[] = SIDEBAR_APP_DEFINITIONS

/**
 * assistants (对话) stay in preferences and routes, but are not shown and an
 * open tab leaves that page. Until the user drags the rail, knowledge (知识)
 * sits directly under 工作, and messages (消息) follows 知识. 数据 follows 知识.
 * 日程 and 邮件
 * live in the team sidebar. 云盘 is the team sidebar entry, not an ActivityBar item.
 * plugins (插件) stays immediately after 团队; a
 * personal center drops 团队 and keeps 插件. A drag saves the upper-rail order.
 */
const TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS = new Set<SidebarAppId>(['assistants'])

/** capabilities and documents stay reachable, but are not ActivityBar items. messages stays on the rail. */
const REMOVED_ACTIVITY_BAR_APP_IDS = new Set<SidebarAppId>(['capabilities', 'documents'])

const SIDEBAR_APP_BY_ID: Record<SidebarAppId, SidebarApp> = SIDEBAR_APPS.reduce(
  (acc, app) => {
    acc[app.id] = app
    return acc
  },
  {} as Record<SidebarAppId, SidebarApp>
)

export function getSidebarApp(id: SidebarAppId): SidebarApp | undefined {
  return SIDEBAR_APP_BY_ID[id]
}

/**
 * A tab belongs to an app when its url is the route itself, a path sub-route,
 * or a query-param instance of it. Shared by the sidebar dispatcher and the
 * conversation-navigation boundary so the matcher lives in exactly one place.
 */
export function tabBelongsToApp(app: SidebarApp, url: string): boolean {
  return url === app.routePrefix || url.startsWith(`${app.routePrefix}/`) || url.startsWith(`${app.routePrefix}?`)
}

/**
 * 侧边栏支持的完整菜单顺序。
 * Preference 默认值可能不包含新菜单，管理态列表仍需要覆盖当前全部支持项。
 */
export const SIDEBAR_FAVORITE_ORDER: SidebarAppId[] = SIDEBAR_APPS.map((app) => app.id)

const sidebarFavoriteSet = new Set<SidebarAppId>(SIDEBAR_FAVORITE_ORDER)

export function getSidebarMenuPath(favorite: SidebarAppId, defaultPaintingProvider: string): string {
  const app = getSidebarApp(favorite)
  if (!app) return ''
  return app.resolveUrl?.({ defaultPaintingProvider }) ?? app.routePrefix
}

export function resolveSidebarActiveItem(url: string): SidebarAppId | '' {
  const match = SIDEBAR_APPS.find((app) => (app.exactRouteFocus ? url === app.routePrefix : tabBelongsToApp(app, url)))
  return match?.id ?? ''
}

/** Where an open tab should go while its ActivityBar app is temporarily hidden. */
export function temporarilyHiddenActivityBarReplacement(url: string): string | null {
  const appId = resolveSidebarActiveItem(url)
  if (!appId || !TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS.has(appId)) return null
  const next = SIDEBAR_APPS.find((app) => !TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS.has(app.id))
  return next?.routePrefix ?? null
}

export function isSidebarAppId(value: string): value is SidebarAppId {
  return sidebarFavoriteSet.has(value as SidebarAppId)
}

export const SIDEBAR_SHORTCUT_PROVIDER_IDS = {
  APP: 'core.app',
  MINI_APP: 'core.mini-app',
  AGENT: 'core.agent',
  ASSISTANT: 'core.assistant',
  KNOWLEDGE_BASE: 'core.knowledge-base',
  TOPIC: 'core.topic',
  AGENT_SESSION: 'core.agent-session',
  FILE_ENTRY: 'core.file-entry',
  CODE_CLI: 'core.code-cli'
} as const

const RETIRED_SIDEBAR_SHORTCUT_PROVIDER_IDS = new Set(['core.skill', 'core.mcp-server', 'core.provider'])

export function createSidebarShortcutTarget(
  providerId: string,
  resourceId: string,
  activationId?: string
): SidebarShortcutTarget {
  return {
    kind: 'resource',
    locator: { providerId, resourceId },
    ...(activationId === undefined ? {} : { activationId })
  }
}

const LEGACY_PROVIDER_BY_TYPE = {
  app: SIDEBAR_SHORTCUT_PROVIDER_IDS.APP,
  mini_app: SIDEBAR_SHORTCUT_PROVIDER_IDS.MINI_APP,
  agent: SIDEBAR_SHORTCUT_PROVIDER_IDS.AGENT,
  assistant: SIDEBAR_SHORTCUT_PROVIDER_IDS.ASSISTANT
} as const

type StoredSidebarItem = Record<string, unknown>

function isRecord(value: unknown): value is StoredSidebarItem {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export function isSidebarShortcutTarget(value: unknown): value is SidebarShortcutTarget {
  if (!isRecord(value) || value.kind !== 'resource' || !isRecord(value.locator)) return false
  const { providerId, resourceId } = value.locator
  return (
    typeof providerId === 'string' &&
    providerId.length > 0 &&
    typeof resourceId === 'string' &&
    resourceId.length > 0 &&
    (value.activationId === undefined || (typeof value.activationId === 'string' && value.activationId.length > 0))
  )
}

export function isSidebarShortcutItem(value: unknown): value is SidebarShortcutItem {
  return isRecord(value) && value.type === 'shortcut' && isSidebarShortcutTarget(value.target)
}

function normalizeKnownSidebarShortcut(value: StoredSidebarItem): SidebarShortcutItem | undefined {
  if (value.type === 'shortcut') {
    if (!isSidebarShortcutTarget(value.target)) return undefined
    if (RETIRED_SIDEBAR_SHORTCUT_PROVIDER_IDS.has(value.target.locator.providerId)) return undefined
    if (
      value.target.locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
      !isSidebarAppId(value.target.locator.resourceId)
    ) {
      return undefined
    }
    return {
      type: 'shortcut',
      id: createSidebarShortcutId(value.target),
      target: value.target,
      ...(typeof value.fallbackLabel === 'string' && value.fallbackLabel.length > 0
        ? { fallbackLabel: value.fallbackLabel }
        : {})
    }
  }

  if (typeof value.type !== 'string' || !Object.hasOwn(LEGACY_PROVIDER_BY_TYPE, value.type)) return undefined
  if (typeof value.id !== 'string' || value.id.length === 0) return undefined
  if (value.type === 'app' && !isSidebarAppId(value.id)) return undefined

  const providerId = LEGACY_PROVIDER_BY_TYPE[value.type as keyof typeof LEGACY_PROVIDER_BY_TYPE]
  const target = createSidebarShortcutTarget(providerId, value.id)
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(target),
    target,
    ...(typeof value.fallbackLabel === 'string' && value.fallbackLabel.length > 0
      ? { fallbackLabel: value.fallbackLabel }
      : {})
  }
}

function isForwardCompatibleSidebarItem(value: StoredSidebarItem): boolean {
  return (
    typeof value.type === 'string' &&
    value.type !== 'shortcut' &&
    !Object.hasOwn(LEGACY_PROVIDER_BY_TYPE, value.type) &&
    typeof value.id === 'string' &&
    value.id.length > 0
  )
}

/** Normalize storage, migrate legacy leaves, and preserve future items. */
export function normalizeSidebarShortcutItems(values: readonly unknown[] | undefined): SidebarShortcutItem[] {
  const items: SidebarShortcutItem[] = []
  const seen = new Set<string>()

  for (const value of values ?? []) {
    if (!isRecord(value)) continue
    const shortcut = normalizeKnownSidebarShortcut(value)
    if (shortcut) {
      if (seen.has(shortcut.id)) continue
      seen.add(shortcut.id)
      items.push(shortcut)
      continue
    }
    if (!isForwardCompatibleSidebarItem(value)) continue

    const futureKey = `${String(value.type)}:${String(value.id)}`
    if (seen.has(futureKey)) continue
    seen.add(futureKey)
    items.push(value as unknown as SidebarShortcutItem)
  }

  return items
}

function isHiddenActivityBarShortcut(item: SidebarShortcutItem): boolean {
  const { locator } = item.target
  return (
    locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
    isSidebarAppId(locator.resourceId) &&
    (locator.resourceId === 'mail' ||
      locator.resourceId === 'schedule' ||
      TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS.has(locator.resourceId) ||
      REMOVED_ACTIVITY_BAR_APP_IDS.has(locator.resourceId))
  )
}

const MESSAGES_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'messages')

function messagesActivityShortcut(): SidebarShortcutItem {
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(MESSAGES_ACTIVITY_TARGET),
    target: MESSAGES_ACTIVITY_TARGET
  }
}

function insertShortcutAfter(
  items: SidebarShortcutItem[],
  item: SidebarShortcutItem,
  anchorId: string
): SidebarShortcutItem[] {
  if (items.some((entry) => entry.id === item.id)) return items
  const anchor = items.findIndex((entry) => entry.id === anchorId)
  if (anchor < 0) return items
  return [...items.slice(0, anchor + 1), item, ...items.slice(anchor + 1)]
}

/** Until the user drags the rail, 消息 follows 知识, or 工作 when 知识 is absent. */
function withMessagesUnderWork(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const messages = messagesActivityShortcut()
  const without = items.filter((item) => item.id !== messages.id)
  const knowledgeId = createSidebarShortcutId(
    createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'knowledge')
  )
  const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
  const knowledgeAnchor = without.findIndex((item) => item.id === knowledgeId)
  const anchor = knowledgeAnchor >= 0 ? knowledgeAnchor : without.findIndex((item) => item.id === agentsId)
  if (anchor < 0) return items
  return [...without.slice(0, anchor + 1), messages, ...without.slice(anchor + 1)]
}

function insertMessagesUnderWork(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
  return insertShortcutAfter(items, messagesActivityShortcut(), agentsId)
}

const KNOWLEDGE_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'knowledge')

function knowledgeActivityShortcut(): SidebarShortcutItem {
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(KNOWLEDGE_ACTIVITY_TARGET),
    target: KNOWLEDGE_ACTIVITY_TARGET
  }
}

/** Until the user drags the rail, 知识 sits directly under 工作. */
function withKnowledgeUnderWork(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const knowledge = knowledgeActivityShortcut()
  const without = items.filter((item) => item.id !== knowledge.id)
  const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
  const anchor = without.findIndex((item) => item.id === agentsId)
  if (anchor < 0) return items
  return [...without.slice(0, anchor + 1), knowledge, ...without.slice(anchor + 1)]
}

function insertKnowledgeUnderWork(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const knowledge = knowledgeActivityShortcut()
  const messagesId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'messages'))
  const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
  if (items.some((item) => item.id === messagesId)) return insertShortcutAfter(items, knowledge, messagesId)
  return insertShortcutAfter(items, knowledge, agentsId)
}

const ORG_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'org')

function orgActivityShortcut(): SidebarShortcutItem {
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(ORG_ACTIVITY_TARGET),
    target: ORG_ACTIVITY_TARGET
  }
}

const DOCUMENTS_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'documents')

function documentsActivityShortcut(): SidebarShortcutItem {
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(DOCUMENTS_ACTIVITY_TARGET),
    target: DOCUMENTS_ACTIVITY_TARGET
  }
}

function shortcutAppId(item: SidebarShortcutItem): SidebarAppId | null {
  const { locator } = item.target
  if (locator.providerId !== SIDEBAR_SHORTCUT_PROVIDER_IDS.APP || !isSidebarAppId(locator.resourceId)) return null
  return locator.resourceId
}

/** Documents stays off the rail. Insert it after the last earlier app only while it is an ActivityBar item. */
function withDocumentsOnRail(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  if (REMOVED_ACTIVITY_BAR_APP_IDS.has('documents')) return items
  const documents = documentsActivityShortcut()
  if (items.some((item) => item.id === documents.id)) return items
  const documentsIndex = SIDEBAR_FAVORITE_ORDER.indexOf('documents')
  let anchor = -1
  items.forEach((item, index) => {
    const appId = shortcutAppId(item)
    if (!appId) return
    const order = SIDEBAR_FAVORITE_ORDER.indexOf(appId)
    if (order >= 0 && order < documentsIndex) anchor = index
  })
  if (anchor < 0) return items
  return [...items.slice(0, anchor + 1), documents, ...items.slice(anchor + 1)]
}

const PLUGINS_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'plugins')

function pluginsActivityShortcut(): SidebarShortcutItem {
  return {
    type: 'shortcut',
    id: createSidebarShortcutId(PLUGINS_ACTIVITY_TARGET),
    target: PLUGINS_ACTIVITY_TARGET
  }
}

/** 插件 sits immediately after 组织. When 组织 is already off the list, 插件 stays. */
function withPluginsOnRail(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const plugins = pluginsActivityShortcut()
  const orgId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'org'))
  const without = items.filter((item) => item.id !== plugins.id)
  const orgIndex = without.findIndex((item) => item.id === orgId)
  if (orgIndex < 0) {
    const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
    if (!without.some((item) => item.id === agentsId)) return items
    return [...without, plugins]
  }
  return [...without.slice(0, orgIndex + 1), plugins, ...without.slice(orgIndex + 1)]
}

function insertPluginsOnRail(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const plugins = pluginsActivityShortcut()
  if (items.some((item) => item.id === plugins.id)) return items
  const orgId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'org'))
  const orgIndex = items.findIndex((item) => item.id === orgId)
  if (orgIndex < 0) {
    const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
    if (!items.some((item) => item.id === agentsId)) return items
    return [...items, plugins]
  }
  return [...items.slice(0, orgIndex + 1), plugins, ...items.slice(orgIndex + 1)]
}

/** Organization follows messages, or work when messages is not on the rail. */
function withOrgUnderMessages(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const org = orgActivityShortcut()
  if (items.some((item) => item.id === org.id)) return items
  const messagesId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'messages'))
  const agentsId = createSidebarShortcutId(createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'agents'))
  const messagesAnchor = items.findIndex((item) => item.id === messagesId)
  const anchor = messagesAnchor >= 0 ? messagesAnchor : items.findIndex((item) => item.id === agentsId)
  if (anchor < 0) return items
  return [...items.slice(0, anchor + 1), org, ...items.slice(anchor + 1)]
}

const PLATFORM_ACTIVITY_TARGET = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'platform')

/** 平台 joins the rail next to 团队; 插件 then takes the slot right after 团队. Shown only for the official platform team. */
function withPlatformAfterOrg(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  const platform: SidebarShortcutItem = {
    type: 'shortcut',
    id: createSidebarShortcutId(PLATFORM_ACTIVITY_TARGET),
    target: PLATFORM_ACTIVITY_TARGET
  }
  return insertShortcutAfter(items, platform, orgActivityShortcut().id)
}

/** 平台 is an ActivityBar app only for directory admins of the official team. */
export function isPlatformActivityApp(id: string): boolean {
  return id === 'platform'
}

const TEAM_ONLY_ACTIVITY_APP_IDS = new Set<SidebarAppId>(['schedule', 'org', 'platform'])

/** 日程 and 组织 stay off the rail while the center is personal. */
export function isTeamOnlyActivityApp(id: string): boolean {
  return isSidebarAppId(id) && TEAM_ONLY_ACTIVITY_APP_IDS.has(id)
}

const ACTIVITY_BAR_ORDER_CUSTOM = {
  type: 'activity-order',
  id: 'custom'
} as unknown as SidebarShortcutItem

export function activityBarOrderIsCustom(values: readonly unknown[] | undefined): boolean {
  return (values ?? []).some((item) => isRecord(item) && item.type === 'activity-order' && item.id === 'custom')
}

/** Remember that the ActivityBar order is the user's, not the default layout. */
export function markActivityBarOrderCustom(items: readonly SidebarShortcutItem[]): SidebarShortcutItem[] {
  if (activityBarOrderIsCustom(items)) return [...items]
  return [...items, ACTIVITY_BAR_ORDER_CUSTOM]
}

function visibleShortcutItems(values: readonly unknown[] | undefined): SidebarShortcutItem[] {
  return normalizeSidebarShortcutItems(values)
    .filter(isSidebarShortcutItem)
    .filter((item) => !isHiddenActivityBarShortcut(item))
}

/** Default rail. 知识 is second, then 消息, 组织, and 插件. 平台 follows 插件. */
function canonicalActivityBar(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  return withPluginsOnRail(
    withDocumentsOnRail(
      withMessagesUnderWork(withKnowledgeUnderWork(withPlatformAfterOrg(withOrgUnderMessages(items))))
    )
  )
}

/** After a drag, stored positions stay. Only a missing app is inserted. */
function customActivityBar(items: SidebarShortcutItem[]): SidebarShortcutItem[] {
  return insertPluginsOnRail(
    withDocumentsOnRail(
      insertKnowledgeUnderWork(insertMessagesUnderWork(withPlatformAfterOrg(withOrgUnderMessages(items))))
    )
  )
}

export function getVisibleSidebarShortcutItems(values: readonly unknown[] | undefined): SidebarShortcutItem[] {
  const items = visibleShortcutItems(values)
  return activityBarOrderIsCustom(values) ? customActivityBar(items) : canonicalActivityBar(items)
}

function isBuiltInAppShortcutTarget(target: SidebarShortcutTarget): target is SidebarShortcutTarget & {
  locator: { providerId: 'core.app'; resourceId: SidebarAppId }
} {
  return (
    target.locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
    (target.activationId === undefined || target.activationId === 'reveal') &&
    isSidebarAppId(target.locator.resourceId)
  )
}

export function getVisibleSidebarAppIds(values: readonly unknown[] | undefined): SidebarAppId[] {
  return getVisibleSidebarShortcutItems(values).flatMap((item) => {
    const { target } = item
    return isBuiltInAppShortcutTarget(target) ? [target.locator.resourceId] : []
  })
}

export function getSidebarDefaultLandingUrl(
  values: readonly unknown[] | undefined,
  defaultPaintingProvider: string
): string {
  const firstApp = getVisibleSidebarAppIds(values)[0]
  return firstApp ? getSidebarMenuPath(firstApp, defaultPaintingProvider) : ''
}

export function addSidebarShortcut(
  values: readonly unknown[] | undefined,
  target: SidebarShortcutTarget,
  fallbackLabel?: string
): SidebarShortcutItem[] {
  const items = normalizeSidebarShortcutItems(values)
  const id = createSidebarShortcutId(target)
  if (items.some((item) => isSidebarShortcutItem(item) && item.id === id)) return items
  return [
    ...items,
    {
      type: 'shortcut',
      id,
      target,
      ...(fallbackLabel ? { fallbackLabel } : {})
    }
  ]
}

export function removeSidebarShortcut(
  values: readonly unknown[] | undefined,
  target: SidebarShortcutTarget
): SidebarShortcutItem[] {
  const id = createSidebarShortcutId(target)
  const items = normalizeSidebarShortcutItems(values)
  return items.filter((item) => !isSidebarShortcutItem(item) || item.id !== id)
}

export function reorderSidebarShortcuts(
  values: readonly unknown[] | undefined,
  orderedItems: readonly SidebarShortcutItem[]
): SidebarShortcutItem[] {
  const items = normalizeSidebarShortcutItems(values)
  const byId = new Map(items.filter(isSidebarShortcutItem).map((item) => [item.id, item]))
  const queue: SidebarShortcutItem[] = []
  const seen = new Set<string>()
  for (const requested of orderedItems) {
    const item = byId.get(requested.id)
    if (!item || seen.has(item.id)) continue
    seen.add(item.id)
    queue.push(item)
  }
  // Shortcuts the caller did not list stay in their slots. That keeps a
  // personal-center drag from moving team-only items that are off the rail.
  return items.map((item) => {
    if (!isSidebarShortcutItem(item) || !seen.has(item.id)) return item
    return queue.shift() ?? item
  })
}

export function isSidebarShortcutPinned(
  values: readonly unknown[] | undefined,
  target: SidebarShortcutTarget
): boolean {
  const id = createSidebarShortcutId(target)
  return getVisibleSidebarShortcutItems(values).some((item) => item.id === id)
}

// --- Launchpad app order --------------------------------------------------
//
// The launchpad orders its built-in app tiles through its own preference
// (`ui.launchpad.app_order`), completely independent of the sidebar favorites
// order. Mini app tiles are ordered by their global `orderKey` instead, so the
// launchpad never reads or writes `ui.sidebar_shortcut`.

/**
 * The ordered launchpad app ids. Stored order is filtered to valid app ids and
 * deduped; any app missing from storage (e.g. an empty default or a newly added
 * app) is appended in canonical order, so a partial or empty store still yields
 * every app exactly once.
 */
export function getOrderedLaunchpadApps(stored: readonly string[] | undefined): SidebarAppId[] {
  const seen = new Set<SidebarAppId>()
  const ordered: SidebarAppId[] = []

  for (const id of stored ?? []) {
    if (isSidebarAppId(id) && !seen.has(id)) {
      seen.add(id)
      ordered.push(id)
    }
  }
  for (const id of SIDEBAR_FAVORITE_ORDER) {
    if (!seen.has(id)) {
      seen.add(id)
      ordered.push(id)
    }
  }

  return ordered
}

/**
 * Reorder the launchpad app list to `orderedIds` (typically the rendered tile
 * order after a drag). Unknown ids are dropped and any app missing from the
 * requested order is kept at the end so a partial order never loses apps.
 */
export function reorderLaunchpadApps(
  stored: readonly string[] | undefined,
  orderedIds: readonly string[]
): SidebarAppId[] {
  const current = getOrderedLaunchpadApps(stored)
  const currentSet = new Set(current)
  const seen = new Set<SidebarAppId>()
  const next: SidebarAppId[] = []

  for (const id of orderedIds) {
    if (isSidebarAppId(id) && currentSet.has(id) && !seen.has(id)) {
      seen.add(id)
      next.push(id)
    }
  }
  for (const id of current) {
    if (!seen.has(id)) next.push(id)
  }

  return next
}
