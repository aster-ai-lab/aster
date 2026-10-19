import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/browser'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'agent-control',
    titleKey: 'settings.browser.control',
    descriptionKey: 'settings.browser.controlHelp'
  },
  {
    anchorId: 'open-links',
    titleKey: 'settings.browser.openLinks',
    descriptionKey: 'settings.browser.openLinksHelp'
  },
  {
    anchorId: 'chrome-import',
    titleKey: 'settings.aligned.browser.chrome',
    aliases: ['chrome', 'cookie', '从 Chrome 导入 Cookie']
  },
  { anchorId: 'import', titleKey: 'settings.browser.import', descriptionKey: 'settings.browser.importHelp' },
  { anchorId: 'history', titleKey: 'settings.browser.history', descriptionKey: 'settings.browser.historyHelp' },
  { anchorId: 'clear', titleKey: 'settings.browser.clear', descriptionKey: 'settings.browser.clearHelp' }
]
