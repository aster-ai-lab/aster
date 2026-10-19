import {
  Activity,
  AppWindow,
  Archive,
  Bell,
  CalendarClock,
  Code,
  Command,
  Contrast,
  Crop,
  FileBox,
  FileCode,
  GitBranch,
  Globe,
  HardDrive,
  Info,
  Keyboard,
  MonitorSmartphone,
  Package,
  Palette,
  PictureInPicture2,
  Radio,
  Route,
  ScanText,
  Search,
  Settings2,
  Terminal,
  TextCursorInput,
  ToolCase,
  Zap
} from 'lucide-react'
import type { ReactNode } from 'react'
import { createElement } from 'react'

import { GatewayIcon } from '@renderer/components/icons/GatewayIcon'
import { McpLogo } from '@renderer/components/icons/SvgIcon'
import { isSettingsPath } from '@shared/data/types/settingsPath'

function navIcon(icon: typeof AppWindow): ReactNode {
  return createElement(icon, { strokeWidth: 1.7, 'aria-hidden': true })
}

export interface SettingsNavItem {
  id: string
  titleKey: string
  icon: ReactNode
  /** Group title key (`settings.menuGroups.*`). Omitted for the ungrouped head item. */
  groupKey?: string
  /** Shown under the page title. 15px muted, 4px below the title. */
  descriptionKey?: string
  to: string
  /** Distinguishes two rows that share `to`. Matched against `?section=`. */
  section?: string
  /** Optional `?panel=` match. The data menu is one row; its panels stay inside the page. */
  panel?: string
  /** Settings anchor scrolled into view when `section` is set. */
  focusId?: string
}

/**
 * Sidebar shown on the settings page. 偏好 holds 通用, 外观, 快捷键, and 用量.
 * 连接 holds 渠道, then 数据.
 * 开发者 holds 执行轨迹, then 环境依赖, and sits above 系统.
 * 自主进化 stays routed, and is omitted from this sidebar.
 * 系统 holds 归档, then 关于.
 */
export const settingsNav: readonly SettingsNavItem[] = [
  {
    id: 'general',
    titleKey: 'settings.general.common.title',
    icon: navIcon(AppWindow),
    groupKey: 'settings.menuGroups.personal',
    to: '/settings/general'
  },
  {
    id: 'appearance',
    titleKey: 'settings.appearance.title',
    icon: navIcon(Contrast),
    groupKey: 'settings.menuGroups.personal',
    to: '/settings/appearance'
  },
  {
    id: 'shortcut',
    titleKey: 'settings.shortcuts.title',
    icon: navIcon(Keyboard),
    groupKey: 'settings.menuGroups.personal',
    to: '/settings/shortcut'
  },
  {
    id: 'usage',
    titleKey: 'settings.usage.title',
    icon: navIcon(Activity),
    groupKey: 'settings.menuGroups.personal',
    to: '/settings/usage'
  },
  {
    id: 'channels',
    titleKey: 'settings.channels.title',
    icon: navIcon(Radio),
    groupKey: 'settings.menuGroups.connections',
    to: '/settings/channels'
  },
  {
    id: 'data',
    titleKey: 'settings.data.title',
    icon: navIcon(HardDrive),
    groupKey: 'settings.menuGroups.connections',
    to: '/settings/data'
  },
  {
    id: 'traces',
    titleKey: 'settings.traces.title',
    icon: navIcon(Route),
    groupKey: 'settings.menuGroups.developer',
    to: '/settings/traces'
  },
  {
    id: 'dependencies',
    titleKey: 'settings.dependencies.title',
    icon: navIcon(Terminal),
    groupKey: 'settings.menuGroups.developer',
    to: '/settings/dependencies'
  },
  {
    id: 'archive',
    titleKey: 'settings.nav.archivedTasks',
    icon: navIcon(Archive),
    groupKey: 'settings.menuGroups.system',
    descriptionKey: 'settings.aligned.archive.desc',
    to: '/settings/archive'
  },
  {
    id: 'about',
    titleKey: 'settings.aligned.about.title',
    icon: navIcon(Info),
    groupKey: 'settings.menuGroups.system',
    to: '/settings/about'
  }
]

function pathMatches(pathname: string, to: string): boolean {
  return pathname === to || pathname.startsWith(`${to}/`)
}

/** Nav row that owns this URL, if the sidebar lists it. */
export function matchSettingsNavItem(url: string): SettingsNavItem | undefined {
  let parsed: URL
  try {
    parsed = new URL(url, 'https://www.cherry-ai.com')
  } catch {
    return undefined
  }
  const pathname = parsed.pathname
  const panel = parsed.searchParams.get('panel') ?? ''
  if (panel) {
    const panelMatch = settingsNav.find((item) => item.panel === panel && pathMatches(pathname, item.to))
    if (panelMatch) return panelMatch
  }
  const section = parsed.searchParams.get('section') ?? ''
  if (section) {
    const sectionMatch = settingsNav.find((item) => item.section === section && pathMatches(pathname, item.to))
    if (sectionMatch) return sectionMatch
  }
  return settingsNav.find((item) => !item.section && !item.panel && pathMatches(pathname, item.to))
}

