import { Bot, Database, Files, LayoutPanelLeft, Package, Terminal } from 'lucide-react'
import ChartCandlestick from 'lucide-react/dist/esm/icons/chart-candlestick'
import { type ReactNode, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { DataAnalysisSurface, DataBrowserSurface } from '@renderer/components/data/DataPackageSurface'
import { ToWorkPageLoading } from '@renderer/components/layout/PageEntry'
import { readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { readConnectorConfig } from '@renderer/services/capabilities/connectorConfig'
import { DATA_PACKAGE_ID } from '@renderer/services/data/dataPackageCatalogue'
import { PLUGIN_CONFIG_EVENT, PRESENTON_SLUG, withPresentonHomepage } from '@renderer/services/plugins'
import {
  AixViewSessions,
  aixWorkbenchPackageIdentity,
  encodeAixWorkbenchRouteKey,
  invokeAixComposer,
  readAixWorkbench,
  matchesWorkbenchCondition,
  checkpointAixEditor,
  hasAixEditorRecovery
} from '@renderer/services/plugins'
import type {
  PluginActivationContext,
  PluginContribution,
  PluginRenderProps,
  PluginScopeKind,
  WorkbenchPlugin
} from '@renderer/types/workbenchPlugin'
import {
  AIX_DOCUMENT_ATTEMPT_LIMIT,
  aixDocumentReadySchema,
  AIX_VIEW_THEME_TOKENS,
  type AixViewEnvironment
} from '@shared/types/aixViewBridge'
import type { AixWorkbenchCatalogue, AixWorkbenchPackage, AixWorkbenchView } from '@shared/types/aixWorkbench'
import { STUDIO_CONTRIBUTION_PROTOCOL_VERSION } from '@shared/types/studioContribution'

import { useMemberProfile } from '../account/memberProfile'
import { AixTreeView } from './AixTreeView'
import { AixViewActions } from './AixViewActions'
import { publishInstalledCatalogue } from './ExtensionPointHost'
import { usePluginContributionContext } from './PluginContributionContext'
import { PluginPage } from './PluginPage'
import { usePluginScope } from './PluginScopeProvider'
import { PluginScopeRuntime } from './PluginScopeRuntime'
import { PluginWindowRuntime } from './PluginWindowRuntime'
import { PluginWorkbenchLayoutProvider } from './PluginWorkbenchLayout'

const logger = loggerService.withContext('AixWorkbenchRuntime')
const EMPTY: AixWorkbenchCatalogue = {
  protocolVersion: STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
  revision: '',
  packages: []
}
const INSTALL_EVENT = 'a3s:plugin-install'
const ICONS = {
  bot: Bot,
  database: Database,
  files: Files,
  terminal: Terminal,
  panels: LayoutPanelLeft,
  'chart-candlestick': ChartCandlestick
}

function AixView({
  item,
  view,
  context,
  sessions
}: PluginRenderProps & { item: AixWorkbenchPackage; view: AixWorkbenchView; sessions: AixViewSessions }) {
  const { t, i18n } = useTranslation()
  const [state, setState] = useState<{ url?: string; failed?: boolean; loaded?: boolean }>({})
  const currentSession = useRef<ReturnType<AixViewSessions['open']> | null>(null)
  const [renewal, setRenewal] = useState(0)
  const frame = useRef<globalThis.HTMLIFrameElement>(null)
  const environment = useRef<AixViewEnvironment | null>(null)
  const publishConfig = () => {
    const target = frame.current?.contentWindow
    if (!target) return
    const values = readConnectorConfig(item.slug)
    if (Object.keys(values).length === 0) return
    target.postMessage({ type: 'a3s.plugin.config', values }, '*')
  }
  useEffect(() => {
    const onConfig = (event: Event) => {
      const detail = (event as CustomEvent<{ slug?: string }>).detail
      if (detail?.slug === item.slug) publishConfig()
    }
    globalThis.window.addEventListener(PLUGIN_CONFIG_EVENT, onConfig)
    return () => globalThis.window.removeEventListener(PLUGIN_CONFIG_EVENT, onConfig)
  }, [item.slug])
  useEffect(() => {
    const root = globalThis.document.documentElement
    const update = () => {
      const styles = globalThis.getComputedStyle(root)
      const locale = i18n.resolvedLanguage || i18n.language
      environment.current = {
        theme: root.classList.contains('dark') ? 'dark' : 'light',
        locale,
        direction: i18n.dir(locale),
        tokens: Object.fromEntries(
          AIX_VIEW_THEME_TOKENS.map((name) => {
            const value = styles.getPropertyValue(name).trim()
            return [name, value.length <= 256 ? value : '']
          })
        )
      }
      currentSession.current?.setEnvironment(environment.current)
    }
    update()
    const observer = new globalThis.MutationObserver(update)
    observer.observe(root, { attributes: true, attributeFilter: ['class', 'style'] })
    i18n.on('languageChanged', update)
    return () => {
      observer.disconnect()
      i18n.off('languageChanged', update)
    }
  }, [i18n])
  useEffect(() => {
    let cancelled = false
    let session: ReturnType<AixViewSessions['open']> | undefined
    let unsubscribeTree: (() => void) | undefined
    let unsubscribeCommands: (() => void) | undefined
    let attempts = 0
    const cancel = () => {
      cancelled = true
      currentSession.current = null
      setState({})
      void session?.close().catch((error) => logger.warn('AIX view cleanup refused', error))
    }
    context.signal.addEventListener('abort', cancel, { once: true })
    const connectDocument = (event: globalThis.MessageEvent) => {
      if (event.source !== frame.current?.contentWindow || event.data?.type !== 'aix.document.ready') return
      if (cancelled) {
        for (const port of event.ports) port.close()
        return
      }
      if (++attempts > AIX_DOCUMENT_ATTEMPT_LIMIT) {
        for (const port of event.ports) port.close()
        cancel()
        setState({ failed: true })
        return
      }
      if (event.origin !== 'null' || event.ports.length !== 1) {
        for (const port of event.ports) port.close()
        return
      }
      const parsed = aixDocumentReadySchema.safeParse(event.data)
      if (!parsed.success || !session) {
        for (const port of event.ports) port.close()
        return
      }
      try {
        session.connect(parsed.data.documentId, event.ports[0])
      } catch (error) {
        logger.warn('AIX document connection refused', error instanceof Error ? error : new Error(String(error)))
        cancel()
        setState({ failed: true })
      }
    }
    globalThis.window.addEventListener('message', connectDocument)
    setState({})
    try {
      session = sessions.open(item, view)
      currentSession.current = session
      if (environment.current) session.setEnvironment(environment.current)
      if (session.tree) {
        unsubscribeTree = session.tree.subscribe(() => {
          if (!cancelled && session?.tree?.getSnapshot().status === 'failed') {
            cancel()
            setState({ failed: true })
          }
        })
      }
      if (session.commands)
        unsubscribeCommands = session.commands.subscribe(() => {
          const status = session?.commands?.getSnapshot().status
          if (!cancelled && (status === 'failed' || status === 'closed')) {
            cancel()
            setState({ failed: true })
          }
        })
      void session.ready
        .then((value) => {
          if (!cancelled && context.isCurrent()) setState({ url: value.url })
        })
        .catch((error) => {
          if (cancelled) return
          cancel()
          logger.warn('AIX view unavailable', error)
          setState({ failed: true })
        })
    } catch (error) {
      logger.warn('AIX view admission refused', error instanceof Error ? error : new Error(String(error)))
      setState({ failed: true })
    }
    const renew = globalThis.setTimeout(
      () => {
        if (!cancelled && context.isCurrent() && hasAixEditorRecovery(item.slug, view.id)) {
          void checkpointAixEditor(item.slug, view.id, 'renew')
            .then(() => {
              if (!cancelled && context.isCurrent()) setRenewal((current) => current + 1)
            })
            .catch((error: unknown) =>
              logger.warn(
                'Editor renewal checkpoint refused',
                error instanceof Error ? error : new Error(String(error))
              )
            )
        }
      },
      28 * 60 * 1000
    )
    // The old authority deadline still withdraws a document if renewal failed.
    const expiry = globalThis.setTimeout(
      () => {
        cancel()
        setState({ failed: true })
      },
      30 * 60 * 1000
    )
    return () => {
      globalThis.clearTimeout(expiry)
      globalThis.clearTimeout(renew)
      globalThis.window.removeEventListener('message', connectDocument)
      context.signal.removeEventListener('abort', cancel)
      unsubscribeTree?.()
      unsubscribeCommands?.()
      cancel()
    }
  }, [item, view, context, sessions, renewal])
  if (!context.isCurrent()) return null
  if (!state.url)
    return (
      <div className="relative flex min-h-32 flex-1 flex-col">
        {state.failed ? (
          <div role="status" className="flex flex-1 items-center justify-center p-4 text-sm text-muted-foreground">
            {t('sidebar.resource_missing')}
          </div>
        ) : (
          <ToWorkPageLoading label={t('common.loading')} />
        )}
      </div>
    )
  const content = (
    <div className="relative flex min-h-32 flex-1 flex-col">
      {currentSession.current?.tree && (
        <AixTreeView provider={currentSession.current.tree} title={view.title || view.id} />
      )}
      <iframe
        ref={frame}
        title={view.title || view.id}
        src={state.url}
        onLoad={() => {
          publishConfig()
          const session = currentSession.current
          if (session && !session.documentLoaded()) {
            currentSession.current = null
            setState({ failed: true })
            void session.close().catch((error) => logger.warn('AIX view cleanup refused', error))
            return
          }
          setState((current) => (current.url ? { ...current, loaded: true } : current))
        }}
        sandbox="allow-scripts"
        referrerPolicy="no-referrer"
        allow="camera 'none'; microphone 'none'; geolocation 'none'; clipboard-read 'none'; clipboard-write 'none'"
        hidden={view.presentation === 'tree'}
        aria-hidden={view.presentation === 'tree' ? true : undefined}
        tabIndex={view.presentation === 'tree' ? -1 : undefined}
        className={view.presentation === 'tree' ? 'hidden' : 'min-h-32 w-full flex-1 border-0'}
      />
      {!state.loaded && <ToWorkPageLoading label={t('common.loading')} />}
    </div>
  )
  const session = currentSession.current
  return session?.commands ? (
    <AixViewActions item={item} view={view} context={context} session={session}>
      {content}
    </AixViewActions>
  ) : (
    content
  )
}

export function createAixWorkbenchPlugins(
  catalogue: AixWorkbenchCatalogue,
  cache = new Map<string, WorkbenchPlugin>()
): readonly WorkbenchPlugin[] {
  const identityOf = aixWorkbenchPackageIdentity
  const current = new Set(catalogue.packages.map(identityOf))
  // Executing scopes retain their own factory references until their leases retire.
  for (const identity of cache.keys()) if (!current.has(identity)) cache.delete(identity)
  const plugins = catalogue.packages.map((item) => {
    const identity = identityOf(item)
    const existing = cache.get(identity)
    if (existing) return existing
    if (cache.size >= 1024) {
      logger.error('AIX factory admission refused', {
        slug: item.slug,
        packageId: item.packageId,
        generation: item.generation,
        reason: 'factory-history-limit'
      })
      return null
    }
    const pluginId = `aix.${item.slug}`
    const contributions: PluginContribution[] = []
    const owners = new WeakMap<PluginActivationContext, AixViewSessions>()
    /** A page holds one view whose title is already on its tab or window, so only containers label views. */
    const renderViews = (views: readonly AixWorkbenchView[], titled = true) =>
      function NativeViews({ context }: PluginRenderProps) {
        const visibility = usePluginContributionContext(usePluginScope())
        const sessions = owners.get(context)
        if (!sessions) throw new Error('AIX view scope was not admitted')
        return (
          <div className="flex h-full min-h-0 w-full flex-1 flex-col">
            {views
              .filter((view) => matchesWorkbenchCondition(view.when, visibility))
              .map((view) => (
                <section key={view.id} className="flex min-h-32 flex-1 flex-col border-b border-border">
                  {titled && !view.commands?.length && (
                    <h2 className="px-2 py-1 text-xs text-muted-foreground">{view.title || view.id}</h2>
                  )}
                  <AixView item={item} view={view} context={context} sessions={sessions} />
                </section>
              ))}
          </div>
        )
      }
    const renderPage = (view: AixWorkbenchView, pageId: string) => {
      const Views = renderViews([view], false)
      return function NativePage({ context }: PluginRenderProps) {
        const scope = usePluginScope()
        if (scope?.options.kind === 'view') return <Views context={context} />
        return (
          <PluginScopeRuntime kind="view" plugins={[pluginId]}>
            <PluginPage page={`${pluginId}/${pageId}`} allowBuiltins={false} />
          </PluginScopeRuntime>
        )
      }
    }
    for (const container of item.containers) {
      const containerKey = encodeAixWorkbenchRouteKey({ package: item, kind: 'container', localId: container.id })
      const views = item.views
        .filter((view) => view.container === container.id)
        .sort((a, b) => a.order - b.order || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
      const component = renderViews(views)
      const base = {
        title: container.title,
        labelKey: 'plugins.title',
        order: container.order,
        scopes: ['window'] as const,
        ...(container.when ? { when: container.when } : {})
      }
      if (container.location === 'activitybar') {
        const surface = item.packageId === DATA_PACKAGE_ID ? DataBrowserSurface : component
        contributions.push(
          { ...base, id: `page.${containerKey}`, slot: 'page', component: surface },
          { ...base, id: `sidebar.${containerKey}`, slot: 'sidebar', component: surface },
          {
            ...base,
            id: `activity.container.${containerKey}`,
            slot: 'activity',
            pageId: `page.${containerKey}`,
            sidebarId: `sidebar.${containerKey}`,
            icon: ICONS[container.icon as keyof typeof ICONS] ?? Package
          }
        )
      } else contributions.push({ ...base, id: `panel.${containerKey}`, slot: 'panel', component })
    }
    for (const view of item.views.filter((view) => view.container === null && view.point === 'studio.editor.custom')) {
      const viewKey = encodeAixWorkbenchRouteKey({ package: item, kind: 'view', localId: view.id })
      contributions.push({
        id: `view.${viewKey}`,
        slot: 'page',
        point: 'studio.editor',
        contractVersion: view.contractVersion ?? 1,
        ...(view.mode ? { mode: view.mode } : {}),
        title: view.title || view.id,
        labelKey: 'plugins.title',
        order: view.order,
        scopes: ['window', 'view'],
        ...(view.when ? { when: view.when } : {}),
        component: renderPage(view, `view.${viewKey}`)
      })
    }
    for (const component of item.harness ?? []) {
      contributions.push({
        id: `harness.${component.id}`,
        slot: 'harness',
        point: 'studio.harness.components',
        contractVersion: 1,
        labelKey: 'plugins.title',
        title: component.component,
        scopes: ['window'],
        components: [component.component]
      })
    }
    const taskEditorIds = new Set(
      (item.composerModes ?? [])
        .filter((mode) => (mode as { task?: unknown }).task === true && typeof mode.editor === 'string')
        .map((mode) => mode.editor)
    )
    for (const view of item.views.filter((view) => view.container === null && view.point !== 'studio.editor.custom')) {
      const viewKey = encodeAixWorkbenchRouteKey({ package: item, kind: 'view', localId: view.id })
      const dataWindow = item.packageId === DATA_PACKAGE_ID && view.id === 'analysis' ? DataAnalysisSurface : null
      if (view.point === 'studio.composer.actions') {
        contributions.push({
          id: `composer.${viewKey}`,
          slot: 'composer',
          point: 'studio.composer.actions',
          contractVersion: view.contractVersion ?? 1,
          ...(view.mode ? { mode: view.mode } : {}),
          labelKey: 'plugins.title',
          title: view.title || view.id,
          order: view.order,
          scopes: view.scope ? [view.scope] : ['window', 'workspace'],
          ...(view.when ? { when: view.when } : {}),
          ...(view.scope === 'turn'
            ? { services: ['conversation', 'run', 'turn'] as const }
            : view.scope === 'run'
              ? { services: ['conversation', 'run'] as const }
              : view.scope === 'session'
                ? { services: ['conversation'] as const }
                : {}),
          invoke: (context, _text, draft, signal) => {
            const sessions = owners.get(context)
            if (!sessions) throw new Error('AIX composer scope was not admitted')
            return invokeAixComposer(sessions, item, view, context, draft, signal)
          }
        })
        continue
      }
      const workspace = view.point === 'studio.workspace'
      const base = {
        title: view.title || view.id,
        labelKey: 'plugins.title',
        order: view.order,
        scopes: workspace ? (['workspace'] as const) : (['window', 'view'] as const),
        ...(view.when ? { when: view.when } : {})
      }
      contributions.push({
        ...base,
        id: `view.${viewKey}`,
        slot: workspace ? 'workspace' : 'page',
        point: workspace ? 'studio.workspace' : 'studio.editor',
        contractVersion: view.contractVersion ?? 1,
        ...(view.mode ? { mode: view.mode } : {}),
        ...(workspace ? { services: ['workspace'] as const } : {}),
        component: dataWindow ?? (workspace ? renderViews([view]) : renderPage(view, `view.${viewKey}`))
      })
      // A task editor fills the conversation workspace. It is not a separate window.
      if (!workspace && view.point !== 'studio.editor' && !taskEditorIds.has(view.id))
        contributions.push({
          ...base,
          scopes: ['window'],
          id: `entry.${viewKey}`,
          slot: view.point === 'studio.applets' ? 'applet' : 'activity',
          pageId: `view.${viewKey}`,
          description: view.description,
          icon: ICONS[view.icon as keyof typeof ICONS] ?? Package
        })
      if (taskEditorIds.has(view.id)) {
        const Views = renderViews([view], false)
        contributions.push({
          id: `task-editor.${viewKey}`,
          slot: 'workspace',
          point: 'studio.workspace',
          contractVersion: view.contractVersion ?? 1,
          title: view.title || view.id,
          labelKey: 'plugins.title',
          order: view.order,
          scopes: ['workspace'],
          component: function TaskEditorCanvas({ context }: PluginRenderProps) {
            return (
              <div
                className="flex h-full min-h-0 flex-col [&_iframe]:h-full [&_iframe]:min-h-0 [&_section]:flex [&_section]:h-full [&_section]:min-h-0 [&_section]:flex-1 [&_section]:border-0"
                data-workspace-presentation-canvas="true">
                <Views context={context} />
              </div>
            )
          }
        })
      }
    }
    const scopes: PluginScopeKind[] = ['window', 'view']
    if (
      taskEditorIds.size > 0 ||
      item.views.some((view) => view.point === 'studio.workspace' || view.point === 'studio.composer.actions')
    )
      scopes.push('workspace')
    if (item.views.some((view) => view.scope === 'session' || view.scope === 'run' || view.scope === 'turn'))
      scopes.push('session')
    if (item.views.some((view) => view.scope === 'run' || view.scope === 'turn')) scopes.push('run')
    if (item.views.some((view) => view.scope === 'turn')) scopes.push('turn')
    const plugin: WorkbenchPlugin = {
      id: pluginId,
      generation: item.generation,
      protocolVersion: STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
      scopes,
      contributions,
      activate: (context) => {
        owners.set(context, new AixViewSessions(context, item))
      }
    }
    cache.set(identity, plugin)
    return plugin
  })
  return plugins.filter((plugin): plugin is WorkbenchPlugin => plugin !== null)
}

/** The catalogue the main window publishes. Only an installed package keeps its homepage work mode. */
export function publishedAixCatalogue(admitted: AixWorkbenchCatalogue, packageSlug?: string): AixWorkbenchCatalogue {
  const scoped =
    packageSlug === undefined
      ? admitted
      : { ...admitted, packages: admitted.packages.filter((item) => item.slug === packageSlug) }
  if (packageSlug !== undefined && packageSlug !== PRESENTON_SLUG) return scoped
  return withPresentonHomepage(scoped)
}

export function AixWorkbenchRuntime({
  windowId,
  builtins,
  packageSlug,
  children
}: {
  windowId: string
  builtins: readonly WorkbenchPlugin[]
  /** A mini app window admits only its own package. */
  packageSlug?: string
  children: ReactNode
}) {
  const member = useMemberProfile()
  const center = useSyncExternalStore(subscribeCenter, readCenterId, () => 'personal')
  const namespace = `studio:${member?.id ?? 'signed-out'}:${center}`
  const [installed, setInstalled] = useState<{ namespace: string; catalogue: AixWorkbenchCatalogue }>({
    namespace: '',
    catalogue: EMPTY
  })
  useEffect(() => {
    if (!member?.id) return undefined
    let current: globalThis.AbortController | undefined
    const refresh = () => {
      current?.abort()
      const abort = new globalThis.AbortController()
      current = abort
      void readAixWorkbench(abort.signal)
        .then((catalogue) => {
          if (!abort.signal.aborted) {
            for (const diagnostic of catalogue.diagnostics ?? [])
              logger.error('AIX package contribution refused', diagnostic)
            setInstalled((previous) =>
              previous.namespace === namespace && previous.catalogue.revision === catalogue.revision
                ? previous
                : { namespace, catalogue }
            )
          }
        })
        .catch((error) => {
          if (abort.signal.aborted) return
          setInstalled({ namespace, catalogue: EMPTY })
          logger.warn('AIX catalogue unavailable', error)
        })
    }
    refresh()
    const interval = globalThis.setInterval(refresh, 15000)
    globalThis.window.addEventListener(INSTALL_EVENT, refresh)
    globalThis.window.addEventListener('focus', refresh)
    return () => {
      current?.abort()
      globalThis.clearInterval(interval)
      globalThis.window.removeEventListener(INSTALL_EVENT, refresh)
      globalThis.window.removeEventListener('focus', refresh)
    }
  }, [namespace, member?.id])
  const admitted = installed.namespace === namespace ? installed.catalogue : EMPTY
  const catalogue = useMemo(() => publishedAixCatalogue(admitted, packageSlug), [admitted, packageSlug])
  useEffect(() => {
    publishInstalledCatalogue(catalogue)
  }, [catalogue])
  const factories = useMemo(() => ({ namespace, entries: new Map<string, WorkbenchPlugin>() }), [namespace])
  const plugins = useMemo(
    () => [...builtins, ...createAixWorkbenchPlugins(catalogue, factories.entries)],
    [builtins, catalogue, factories]
  )
  return (
    <PluginWindowRuntime namespace={factories.namespace} windowId={windowId} plugins={plugins}>
      <PluginWorkbenchLayoutProvider key={namespace}>{children}</PluginWorkbenchLayoutProvider>
    </PluginWindowRuntime>
  )
}
