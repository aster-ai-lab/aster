import type { SettingsSearchEntry } from '../settingsSearch/types'

// Indexed rows = statically visible actionable rows (D8): context-management
// children behind their master switch stay out — their anchors may not exist on jump.
export const route = '/settings/general'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'launch-onboot',
    titleKey: 'settings.launch.onboot',
    groupKey: 'settings.launch.title',
    aliases: ['auto start', '开机自启', '开机启动', '自启动']
  },
  {
    anchorId: 'launch-totray',
    titleKey: 'settings.launch.totray',
    groupKey: 'settings.launch.title'
  },
  {
    anchorId: 'tray-show',
    titleKey: 'settings.tray.show',
    groupKey: 'settings.launch.title'
  },
  {
    anchorId: 'tray-onclose',
    titleKey: 'settings.tray.onclose',
    groupKey: 'settings.launch.title'
  },
  {
    anchorId: 'prevent-sleep-when-busy',
    titleKey: 'settings.power.prevent_sleep_when_busy',
    groupKey: 'settings.launch.title'
  },
  {
    anchorId: 'workspace-folder',
    titleKey: 'settings.workspace.folder',
    descriptionKey: 'settings.workspace.description',
    groupKey: 'settings.workspace.title',
    aliases: ['workspace', '.a3s', '工作区']
  },
  {
    anchorId: 'context-max-messages',
    titleKey: 'settings.models.context_management.max_messages',
    groupKey: 'settings.models.context_management.title',
    aliases: ['context count', '上下文数量']
  },
  {
    anchorId: 'context-enabled',
    titleKey: 'settings.models.context_management.enabled',
    groupKey: 'settings.models.context_management.title'
  }
]
