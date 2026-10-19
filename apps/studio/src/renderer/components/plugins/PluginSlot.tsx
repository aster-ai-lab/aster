import { Suspense } from 'react'
import { ErrorBoundary } from 'react-error-boundary'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { ToWorkPageLoading } from '@renderer/components/layout/PageEntry'

import { usePluginContributions } from './PluginScopeProvider'

const logger = loggerService.withContext('PluginSlot')

export function PluginSlot({
  slot,
  contributionKey
}: {
  slot: 'page' | 'sidebar' | 'panel' | 'workspace'
  contributionKey?: string
}) {
  const { t } = useTranslation()
  const contributions = usePluginContributions(slot)
  return contributions.map((item) => {
    if (contributionKey && contributionKey !== item.key) return null
    const contribution = item.contribution
    if (
      contribution.slot !== 'page' &&
      contribution.slot !== 'sidebar' &&
      contribution.slot !== 'panel' &&
      contribution.slot !== 'workspace'
    )
      return null
    const Component = contribution.component
    return (
      <ErrorBoundary
        key={`${item.key}@${item.pluginGeneration}`}
        fallback={<div role="alert">{t('error.boundary.default.message')}</div>}
        onError={(error) =>
          logger.error('Plugin view failed', error instanceof Error ? error : new Error(String(error)))
        }>
        <Suspense
          fallback={
            slot === 'page' ? (
              <ToWorkPageLoading label={t('common.loading')} />
            ) : (
              <div role="status">{t('common.loading')}</div>
            )
          }>
          <Component context={item.context} />
        </Suspense>
      </ErrorBoundary>
    )
  })
}
