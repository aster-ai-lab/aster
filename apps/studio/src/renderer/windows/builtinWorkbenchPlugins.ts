import { type ComponentType, createElement, lazy } from 'react'

import { SIDEBAR_ICON_COMPONENTS } from '@renderer/components/app/sidebarIcons'
import { getSidebarIconLabelKey } from '@renderer/i18n/label'
import type { PluginRenderProps, WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { SIDEBAR_APPS, type SidebarAppId } from '@renderer/utils/sidebar'

const PAGES = {
  agents: lazy(() => import('@renderer/pages/agents/AgentPage')),
  assistants: lazy(() => import('@renderer/pages/home/HomePage')),
  messages: lazy(() => import('@renderer/pages/messages/MessagesPage')),
  mail: lazy(() =>
    import('@renderer/components/workInbox/InboxHome').then((m) => ({
      default: (_props: PluginRenderProps) => createElement(m.InboxHome, { initialLane: 'mail' })
    }))
  ),
  schedule: lazy(() =>
    import('@renderer/pages/org/OrgPage').then((m) => ({
      default: (props: PluginRenderProps) =>
        createElement<Partial<PluginRenderProps> & { initialSection?: 'schedule' }>(m.default, {
          ...props,
          initialSection: 'schedule'
        })
    }))
  ),
  org: lazy(() => import('@renderer/pages/org/OrgPage')),
  platform: lazy(() => import('@renderer/pages/platform/PlatformPage')),
  mini_app: lazy(() => import('@renderer/pages/miniApps/MiniAppsPage')),
  notes: lazy(() => import('@renderer/pages/notes/NotesPage')),
  capabilities: lazy(() =>
    import('@renderer/pages/capabilities/CapabilitiesPage').then((m) => ({ default: () => createElement(m.default) }))
  ),
  documents: lazy(() =>
    import('@renderer/pages/documents/DocumentsPage').then((m) => ({ default: () => createElement(m.default) }))
  ),
  plugins: lazy(() => import('@renderer/pages/plugins/PluginStorePage')),
  knowledge: lazy(() => import('./builtinRoutePages').then((m) => ({ default: m.KnowledgePluginPage }))),
  files: lazy(() => import('./builtinRoutePages').then((m) => ({ default: m.FilesPluginPage }))),
  code_tools: lazy(() => import('./builtinRoutePages').then((m) => ({ default: m.CodeToolsPluginPage })))
} satisfies Record<SidebarAppId, ComponentType<PluginRenderProps>>

const TEAM_PAGES = {
  knowledge: lazy(() => import('./builtinRoutePages').then((m) => ({ default: m.TeamKnowledgePage }))),
  mail: lazy(() => import('@renderer/pages/mail/MailPage')),
  schedule: lazy(() => import('@renderer/pages/schedule/SchedulePage'))
} satisfies Partial<Record<SidebarAppId, ComponentType<PluginRenderProps>>>

export const BUILTIN_WORKBENCH_PLUGINS: readonly WorkbenchPlugin[] = Object.freeze(
  SIDEBAR_APPS.map(
    (app): WorkbenchPlugin => ({
      id: `studio.${app.id}`,
      generation: app.id === 'mail' || app.id === 'schedule' || app.id === 'knowledge' || app.id === 'org' ? 3 : 2,
      scopes: ['window'],
      contributions: [
        ...(app.id === 'mail' || app.id === 'schedule'
          ? []
          : [
              {
                id: 'activity',
                slot: 'activity' as const,
                labelKey: getSidebarIconLabelKey(app.id),
                pageId: 'page',
                applicationId: app.id,
                icon: SIDEBAR_ICON_COMPONENTS[app.id]
              }
            ]),
        { id: 'page', slot: 'page', labelKey: getSidebarIconLabelKey(app.id), component: PAGES[app.id] },
        ...(app.id === 'mail' || app.id === 'schedule' || app.id === 'knowledge'
          ? [
              {
                id: 'team-content',
                slot: 'page' as const,
                labelKey: getSidebarIconLabelKey(app.id),
                component: TEAM_PAGES[app.id]
              }
            ]
          : [])
      ]
    })
  )
)
