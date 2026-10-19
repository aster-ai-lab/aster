import { Button } from '@a3s-lab/ui'
import { Copy } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

type CopyState = 'idle' | 'pending' | 'copied' | 'failed'

/**
 * A read-only batch utility for exact IDs already visible to the current reader.
 * It floats: render it in a collection's overlay slot (bottom-centre of the list viewport), never in the flow.
 */
export function CollectionSelectionBar({
  ids,
  onClear,
  disabled = false,
  state = 'open'
}: {
  ids: readonly string[]
  onClear: () => void
  disabled?: boolean
  state?: 'open' | 'closed'
}) {
  const { t } = useTranslation()
  const selectionKey = ids.join('\n')
  const [copy, setCopy] = useState<{ key: string; state: CopyState }>({ key: selectionKey, state: 'idle' })
  const state = copy.key === selectionKey ? copy.state : 'idle'
  return (
    <section
      aria-label={t('collection.selectedActions')}
      data-ui="collection.selection"
      data-state={state}
      className={`a3s-popup-rise flex max-w-full flex-wrap items-center gap-1 rounded-[12px] border border-[#ececec] bg-white py-1.5 pr-1.5 pl-4 shadow-[0_12px_32px_rgba(0,0,0,0.12)] ${
        state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
      }`}>
      <span role="status" className="mr-3 text-[13px] whitespace-nowrap text-[#171717]">
        {t('collection.selected', { count: ids.length })}
        <span className="ml-2 text-[12px] text-[#3f3f3f]">{t('collection.currentPageOnly')}</span>
      </span>
      <Button
        size="sm"
        variant="ghost"
        disabled={disabled || state === 'pending'}
        onClick={async () => {
          setCopy({ key: selectionKey, state: 'pending' })
          try {
            await navigator.clipboard.writeText(ids.join('\n'))
            setCopy({ key: selectionKey, state: 'copied' })
          } catch {
            setCopy({ key: selectionKey, state: 'failed' })
          }
        }}>
        <Copy aria-hidden className="size-4" />
        {t('collection.copyIds')}
      </Button>
      <Button size="sm" variant="ghost" disabled={state === 'pending'} onClick={onClear}>
        {t('collection.clearSelection')}
      </Button>
      {state === 'copied' ? (
        <span role="status" className="px-2 text-[12px] text-[#3f3f3f]">
          {t('collection.copiedIds', { count: ids.length })}
        </span>
      ) : null}
      {state === 'failed' ? (
        <span role="alert" className="px-2 text-[12px] text-[#b42318]">
          {t('common.copy_failed')}
        </span>
      ) : null}
    </section>
  )
}
