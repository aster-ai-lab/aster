import { useCatalogIconSrc } from '@renderer/hooks/useCatalogIconSrc'
import { cn } from '@renderer/utils/style'

import type { CapabilityEntry } from './capabilitiesData'

export function CapabilityMark({ entry, compact = false }: { entry: CapabilityEntry; compact?: boolean }) {
  const src = useCatalogIconSrc(entry.iconUrl)
  const initial = Array.from(entry.name.trim())[0]?.toLocaleUpperCase() ?? '?'
  return (
    <span
      aria-hidden
      data-ui="capabilities.mark"
      className={cn(
        'flex shrink-0 items-center justify-center overflow-hidden rounded-[10px]',
        compact ? 'size-9' : 'size-10',
        src ? 'bg-[#f4f5f7]' : 'text-white'
      )}
      style={src ? undefined : { backgroundColor: entry.color }}>
      {src ? (
        <img src={src} alt="" className="size-full object-contain p-1" />
      ) : (
        <span className={cn('font-semibold', compact ? 'text-[15px]' : 'text-[16px]')}>{initial}</span>
      )}
    </span>
  )
}
