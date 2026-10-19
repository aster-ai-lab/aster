import { createContext, use, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { useRightPanelState } from '@renderer/components/chat/panes/Shell'
import type { WorkbenchProof } from '@shared/types/workbenchProof'

import { WorkbenchProjectLockedContext } from './workbenchComposerHost'

export type WorkbenchWorkspaceKind = 'files' | 'browser' | 'terminal' | 'changes' | 'editor' | 'lineage'
export type WorkbenchSurfaceKind = Exclude<WorkbenchWorkspaceKind, 'editor' | 'lineage'>

export interface WorkbenchWorkspaceTab {
  key: string
  kind: WorkbenchWorkspaceKind
  label: string
  /** Absolute path for an editor tab. */
  path?: string
  lineage?: { proof: WorkbenchProof; artifactId: string; review: boolean; requestId: string }
}

export interface WorkbenchTerminalSession {
  key: string
  label: string
}

export interface WorkbenchWorkspaceTabsApi {
  tabs: readonly WorkbenchWorkspaceTab[]
  activeTabKey: string | null
  sessions: readonly WorkbenchTerminalSession[]
  activeSessionKey: string | null
  /** Focus the singleton surface, creating it when it is missing. A terminal focus keeps the current session. */
  openSurface: (kind: WorkbenchSurfaceKind) => void
  openLineage: (proof: WorkbenchProof, artifactId: string, review?: boolean) => void
  syncProof: (proof: WorkbenchProof) => void
  /** Focus the built-in editor for one workspace file, creating the tab when it is missing. */
  openEditor: (path: string, label: string) => void
  /** Focus the terminal surface and add another shell session. */
  newTerminal: () => void
  focusTab: (key: string) => void
  closeTab: (key: string) => number
  /** Closes every tab, every tab except one, or the tabs to its right. Returns how many remain. */
  closeTabGroup: (key: string, mode: 'others' | 'right' | 'all') => number
  focusSession: (key: string) => void
  /** Closes one shell. The last shell also closes the terminal surface. Returns how many workspace tabs remain. */
  closeSession: (key: string) => number
  /** Closes shells the same way as workspace tabs. The last shell closes the terminal surface. */
  closeSessionGroup: (key: string, mode: 'others' | 'right' | 'all') => number
}

const WorkbenchWorkspaceTabsContext = createContext<WorkbenchWorkspaceTabsApi | null>(null)

function createKey(prefix: string): string {
  const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
  return `${prefix}-${id}`
}

function isWorkspaceKind(id: string | undefined): id is WorkbenchSurfaceKind {
  return id === 'files' || id === 'browser' || id === 'terminal' || id === 'changes'
}

/**
 * Tabs for the task-conversation workspace. Files and the browser each keep one
 * tab. Terminal sessions stay here so a hidden tab does not drop its shell.
 */
export function WorkbenchWorkspaceTabsProvider({ children, sessionId }: { children: ReactNode; sessionId?: string }) {
  const { t } = useTranslation()
  const workbench = use(WorkbenchProjectLockedContext)
  const panel = useRightPanelState()
  const filesLabel = t('agent.right_pane.tabs.files', '文件')
  const browserLabel = t('agent.right_pane.tabs.browser', '浏览器')
  const terminalLabel = t('agent.right_pane.tabs.terminal', '终端')
  const changesLabel = t('agent.right_pane.tabs.changes', '变更')
  const [tabs, setTabs] = useState<readonly WorkbenchWorkspaceTab[]>([])
  const [activeTabKey, setActiveTabKey] = useState<string | null>(null)
  const [sessions, setSessions] = useState<readonly WorkbenchTerminalSession[]>([])
  const [activeSessionKey, setActiveSessionKey] = useState<string | null>(null)
  const tabsRef = useRef(tabs)
  const sessionsRef = useRef(sessions)
  const activeTabKeyRef = useRef<string | null>(activeTabKey)
  const syncedKind = useRef<string | null>(null)
  const sessionSeen = useRef(sessionId)
  tabsRef.current = tabs
  sessionsRef.current = sessions
  activeTabKeyRef.current = activeTabKey
  if (sessionSeen.current !== sessionId) {
    sessionSeen.current = sessionId
    syncedKind.current = null
    tabsRef.current = []
    sessionsRef.current = []
    activeTabKeyRef.current = null
    setTabs([])
    setSessions([])
    setActiveTabKey(null)
    setActiveSessionKey(null)
  }

  const labelFor = (kind: WorkbenchSurfaceKind) => {
    if (kind === 'files') return filesLabel
    if (kind === 'browser') return browserLabel
    if (kind === 'changes') return changesLabel
    return terminalLabel
  }

  const focusTabKey = (key: string) => {
    activeTabKeyRef.current = key
    setActiveTabKey(key)
  }

  const ensureSurface = (kind: WorkbenchSurfaceKind, focus = true) => {
    const existing = tabsRef.current.find((tab) => tab.kind === kind)
    if (existing) {
      if (focus) focusTabKey(existing.key)
      return existing.key
    }
    const key = createKey(kind)
    const next = [...tabsRef.current, { key, kind, label: labelFor(kind) }]
    tabsRef.current = next
    setTabs(next)
    if (focus || activeTabKeyRef.current == null) focusTabKey(key)
    return key
  }

  const ensureFirstSession = () => {
    if (sessionsRef.current.length > 0) return
    const key = createKey('session')
    const next = [{ key, label: terminalLabel }]
    sessionsRef.current = next
    setSessions(next)
    setActiveSessionKey(key)
  }

  const openSurface = (kind: WorkbenchSurfaceKind) => {
    ensureSurface(kind)
    if (kind === 'terminal') ensureFirstSession()
  }

  const openLineage = (proof: WorkbenchProof, artifactId: string, review = false) => {
    if (sessionId && proof.sessionId !== sessionId) return
    const artifact = proof.artifacts.find((version) => version.id === artifactId)
    if (!proof.policy.artifactTrace || !artifact) return
    const existing = tabsRef.current.find((tab) => tab.kind === 'lineage')
    const tab: WorkbenchWorkspaceTab = {
      key: existing?.key ?? createKey('lineage'),
      kind: 'lineage',
      label: artifact.name,
      lineage: { proof, artifactId, review, requestId: createKey('lineage-open') }
    }
    const next = existing
      ? tabsRef.current.map((item) => (item.key === tab.key ? tab : item))
      : [...tabsRef.current, tab]
    tabsRef.current = next
    setTabs(next)
    focusTabKey(tab.key)
  }

  const syncProof = (proof: WorkbenchProof) => {
    const current = tabsRef.current
    if (
      !current.some(
        (tab) =>
          tab.lineage?.proof.messageId === proof.messageId &&
          tab.lineage.proof.sessionId === proof.sessionId &&
          tab.lineage.proof !== proof
      )
    )
      return
    const next = current.map((tab) =>
      tab.lineage?.proof.messageId === proof.messageId && tab.lineage.proof.sessionId === proof.sessionId
        ? { ...tab, lineage: { ...tab.lineage, proof } }
        : tab
    )
    tabsRef.current = next
    setTabs(next)
  }

  const openEditor = (path: string, label: string) => {
    const trimmed = path.trim()
    if (!trimmed) return
    const name = label.trim() || trimmed.split(/[/\\]/u).pop() || trimmed
    const existing = tabsRef.current.find((tab) => tab.kind === 'editor' && tab.path === trimmed)
    if (existing) {
      if (existing.label !== name) {
        const next = tabsRef.current.map((tab) => (tab.key === existing.key ? { ...tab, label: name } : tab))
        tabsRef.current = next
        setTabs(next)
      }
      focusTabKey(existing.key)
      return
    }
    const key = createKey('editor')
    const next = [...tabsRef.current, { key, kind: 'editor' as const, label: name, path: trimmed }]
    tabsRef.current = next
    setTabs(next)
    focusTabKey(key)
  }

  const newTerminal = () => {
    ensureSurface('terminal')
    const key = createKey('session')
    const next = [
      ...sessionsRef.current,
      {
        key,
        label: sessionsRef.current.length === 0 ? terminalLabel : `${terminalLabel} ${sessionsRef.current.length + 1}`
      }
    ]
    sessionsRef.current = next
    setSessions(next)
    setActiveSessionKey(key)
  }

  const focusTab = (key: string) => {
    if (!tabsRef.current.some((tab) => tab.key === key)) return
    focusTabKey(key)
  }

  const commitTabs = (next: readonly WorkbenchWorkspaceTab[], activeKey?: string) => {
    const removedTerminal =
      tabsRef.current.some((tab) => tab.kind === 'terminal') && !next.some((tab) => tab.kind === 'terminal')
    tabsRef.current = next
    setTabs(next)
    if (removedTerminal) {
      sessionsRef.current = []
      setSessions([])
      setActiveSessionKey(null)
    }
    setActiveTabKey((current) => {
      if (activeKey && next.some((tab) => tab.key === activeKey)) return activeKey
      return next.some((tab) => tab.key === current) ? current : (next.at(-1)?.key ?? null)
    })
    return next.length
  }

  const closeTab = (key: string) => {
    return commitTabs(tabsRef.current.filter((item) => item.key !== key))
  }

  const closeTabGroup = (key: string, mode: 'others' | 'right' | 'all') => {
    const current = tabsRef.current
    const index = current.findIndex((tab) => tab.key === key)
    if (mode !== 'all' && index < 0) return current.length
    const next =
      mode === 'all' ? [] : mode === 'others' ? current.filter((tab) => tab.key === key) : current.slice(0, index + 1)
    return commitTabs(next, mode === 'others' ? key : undefined)
  }

  const focusSession = (key: string) => {
    if (!sessionsRef.current.some((session) => session.key === key)) return
    setActiveSessionKey(key)
    const terminal = tabsRef.current.find((tab) => tab.kind === 'terminal')
    if (terminal) setActiveTabKey(terminal.key)
  }

  const commitSessions = (next: readonly WorkbenchTerminalSession[], activeKey?: string) => {
    sessionsRef.current = next
    setSessions(next)
    setActiveSessionKey((current) => {
      if (activeKey && next.some((session) => session.key === activeKey)) return activeKey
      return next.some((session) => session.key === current) ? current : (next.at(-1)?.key ?? null)
    })
    if (next.length > 0) return tabsRef.current.length
    const terminal = tabsRef.current.find((tab) => tab.kind === 'terminal')
    if (!terminal) return tabsRef.current.length
    return commitTabs(tabsRef.current.filter((tab) => tab.kind !== 'terminal'))
  }

  const closeSession = (key: string) => {
    return commitSessions(sessionsRef.current.filter((session) => session.key !== key))
  }

  const closeSessionGroup = (key: string, mode: 'others' | 'right' | 'all') => {
    const current = sessionsRef.current
    const index = current.findIndex((session) => session.key === key)
    if (mode !== 'all' && index < 0) return tabsRef.current.length
    const next =
      mode === 'all'
        ? []
        : mode === 'others'
          ? current.filter((session) => session.key === key)
          : current.slice(0, index + 1)
    return commitSessions(next, mode === 'others' ? key : undefined)
  }

  useEffect(() => {
    if (!workbench || !panel.presentationOpen) {
      syncedKind.current = null
      return
    }
    const kind = panel.activePanelId
    if (!isWorkspaceKind(kind) || syncedKind.current === kind) return
    syncedKind.current = kind
    // A document opened in this commit stays the page. The files list stays behind the + menu.
    const activeKey = activeTabKeyRef.current
    const keepPage = tabsRef.current.some(
      (tab) => (tab.kind === 'editor' || tab.kind === 'lineage' || tab.kind === 'changes') && tab.key === activeKey
    )
    if (kind === 'files' && keepPage) return
    ensureSurface(kind)
    if (kind === 'terminal') ensureFirstSession()
    // Labels are read when a tab is created. Re-running on every translation refresh would reopen a tab the user closed.
    // Session changes clear the tabs during render, so this effect has to run again for the new session.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- panel id, open state, and session are the request
  }, [panel.activePanelId, panel.presentationOpen, sessionId, workbench])

  const api = useMemo<WorkbenchWorkspaceTabsApi>(
    () => ({
      tabs,
      activeTabKey,
      sessions,
      activeSessionKey,
      openSurface,
      openLineage,
      syncProof,
      openEditor,
      newTerminal,
      focusTab,
      closeTab,
      closeTabGroup,
      focusSession,
      closeSession,
      closeSessionGroup
    }),
    // The handlers close over the latest refs. Identity follows the visible state.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refs keep the handlers current
    [activeSessionKey, activeTabKey, sessions, tabs]
  )

  return <WorkbenchWorkspaceTabsContext value={api}>{children}</WorkbenchWorkspaceTabsContext>
}

export function useWorkbenchWorkspaceTabs(): WorkbenchWorkspaceTabsApi {
  const value = use(WorkbenchWorkspaceTabsContext)
  if (!value) throw new Error('useWorkbenchWorkspaceTabs must be used within WorkbenchWorkspaceTabsProvider')
  return value
}

export function useOptionalWorkbenchWorkspaceTabs(): WorkbenchWorkspaceTabsApi | null {
  return use(WorkbenchWorkspaceTabsContext)
}
