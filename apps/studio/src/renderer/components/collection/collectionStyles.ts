import { cn } from '@renderer/utils/style'

const focusRing = 'outline-none focus-visible:ring-2 focus-visible:ring-black/10'

export function quietButtonClassName(height: 30 | 36 = 30) {
  return cn(
    'inline-flex shrink-0 items-center justify-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] leading-none text-[#171717] hover:bg-[#fafafa] disabled:pointer-events-none disabled:opacity-40',
    height === 30 ? 'h-[30px]' : 'h-9',
    focusRing
  )
}

export const ghostIconButtonClassName = cn(
  'flex size-[30px] shrink-0 items-center justify-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f5f5f5] hover:text-[#171717] disabled:opacity-40',
  focusRing
)
