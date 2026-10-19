import { cn } from '@renderer/utils/style'
export { ghostIconButtonClassName, quietButtonClassName } from '@renderer/components/collection/collectionStyles'

/** DESIGN.md §6 controls shared by the team pages. */

const focusRing = 'outline-none focus-visible:ring-2 focus-visible:ring-black/10'

export const orgFieldClassName =
  'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] shadow-none focus-visible:border-0 dark:bg-[#2c2c2c]'

export const orgSectionLabelClassName = 'm-0 text-[12px] leading-4 text-[#8a8a8a]'

export function filledButtonClassName(height: 30 | 36 = 36) {
  return cn(
    'inline-flex shrink-0 items-center justify-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] leading-none text-white hover:bg-[#2a2a2a] disabled:pointer-events-none disabled:opacity-40',
    height === 30 ? 'h-[30px] font-medium' : 'h-9',
    focusRing
  )
}

export const rowActionClassName = cn(
  'flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] hover:bg-[#f2f3f5] disabled:opacity-40',
  focusRing
)

export const orgComboboxClassName =
  'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] shadow-none dark:bg-[#2c2c2c]'
