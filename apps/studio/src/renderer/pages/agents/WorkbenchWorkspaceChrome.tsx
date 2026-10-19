import { ChevronsLeft, FolderOpen, Globe, Terminal } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRightPanelActions, useRightPanelState } from '@renderer/components/chat/panes/Shell'
import { RightSidebarExpandIcon } from '@renderer/components/icons/SidebarToggleIcons'
import { useRestingDialog } from '@renderer/components/restingDialog'

import { useOptionalWorkbenchWorkspaceTabs, type WorkbenchSurfaceKind } from './workbenchWorkspaceTabs'

/** Same hit target, radius, and ink as the workspace panel header buttons. */
const panelChromeButtonClass =
  'grid size-7 place-items-center rounded-[8px] text-[#60646c] hover:bg-black/[0.05] hover:text-[#171717]'

const RAIL_TABS = [
  { id: 'browser', labelKey: 'agent.right_pane.tabs.browser', fallback: '浏览器', icon: Globe },
  { id: 'terminal', labelKey: 'agent.right_pane.tabs.terminal', fallback: '终端', icon: Terminal },
  { id: 'files', labelKey: 'agent.right_pane.tabs.files', fallback: '文件', icon: FolderOpen }
] as const

/**
 * Task conversation chrome from Work: a workspace-open button at the page's
 * top end, and the vertical shortcut rail beneath it. Both leave once the
 * workspace pane is open.
 */
