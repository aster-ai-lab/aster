import type { DragEvent as ReactDragEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

type DragFileEvent = { dataTransfer: DataTransfer | null }

/** WebKit often withholds `types` until drop, while `files` is already populated. */
export function dragEventCarriesFiles(event: ReactDragEvent<HTMLElement> | DragFileEvent): boolean {
  const transfer = event.dataTransfer
  if (!transfer) return false
  if (transfer.files && transfer.files.length > 0) return true
  return Array.from(transfer.types ?? []).includes('Files')
}

/**
 * The active-drag affordance. The dashed frame sits on the overlay so the
 * composer box does not change border width and shift while the pointer moves.
 */
export function ComposerFileDropOverlay({
  active,
  roundedClassName = 'rounded-[20px]'
}: {
  active: boolean
  roundedClassName?: string
}) {
  const { t } = useTranslation()
  if (!active) return null
  return (
    <div
      data-ui="composer.file-drop"
      role="status"
      aria-live="polite"
      className={cn(
        'pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed border-[#171717] bg-white/90',
        roundedClassName
      )}>
      <span className="text-[13px] font-medium text-[#171717]">{t('chat.input.drop_files')}</span>
    </div>
  )
}
