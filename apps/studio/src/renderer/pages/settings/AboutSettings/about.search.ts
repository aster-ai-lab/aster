import type { SettingsSearchEntry } from '../settingsSearch/types'

export const route = '/settings/about'

export const entries: SettingsSearchEntry[] = [
  {
    anchorId: 'update',
    titleKey: 'settings.about.checkUpdate.label',
    groupKey: 'settings.aligned.about.title',
    aliases: ['update', '更新', '重新启动', '版本', 'restart']
  },
  {
    anchorId: 'terms',
    titleKey: 'settings.about.page.terms',
    groupKey: 'settings.aligned.about.title',
    aliases: ['terms', '用户协议', '协议']
  },
  {
    anchorId: 'privacy',
    titleKey: 'settings.about.page.privacy',
    groupKey: 'settings.aligned.about.title',
    aliases: ['privacy', '隐私政策', '隐私']
  },
  {
    anchorId: 'copyright',
    titleKey: 'settings.about.page.copyright',
    groupKey: 'settings.aligned.about.title',
    aliases: ['copyright', '侵权', '投诉']
  },
  {
    anchorId: 'support',
    titleKey: 'settings.about.page.support',
    groupKey: 'settings.aligned.about.title',
    aliases: ['support', '客服', '反馈']
  }
]
