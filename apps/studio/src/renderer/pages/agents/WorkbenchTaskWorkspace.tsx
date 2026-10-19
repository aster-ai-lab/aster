import { Button } from '@a3s-lab/ui'
import { FolderOpen, GitCompare, Globe, Link2, Plus, Presentation, Terminal, X } from 'lucide-react'
import { use, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { RightPanelHeaderControls, useRightPanelActions } from '@renderer/components/chat/panes/Shell'
import { DocumentFileIcon, documentFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import { usePluginContributions } from '@renderer/components/plugins/PluginScopeProvider'
import { PluginScopeRuntime } from '@renderer/components/plugins/PluginScopeRuntime'
import { PluginSlot } from '@renderer/components/plugins/PluginSlot'
import { cn } from '@renderer/utils/style'

import { CodeChangesPanel } from './CodeChangesPanel'
import { readCodeSession } from './codeMode'
import { AgentWorkspaceTerminal } from './components/AgentRightPane'
import { PAPERLOOM_OPEN, type PaperloomOpenRequest } from './paperloom/paperloomApi'
import { PaperloomPanel } from './paperloom/PaperloomPanel'
import { PresentationSessionDeck, usePresentationDeck } from './presentationDeck'
import { WorkbenchHomeActiveContext } from './workbenchComposerHost'
import { WorkbenchFileEditor } from './WorkbenchFileEditor'
import { WorkbenchLineagePane } from './WorkbenchLineagePane'
import { WorkbenchTabContextMenu, useWorkbenchTabContextMenu } from './workbenchTabContextMenu'
import {
  useWorkbenchWorkspaceTabs,
  type WorkbenchWorkspaceKind,
  type WorkbenchWorkspaceTab
} from './workbenchWorkspaceTabs'

const NEW_TAB_ITEMS: readonly {
  kind: Exclude<WorkbenchWorkspaceKind, 'editor' | 'lineage'>
  labelKey: string
  fallback: string
}[] = [
  { kind: 'files', labelKey: 'agent.right_pane.tabs.files', fallback: '文件' },
  { kind: 'changes', labelKey: 'agent.right_pane.tabs.changes', fallback: '变更' },
  { kind: 'terminal', labelKey: 'agent.right_pane.tabs.terminal', fallback: '终端' },
  { kind: 'browser', labelKey: 'agent.right_pane.tabs.browser', fallback: '浏览器' }
]

function TabIcon({ kind, label }: { kind: WorkbenchWorkspaceKind; label?: string }) {
  if (kind === 'editor') {
    return <DocumentFileIcon type={documentFileIconType(label ?? '')} className="h-4 w-3.5" />
  }
  const props = { size: 14, strokeWidth: 1.7, className: 'shrink-0 text-[#60646c]' }
  if (kind === 'files') return <FolderOpen {...props} />
  if (kind === 'browser') return <Globe {...props} />
  if (kind === 'changes') return <GitCompare {...props} />
  if (kind === 'lineage') return <Link2 {...props} />
  return <Terminal {...props} />
}

/**
 * Task workspace for the conversation: one tab strip for files, browser, and
 * terminal, and a second strip inside the terminal for each live shell.
 */
interface TaskWorkspaceProps {
  agentSessionId?: string
  workspaceId?: string
  cwd?: string
  renderBrowser: (active: boolean) => ReactNode
  renderChanges?: () => ReactNode
  renderFiles: (active: boolean) => ReactNode
}

export function WorkbenchTaskWorkspace(props: TaskWorkspaceProps) {
  const workspace = props.workspaceId && props.cwd ? { id: props.workspaceId, root: props.cwd } : undefined
  const conversation = props.agentSessionId ? { sessionId: props.agentSessionId } : undefined
  return (
    <PluginScopeRuntime kind="workspace" services={{ workspace, conversation }}>
      <WorkbenchTaskWorkspaceContent {...props} />
    </PluginScopeRuntime>
  )
}

function WorkbenchTaskWorkspaceContent({
  agentSessionId,
  cwd,
  renderBrowser,
  renderChanges,
  renderFiles
}: TaskWorkspaceProps) {
  const { t } = useTranslation()
  const homeActive = use(WorkbenchHomeActiveContext)
  const actions = useRightPanelActions()
  const api = useWorkbenchWorkspaceTabs()
  const menuId = useId()
  const hostRef = useRef<HTMLDivElement>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  const pluginPanels = usePluginContributions('workspace')
  const [pluginTabKey, setPluginTabKey] = useState<string | null>(null)
  const [paperloomState, setPaperloomRequest] = useState<{
    sessionId: string
    root: string
    request: PaperloomOpenRequest
  } | null>(null)
  const paperloomRequest =
    paperloomState && paperloomState.sessionId === agentSessionId && paperloomState.root === (cwd ?? '')
      ? paperloomState.request
      : null
  const [paperloomVisible, setShowingPaperloom] = useState(false)
  const showingPaperloom = paperloomRequest !== null && paperloomVisible
  useEffect(() => {
    setPaperloomRequest(null)
    setShowingPaperloom(false)
  }, [agentSessionId, cwd])
  const selectedPlugin = pluginPanels.find((item) => item.key === pluginTabKey)
  const deck = usePresentationDeck()
  const presentation = deck?.sessionId === agentSessionId ? deck : null
  const canvas = pluginPanels.find((item) => item.contribution.id.startsWith('task-editor.'))
  const presentationKey = presentation ? `${presentation.sessionId}:${presentation.topic}` : null
  const [hiddenPresentation, setHiddenPresentation] = useState<string | null>(null)
  const showingPresentation = presentationKey !== null && hiddenPresentation !== presentationKey
  const hidePresentation = () => {
    if (presentationKey) setHiddenPresentation(presentationKey)
  }
  useEffect(() => {
    const open = (event: Event) => {
      if (homeActive || !agentSessionId || event.defaultPrevented || !actions.canOpen('files')) return
      event.preventDefault()
      const request = (event as CustomEvent<PaperloomOpenRequest>).detail
      setPaperloomRequest({
        sessionId: agentSessionId,
        root: cwd ?? '',
        request: { ...request, source: { root: cwd ?? '', paths: request.source?.paths ?? [] } }
      })
      setShowingPaperloom(true)
      setPluginTabKey(null)
      if (presentationKey) setHiddenPresentation(presentationKey)
      actions.tryOpen('files', { userInitiated: true })
    }
    window.addEventListener(PAPERLOOM_OPEN, open)
    return () => window.removeEventListener(PAPERLOOM_OPEN, open)
  }, [actions, agentSessionId, cwd, homeActive, presentationKey])
  const activeTab = api.tabs.find((tab) => tab.key === api.activeTabKey) ?? api.tabs[0]
  useEffect(() => {
    if (activeTab?.kind === 'lineage') {
      setShowingPaperloom(false)
      setPluginTabKey(null)
      if (presentationKey) setHiddenPresentation(presentationKey)
    }
  }, [activeTab?.key, activeTab?.kind, activeTab?.lineage?.requestId, presentationKey])
  const tabKeys = api.tabs.map((tab) => tab.key)
  const tabContext = useWorkbenchTabContextMenu(tabKeys)
  const contextTab = tabContext.menu ? api.tabs.find((tab) => tab.key === tabContext.menu?.tabKey) : undefined
  const contextTabIndex = contextTab ? api.tabs.findIndex((tab) => tab.key === contextTab.key) : -1

  const runTabContextAction = (action: 'close' | 'close-others' | 'close-right' | 'close-all') => {
    if (!contextTab) return
    tabContext.dismiss()
    const left =
      action === 'close'
        ? api.closeTab(contextTab.key)
        : api.closeTabGroup(
            contextTab.key,
            action === 'close-others' ? 'others' : action === 'close-right' ? 'right' : 'all'
          )
    // Close-all leaves the empty launcher. Closing the last tab by itself still dismisses the pane.
    if (left === 0 && action !== 'close-all' && !showingPresentation && paperloomRequest === null) actions.close()
  }

  useEffect(() => {
    if (!menuOpen) return
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node) || hostRef.current?.contains(target)) return
      setMenuOpen(false)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenuOpen(false)
    }
    document.addEventListener('pointerdown', onPointerDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [menuOpen])

  const openedChanges = useRef<string | null>(null)
  useEffect(() => {
    if (!agentSessionId || openedChanges.current === agentSessionId || !readCodeSession(agentSessionId)) return
    openedChanges.current = agentSessionId
    api.openSurface('changes')
  }, [agentSessionId, api])

  const showKind = (kind: Exclude<WorkbenchWorkspaceKind, 'editor' | 'lineage'>) => {
    setShowingPaperloom(false)
    setPluginTabKey(null)
    hidePresentation()
    if (kind === 'terminal') api.newTerminal()
    else api.openSurface(kind)
    if (kind !== 'changes') actions.tryOpen(kind, { userInitiated: true })
    setMenuOpen(false)
  }

  const selectTab = (tab: WorkbenchWorkspaceTab) => {
    setShowingPaperloom(false)
    setPluginTabKey(null)
    hidePresentation()
    api.focusTab(tab.key)
    if (tab.kind !== 'editor') actions.tryOpen(tab.kind === 'lineage' ? 'files' : tab.kind, { userInitiated: true })
    setMenuOpen(false)
  }

  const closeWorkspaceTab = (key: string) => {
    const left = api.closeTab(key)
    if (left === 0 && !showingPresentation && paperloomRequest === null) actions.close()
  }

  return (
    <section
      ref={hostRef}
      aria-label={t('workbench.workspace.tablist', '任务工作区')}
      data-workspace-multitab="true"
      data-workspace-new-tab-open={menuOpen ? 'true' : undefined}
      className="flex h-full min-h-0 flex-col bg-white text-[#171717]">
      <header className="relative z-30 flex h-10 shrink-0 items-center gap-1 border-b border-[#ececec] bg-white ps-1 pe-1">
        <div className="relative shrink-0">
          <button
            type="button"
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-controls={menuId}
            aria-label={t('workbench.workspace.new_tab', '新建工作区标签')}
            title={t('workbench.workspace.new_tab', '新建工作区标签')}
            data-workspace-chrome-action="new-tab"
            className="grid size-7 place-items-center rounded-[8px] text-[#60646c] hover:bg-black/[0.05] hover:text-[#171717]"
            onClick={() => setMenuOpen((open) => !open)}>
            <Plus size={15} strokeWidth={1.7} />
          </button>
          {menuOpen ? (
            <div
              id={menuId}
              role="menu"
              aria-label={t('workbench.workspace.new_tab', '新建工作区标签')}
              data-workspace-new-tab-menu="true"
              className="absolute top-8 left-0 z-10 w-40 rounded-[10px] border border-[#ececec] bg-white p-1 shadow-[0_8px_18px_rgb(0_0_0/0.08)]">
              {NEW_TAB_ITEMS.map((item) => (
                <button
                  key={item.kind}
                  type="button"
                  role="menuitem"
                  data-workspace-new-tab-item={item.kind}
                  className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left text-[13px] hover:bg-black/[0.05]"
                  onClick={() => showKind(item.kind)}>
                  <TabIcon kind={item.kind} />
                  <span>{t(item.labelKey, item.fallback)}</span>
                </button>
              ))}
              {pluginPanels.map((item) => {
                const Icon = item.contribution.id.startsWith('task-editor.') ? Presentation : null
                return (
                  <button
                    key={item.key}
                    type="button"
                    role="menuitem"
                    className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left text-[13px] hover:bg-black/[0.05]"
                    onClick={() => {
                      setShowingPaperloom(false)
                      setPluginTabKey(item.key)
                      hidePresentation()
                      setMenuOpen(false)
                    }}>
                    {Icon ? <Icon size={14} strokeWidth={1.7} className="shrink-0 text-[#60646c]" /> : null}
                    <span>{item.contribution.title ?? t(item.contribution.labelKey)}</span>
                  </button>
                )
              })}
            </div>
          ) : null}
        </div>
        <div
          role="tablist"
          aria-label={t('workbench.workspace.tablist', '任务工作区')}
          className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto">
          {paperloomRequest !== null && (
            <div className="flex shrink-0 items-center gap-1">
              <Button
                size="sm"
                variant={showingPaperloom ? 'secondary' : 'ghost'}
                role="tab"
                aria-selected={showingPaperloom}
                onClick={() => {
                  setShowingPaperloom(true)
                  setPluginTabKey(null)
                  hidePresentation()
                }}>
                {t('paperloom.title')}
              </Button>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={t('paperloom.close_workspace')}
                onClick={() => {
                  setPaperloomRequest(null)
                  setShowingPaperloom(false)
                }}>
                <X size={12} />
              </Button>
            </div>
          )}
          {presentation ? (
            <div
              data-active={showingPresentation ? 'true' : undefined}
              className={cn(
                'group/tab relative flex h-7 max-w-40 shrink-0 items-center rounded-[8px]',
                showingPresentation ? 'bg-[#f3f3f3] text-[#171717]' : 'text-[#60646c] hover:bg-black/[0.04]'
              )}>
              <button
                type="button"
                role="tab"
                aria-selected={showingPresentation}
                data-workspace-tab="presentation"
                className="flex h-7 min-w-0 items-center gap-1.5 rounded-[8px] ps-2 pe-2 text-[13px] group-focus-within/tab:pe-5 group-hover/tab:pe-5"
                onClick={() => {
                  setShowingPaperloom(false)
                  setHiddenPresentation(null)
                }}>
                <Presentation size={14} strokeWidth={1.7} className="shrink-0 text-[#60646c]" />
                <span className="truncate">{presentation.title}</span>
              </button>
              <button
                type="button"
                aria-label={`${t('workbench.workspace.close_tab', '关闭标签')} ${presentation.title}`}
                data-workspace-tab-close="presentation"
                className="pointer-events-none absolute end-0.5 grid size-4 place-items-center rounded-[5px] text-current opacity-0 transition-opacity group-focus-within/tab:pointer-events-auto group-focus-within/tab:opacity-100 group-hover/tab:pointer-events-auto group-hover/tab:opacity-100 hover:bg-black/[0.08]"
                onClick={() => hidePresentation()}>
                <X size={12} strokeWidth={1.7} />
              </button>
            </div>
          ) : null}
          {api.tabs.map((tab) => {
            const selected = !showingPaperloom && !showingPresentation && !selectedPlugin && tab.key === activeTab?.key
            return (
              <div
                key={tab.key}
                data-active={selected ? 'true' : undefined}
                className={cn(
                  'group/tab relative flex h-7 max-w-40 shrink-0 items-center rounded-[8px]',
                  selected ? 'bg-[#f3f3f3] text-[#171717]' : 'text-[#60646c] hover:bg-black/[0.04]'
                )}
                onContextMenu={(event) => tabContext.openFromContextMenu(tab.key, event)}>
                <button
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  aria-haspopup="menu"
                  aria-controls={tabContext.menu?.tabKey === tab.key ? tabContext.menuId : undefined}
                  data-workspace-tab={tab.kind}
                  data-workspace-tab-key={tab.key}
                  data-workspace-editor-path={tab.kind === 'editor' ? tab.path : undefined}
                  className="flex h-7 min-w-0 items-center gap-1.5 rounded-[8px] ps-2 pe-2 text-[13px] group-focus-within/tab:pe-5 group-hover/tab:pe-5"
                  onClick={() => selectTab(tab)}
                  onKeyDown={(event) => {
                    tabContext.openFromKeyboard(tab.key, event)
                  }}>
                  <TabIcon kind={tab.kind} label={tab.label} />
                  <span className="truncate">{tab.label}</span>
                </button>
                <button
                  type="button"
                  aria-label={`${t('workbench.workspace.close_tab', '关闭标签')} ${tab.label}`}
                  data-workspace-tab-close={tab.kind}
                  data-workspace-tab-close-key={tab.key}
                  className="pointer-events-none absolute end-0.5 grid size-4 place-items-center rounded-[5px] text-current opacity-0 transition-opacity group-focus-within/tab:pointer-events-auto group-focus-within/tab:opacity-100 group-hover/tab:pointer-events-auto group-hover/tab:opacity-100 hover:bg-black/[0.08]"
                  onClick={() => closeWorkspaceTab(tab.key)}>
                  <X size={12} strokeWidth={1.7} />
                </button>
              </div>
            )
          })}
          {selectedPlugin ? (
            <div className="flex shrink-0 items-center gap-1">
              <Button role="tab" aria-selected={true} variant="secondary" size="sm">
                {selectedPlugin.contribution.title ?? t(selectedPlugin.contribution.labelKey)}
              </Button>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={`${t('workbench.workspace.close_tab')} ${selectedPlugin.contribution.title ?? t(selectedPlugin.contribution.labelKey)}`}
                onClick={() => setPluginTabKey(null)}>
                <X size={12} />
              </Button>
            </div>
          ) : null}
        </div>
        {tabContext.menu && contextTab ? (
          <WorkbenchTabContextMenu
            canCloseOthers={api.tabs.length > 1}
            canCloseRight={contextTabIndex >= 0 && contextTabIndex < api.tabs.length - 1}
            id={tabContext.menuId}
            label={contextTab.label}
            menuRef={tabContext.menuRef}
            onAction={runTabContextAction}
            onKeyDown={tabContext.onMenuKeyDown}
            x={tabContext.menu.x}
            y={tabContext.menu.y}
          />
        ) : null}
        <RightPanelHeaderControls canMaximize />
      </header>
      <div
        className="relative min-h-0 flex-1"
        data-workspace-tab-body={
          showingPaperloom
            ? 'paperloom'
            : showingPresentation
              ? 'presentation'
              : selectedPlugin
                ? 'plugin'
                : activeTab?.kind
        }>
        {paperloomRequest !== null && (
          <div hidden={!showingPaperloom} className="h-full min-h-0">
            <PaperloomPanel
              key={paperloomRequest.requestId}
              {...paperloomRequest}
              closeLabel={t('paperloom.close_workspace')}
              onClose={() => {
                setPaperloomRequest(null)
                setShowingPaperloom(false)
              }}
            />
          </div>
        )}
        <div hidden={showingPaperloom} className="h-full min-h-0">
          {showingPresentation && agentSessionId ? (
            canvas ? (
              <PluginSlot slot="workspace" contributionKey={canvas.key} />
            ) : (
              <PresentationSessionDeck sessionId={agentSessionId} />
            )
          ) : api.tabs.length === 0 && !selectedPlugin ? (
            <div data-workspace-launcher="true" className="grid h-full place-items-center">
              <div className="grid grid-cols-3 gap-2">
                {NEW_TAB_ITEMS.map((item) => (
                  <button
                    key={item.kind}
                    type="button"
                    data-workspace-launcher-item={item.kind}
                    className="flex w-24 flex-col items-center gap-2 rounded-[12px] border border-[#ececec] px-3 py-4 text-[13px] text-[#171717] hover:bg-black/[0.04]"
                    onClick={() => showKind(item.kind)}>
                    <TabIcon kind={item.kind} />
                    <span>{t(item.labelKey, item.fallback)}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            api.tabs.map((tab) => {
              const active = !showingPaperloom && !showingPresentation && !selectedPlugin && tab.key === activeTab?.key
              return (
                <div
                  key={tab.key}
                  hidden={!active}
                  data-workspace-pane={tab.kind}
                  data-workspace-pane-active={active ? 'true' : undefined}
                  className="h-full min-h-0 overflow-hidden">
                  {tab.kind === 'files' ? renderFiles(active) : null}
                  {tab.kind === 'changes' ? (
                    renderChanges ? (
                      renderChanges()
                    ) : (
                      <CodeChangesPanel sessionId={agentSessionId} diffs={[]} />
                    )
                  ) : null}
                  {tab.kind === 'editor' && tab.path ? <WorkbenchFileEditor root={cwd ?? ''} path={tab.path} /> : null}
                  {tab.kind === 'lineage' && tab.lineage ? <WorkbenchLineagePane lineage={tab.lineage} /> : null}
                  {tab.kind === 'browser' ? renderBrowser(active) : null}
                  {tab.kind === 'terminal' ? (
                    <WorkbenchTerminalDock active={active} agentSessionId={agentSessionId} cwd={cwd} />
                  ) : null}
                </div>
              )
            })
          )}
          {selectedPlugin && !showingPresentation ? (
            <PluginSlot slot="workspace" contributionKey={selectedPlugin.key} />
          ) : null}
        </div>
      </div>
    </section>
  )
}

function WorkbenchTerminalDock({
  active,
  agentSessionId,
  cwd
}: {
  active: boolean
  agentSessionId?: string
  cwd?: string
}) {
  const { t } = useTranslation()
  const actions = useRightPanelActions()
  const api = useWorkbenchWorkspaceTabs()
  const sessionKeys = api.sessions.map((session) => session.key)
  const tabContext = useWorkbenchTabContextMenu(sessionKeys)
  const contextSession = tabContext.menu
    ? api.sessions.find((session) => session.key === tabContext.menu?.tabKey)
    : undefined
  const contextIndex = contextSession ? api.sessions.findIndex((session) => session.key === contextSession.key) : -1

  const runTabContextAction = (action: 'close' | 'close-others' | 'close-right' | 'close-all') => {
    if (!contextSession) return
    tabContext.dismiss()
    const left =
      action === 'close'
        ? api.closeSession(contextSession.key)
        : api.closeSessionGroup(
            contextSession.key,
            action === 'close-others' ? 'others' : action === 'close-right' ? 'right' : 'all'
          )
    if (left === 0) actions.close()
  }

  return (
    <div className="flex h-full min-h-0 flex-col" data-workspace-terminal-dock="true">
      <div
        role="tablist"
        aria-label={t('workbench.workspace.terminal_tabs', '终端标签')}
        data-conversation-terminal-tabs="true"
        className="flex h-9 shrink-0 items-center gap-0.5 border-b border-[#ececec] bg-[#fafafa] px-1">
        {api.sessions.map((session) => {
          const selected = session.key === api.activeSessionKey
          return (
            <div
              key={session.key}
              data-conversation-terminal-tab={session.key}
              className={cn(
                'group/tab relative flex h-7 shrink-0 items-center rounded-[8px]',
                selected
                  ? 'bg-white text-[#171717] shadow-[0_1px_2px_rgb(0_0_0/0.04)]'
                  : 'text-[#60646c] hover:bg-black/[0.04]'
              )}
              onContextMenu={(event) => tabContext.openFromContextMenu(session.key, event)}>
              <button
                type="button"
                role="tab"
                aria-selected={selected}
                aria-haspopup="menu"
                aria-controls={tabContext.menu?.tabKey === session.key ? tabContext.menuId : undefined}
                data-conversation-terminal-tab-button={session.key}
                className="flex h-7 items-center gap-1.5 rounded-[8px] ps-2 pe-2 text-[13px] group-focus-within/tab:pe-5 group-hover/tab:pe-5"
                onClick={() => api.focusSession(session.key)}
                onKeyDown={(event) => {
                  tabContext.openFromKeyboard(session.key, event)
                }}>
                <Terminal size={13} strokeWidth={1.7} />
                <span>{session.label}</span>
              </button>
              <button
                type="button"
                aria-label={`${t('workbench.workspace.close_terminal_tab', '关闭终端标签')} ${session.label}`}
                data-conversation-terminal-tab-close={session.key}
                className="pointer-events-none absolute end-0.5 grid size-4 place-items-center rounded-[5px] text-current opacity-0 transition-opacity group-focus-within/tab:pointer-events-auto group-focus-within/tab:opacity-100 group-hover/tab:pointer-events-auto group-hover/tab:opacity-100 hover:bg-black/[0.08]"
                onClick={() => {
                  const left = api.closeSession(session.key)
                  if (left === 0) actions.close()
                }}>
                <X size={12} strokeWidth={1.7} />
              </button>
            </div>
          )
        })}
        <button
          type="button"
          aria-label={t('workbench.workspace.new_terminal', '新建终端')}
          title={t('workbench.workspace.new_terminal', '新建终端')}
          data-conversation-terminal-action="new-tab"
          className="grid size-7 place-items-center rounded-[8px] text-[#60646c] hover:bg-black/[0.05]"
          onClick={() => api.newTerminal()}>
          <Plus size={14} strokeWidth={1.7} />
        </button>
        {tabContext.menu && contextSession ? (
          <WorkbenchTabContextMenu
            canCloseOthers={api.sessions.length > 1}
            canCloseRight={contextIndex >= 0 && contextIndex < api.sessions.length - 1}
            id={tabContext.menuId}
            label={contextSession.label}
            menuRef={tabContext.menuRef}
            onAction={runTabContextAction}
            onKeyDown={tabContext.onMenuKeyDown}
            x={tabContext.menu.x}
            y={tabContext.menu.y}
          />
        ) : null}
      </div>
      <div className="relative min-h-0 flex-1 bg-[#1e1e1e]">
        {api.sessions.map((session) => {
          const selected = active && session.key === api.activeSessionKey
          return (
            <div
              key={session.key}
              hidden={!selected}
              role="tabpanel"
              data-conversation-terminal-pane={session.key}
              data-conversation-terminal-pane-active={selected ? 'true' : undefined}
              className="h-full min-h-0">
              <AgentWorkspaceTerminal active={selected} cwd={cwd} label={session.label} sessionId={agentSessionId} />
            </div>
          )
        })}
      </div>
    </div>
  )
}