export function WorkbenchWorkspaceChrome() {
  const { t } = useTranslation()
  const state = useRightPanelState()
  const actions = useRightPanelActions()
  const workspaceTabs = useOptionalWorkbenchWorkspaceTabs()
  const rootRef = useRef<HTMLDivElement>(null)
  const menuId = useId()
  const [expanded, setExpanded] = useState(false)
  const menuPresence = useRestingDialog(expanded)
  const tabs = RAIL_TABS

  useEffect(() => {
    if (!expanded) return
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node) || rootRef.current?.contains(target)) return
      setExpanded(false)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setExpanded(false)
    }
    document.addEventListener('pointerdown', onPointerDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [expanded])

  useEffect(() => {
    if (state.presentationOpen) setExpanded(false)
  }, [state.presentationOpen])

  if (state.presentationOpen || state.presentationMaximized) return null

  const openKind = (id: WorkbenchSurfaceKind) => {
    workspaceTabs?.openSurface(id)
    actions.tryOpen(id, { userInitiated: true })
    setExpanded(false)
  }

  const openWorkspace = () => {
    if (actions.canOpen('files')) {
      openKind('files')
      return
    }
    if (actions.canOpen('browser')) openKind('browser')
  }

  const openTab = (id: WorkbenchSurfaceKind) => {
    openKind(id)
  }

  return (
    <div ref={rootRef} className="pointer-events-none absolute inset-0" data-workbench-workspace-chrome="true">
      <button
        type="button"
        data-agent-detail-action="open"
        aria-label={t('workbench.workspace.open', '打开任务工作区')}
        title={t('workbench.workspace.open', '打开任务工作区')}
        className={`pointer-events-auto absolute top-3 right-3 ${panelChromeButtonClass}`}
        onClick={openWorkspace}>
        <RightSidebarExpandIcon className="size-[15px]" />
      </button>
      <div
        className="pointer-events-auto absolute top-12 right-3 flex flex-col items-end gap-2"
        data-workspace-shortcut-float="true"
        data-workspace-shortcut-placement="float"
        data-expanded={expanded ? 'true' : 'false'}>
        {menuPresence.present ? (
          <div
            id={menuId}
            role="dialog"
            aria-label={t('workbench.workspace.menu', '工作区快捷菜单')}
            data-workspace-shortcut-menu="true"
            data-state={menuPresence.state}
            className={`a3s-popup-rise w-56 rounded-[12px] border border-[#ececec] bg-white p-2 text-[#171717] shadow-[0_1px_2px_rgb(0_0_0/0.05),0_8px_18px_rgb(0_0_0/0.08)]${
              menuPresence.state === 'closed' ? ' pointer-events-none' : ''
            }`}>
            <section aria-label={t('workbench.workspace.open_tabs', '打开的标签')}>
              <h2 className="px-2 pb-1 text-[11px] font-medium text-[#8a8a8a]">
                {t('workbench.workspace.open_tabs', '打开的标签')}
              </h2>
              {workspaceTabs && workspaceTabs.tabs.length > 0 ? (
                <ul className="grid gap-0.5">
                  {workspaceTabs.tabs.map((tab) => (
                    <li key={tab.key}>
                      <button
                        type="button"
                        data-workspace-open-tab={tab.kind}
                        className="flex w-full items-center rounded-[8px] px-2 py-1.5 text-left text-[13px] hover:bg-black/[0.05]"
                        onClick={() => {
                          if (tab.kind === 'editor' || tab.kind === 'lineage') {
                            workspaceTabs?.focusTab(tab.key)
                            actions.tryOpen('files', { userInitiated: true })
                            setExpanded(false)
                            return
                          }
                          openKind(tab.kind)
                        }}>
                        {tab.label}
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p data-workspace-shortcut-empty="open-tabs" className="px-2 pb-1 text-[12px] text-[#8a8a8a]">
                  {t('workbench.workspace.open_tabs_empty', '暂无打开的标签')}
                </p>
              )}
            </section>
            <section
              aria-label={t('workbench.workspace.on_workspace', '工作区上')}
              className="mt-2 border-t border-[#ececec] pt-2">
              <h2 className="px-2 pb-1 text-[11px] font-medium text-[#8a8a8a]">
                {t('workbench.workspace.on_workspace', '工作区上')}
              </h2>
              <ul className="grid gap-0.5">
                {tabs.map((tab) => {
                  const Icon = tab.icon
                  const label = t(tab.labelKey, tab.fallback)
                  return (
                    <li key={tab.id}>
                      <button
                        type="button"
                        data-workspace-shortcut-item={tab.id}
                        className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left text-[13px] hover:bg-black/[0.05]"
                        onClick={() => openTab(tab.id)}>
                        <Icon size={15} strokeWidth={1.7} className="text-[#60646c]" />
                        <span>{label}</span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            </section>
            <div className="mt-2 border-t border-[#ececec] pt-1">
              <button
                type="button"
                data-workspace-shortcut-item="collapse"
                aria-label={t('workbench.workspace.collapse', '折叠')}
                className="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left text-[13px] hover:bg-black/[0.05]"
                onClick={() => setExpanded(false)}>
                <ChevronsLeft size={15} strokeWidth={1.7} className="scale-x-[-1] text-[#60646c]" />
                <span>{t('workbench.workspace.collapse', '折叠')}</span>
              </button>
            </div>
          </div>
        ) : (
          <nav
            aria-label={t('workbench.workspace.shortcuts', '工作区快捷栏')}
            data-workspace-shortcut-rail="true"
            className="flex flex-col items-center gap-0.5 rounded-[10px] border border-[#ececec] bg-white p-1 shadow-[0_1px_2px_rgb(0_0_0/0.05),0_4px_10px_rgb(0_0_0/0.06)]">
            {tabs.map((tab) => {
              const Icon = tab.icon
              const label = t(tab.labelKey, tab.fallback)
              return (
                <button
                  key={tab.id}
                  type="button"
                  aria-label={label}
                  data-workspace-shortcut-rail-item={tab.id}
                  className={panelChromeButtonClass}
                  onClick={() => openTab(tab.id)}>
                  <Icon size={15} strokeWidth={1.7} />
                </button>
              )
            })}
            <button
              type="button"
              aria-controls={menuId}
              aria-expanded={false}
              aria-label={t('workbench.workspace.expand', '展开工作区快捷菜单')}
              data-workspace-shortcut-rail-item="expand"
              className={panelChromeButtonClass}
              onClick={() => setExpanded(true)}>
              <ChevronsLeft size={15} strokeWidth={1.7} />
            </button>
          </nav>
        )}
      </div>
    </div>
  )
}