/** Resolve the section label without changing the default settings tab title. */
export function getSettingsSectionTitleKey(url: string): string | undefined {
  if (!isSettingsPath(url)) return undefined
  const navTitle = matchSettingsNavItem(url)?.titleKey
  if (navTitle) return navTitle
  const pathname = new URL(url, 'https://www.cherry-ai.com').pathname
  return (
    settingsMenu.find(({ route }) => pathname === route || pathname.startsWith(`${route}/`))?.titleKey ??
    (pathname === '/settings/search' ? 'common.search' : undefined)
  )
}

export interface SettingsMenuEntry {
  /** Settings section route; also the aggregation key for `.search.ts` leaves */
  route: string
  /** i18n key of the menu title — always searchable as the section baseline */
  titleKey: string
  icon: ReactNode
  /** Group title key (`settings.menuGroups.*`); omitted for the ungrouped head section */
  groupKey?: string
}

/**
 * Search index for every settings section. Sidebar order is `settingsNav`.
 * Array order is the search tie-break order.
 * Adding a settings section requires registering it here, which also makes its
 * title searchable — the settings search baseline is structural, not manual.
 */
export const settingsMenu: readonly SettingsMenuEntry[] = [
  { route: '/settings/model', titleKey: 'settings.model', icon: createElement(Package) },
  {
    route: '/settings/local-models',
    titleKey: 'settings.dependencies.localModels.title',
    icon: createElement(FileBox)
  },
  { route: '/settings/api-gateway', titleKey: 'apiGateway.title', icon: createElement(GatewayIcon) },
  {
    route: '/settings/mcp',
    titleKey: 'agent.settings.toolsMcp.mcp.tab',
    icon: createElement(McpLogo, { width: 16, height: 16, className: 'text-foreground' }),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/skills',
    titleKey: 'settings.skills.title',
    icon: createElement(ToolCase),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/prompts',
    titleKey: 'settings.nav.personalization',
    icon: createElement(Zap),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/browser',
    titleKey: 'settings.browser.title',
    icon: createElement(Globe),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/websearch',
    titleKey: 'settings.tool.websearch.title',
    icon: createElement(Search),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/file-processing',
    titleKey: 'settings.tool.file_processing.features.document_to_markdown.title',
    icon: createElement(FileCode),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/ocr',
    titleKey: 'settings.tool.file_processing.features.image_to_text.title',
    icon: createElement(ScanText),
    groupKey: 'settings.menuGroups.capabilities'
  },
  {
    route: '/settings/general',
    titleKey: 'settings.general.common.title',
    icon: createElement(Settings2),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/appearance',
    titleKey: 'settings.appearance.title',
    icon: createElement(Palette),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/notifications',
    titleKey: 'settings.notification.title',
    icon: createElement(Bell),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/data',
    titleKey: 'settings.data.title',
    icon: createElement(HardDrive),
    groupKey: 'settings.menuGroups.connections'
  },
  {
    route: '/settings/code-review',
    titleKey: 'settings.nav.codeReview',
    icon: createElement(Code),
    groupKey: 'settings.menuGroups.coding'
  },
  {
    route: '/settings/worktree',
    titleKey: 'settings.nav.worktrees',
    icon: createElement(GitBranch),
    groupKey: 'settings.menuGroups.coding'
  },
  {
    route: '/settings/archive',
    titleKey: 'settings.nav.archivedTasks',
    icon: createElement(Archive),
    groupKey: 'settings.menuGroups.system'
  },
  {
    route: '/settings/usage',
    titleKey: 'settings.usage.title',
    icon: createElement(Activity),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/channels',
    titleKey: 'settings.channels.title',
    icon: createElement(Radio),
    groupKey: 'settings.menuGroups.connections'
  },
  {
    route: '/settings/device-connections',
    titleKey: 'deviceConnections.title',
    icon: createElement(MonitorSmartphone),
    groupKey: 'settings.menuGroups.automation'
  },
  {
    route: '/settings/scheduled-tasks',
    titleKey: 'settings.scheduledTasks.title',
    icon: createElement(CalendarClock),
    groupKey: 'settings.menuGroups.automation'
  },
  {
    route: '/settings/shortcut',
    titleKey: 'settings.shortcuts.title',
    icon: createElement(Command),
    groupKey: 'settings.menuGroups.personal'
  },
  {
    route: '/settings/quick-assistant',
    titleKey: 'settings.quickAssistant.title',
    icon: createElement(PictureInPicture2),
    groupKey: 'settings.menuGroups.automation'
  },
  {
    route: '/settings/selection-assistant',
    titleKey: 'selection.name',
    icon: createElement(TextCursorInput),
    groupKey: 'settings.menuGroups.automation'
  },
  {
    route: '/settings/screenshot',
    titleKey: 'settings.screenshot.title',
    icon: createElement(Crop),
    groupKey: 'settings.menuGroups.automation'
  },
  {
    route: '/settings/about',
    titleKey: 'settings.aligned.about.title',
    icon: createElement(Info),
    groupKey: 'settings.menuGroups.system'
  },
  {
    route: '/settings/traces',
    titleKey: 'settings.traces.title',
    icon: createElement(Route),
    groupKey: 'settings.menuGroups.developer'
  },
  {
    route: '/settings/dependencies',
    titleKey: 'settings.dependencies.title',
    icon: createElement(Terminal),
    groupKey: 'settings.menuGroups.developer'
  }
]
