import { useTranslation } from 'react-i18next'

import { usePluginContributions } from './PluginScopeProvider'
import { PluginSlot } from './PluginSlot'

export function PluginPage({ page, allowBuiltins = true }: { page?: string; allowBuiltins?: boolean }) {
  const { t } = useTranslation()
  const activities = usePluginContributions('activity')
  const boundToBuiltin = activities.some(
    (item) =>
      item.contribution.slot === 'activity' &&
      item.contribution.applicationId !== undefined &&
      `${item.pluginId}/${item.contribution.pageId}` === page
  )
  const available = usePluginContributions('page').some((item) => item.key === page && item.context.isCurrent())
  return (
    <div className="flex h-full min-h-0 w-full flex-1 flex-col">
      {available && (allowBuiltins || !boundToBuiltin) ? (
        <PluginSlot slot="page" contributionKey={page} />
      ) : (
        <div role="status">{t('sidebar.resource_missing')}</div>
      )}
    </div>
  )
}
