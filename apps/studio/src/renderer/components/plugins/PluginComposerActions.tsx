import { Button } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import type { PluginComposerDraft } from '@renderer/types/workbenchPlugin'

import { usePluginContributions, usePluginScope } from './PluginScopeProvider'

const logger = loggerService.withContext('PluginComposerActions')

export function PluginComposerActions({
  readDraft,
  applyEdit,
  disabled = false
}: {
  readDraft: () => PluginComposerDraft
  applyEdit: (expected: PluginComposerDraft, replacement: string) => void
  disabled?: boolean
}) {
  const { t } = useTranslation()
  const contributions = usePluginContributions('composer', true)
  let consumer = usePluginScope()
  while (consumer && ['run', 'turn', 'subtask'].includes(consumer.options.kind))
    consumer = consumer.options.parent ?? null
  const ownerId = consumer?.id
  const [busy, setBusy] = useState<ReadonlySet<string>>(new Set())
  const mounted = useRef(false)
  const controllers = useRef(new Map<string, globalThis.AbortController>())
  useEffect(() => {
    const active = controllers.current
    mounted.current = true
    return () => {
      mounted.current = false
      for (const controller of active.values()) controller.abort()
      active.clear()
    }
  }, [])
  useEffect(() => {
    const keys = new Set(
      contributions.map((item) => `${item.key}@${item.pluginGeneration}/${item.context.scopeId}/${ownerId}`)
    )
    for (const [key, controller] of controllers.current) {
      if (disabled || !keys.has(key)) controller.abort()
    }
  }, [contributions, disabled, ownerId])

  return contributions.map((item) => {
    const contribution = item.contribution
    if (contribution.slot !== 'composer') return null
    const key = `${item.key}@${item.pluginGeneration}/${item.context.scopeId}/${ownerId}`
    return (
      <Button
        key={key}
        type="button"
        variant="ghost"
        size="sm"
        disabled={disabled || busy.has(key)}
        onClick={() => {
          if (disabled || controllers.current.has(key) || !item.context.isCurrent()) return
          const draft = readDraft()
          const controller = new globalThis.AbortController()
          controllers.current.set(key, controller)
          setBusy((current) => new Set([...current, key]))
          const finish = () => {
            controllers.current.delete(key)
            if (mounted.current) setBusy((current) => new Set([...current].filter((value) => value !== key)))
          }
          try {
            item.context.spawn(async (signal) => {
              try {
                const replacement = await contribution.invoke(item.context, draft.text, draft, controller.signal)
                if (typeof replacement !== 'string') throw new Error('Plugin composer edit must be text')
                if (!signal.aborted && !controller.signal.aborted && mounted.current && item.context.isCurrent())
                  applyEdit(draft, replacement)
              } catch (error) {
                logger.error('Plugin composer action failed', error instanceof Error ? error : new Error(String(error)))
              } finally {
                finish()
              }
            })
          } catch (error) {
            logger.error('Plugin composer admission failed', error instanceof Error ? error : new Error(String(error)))
            finish()
          }
        }}>
        {contribution.title || t(contribution.labelKey)}
      </Button>
    )
  })
}
