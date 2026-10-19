import { RefreshCw, Search } from 'lucide-react'
import type { ButtonHTMLAttributes, InputHTMLAttributes } from 'react'

import { cn } from '@renderer/utils/style'

type FileTreeSearchFieldProps = {
  value: string
  onChange: (value: string) => void
  placeholder: string
  searchLabel: string
  refreshLabel: string
  onRefresh: () => void
  refreshing?: boolean
  /** File managers keep refresh inside the field. A caller can move it outside. */
  showRefresh?: boolean
  className?: string
  inputProps?: Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'type' | 'placeholder' | 'spellCheck'>
  refreshProps?: Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type' | 'onClick' | 'aria-label' | 'aria-busy'>
}

/** File-tree search used by the workspace file manager and the knowledge sidebar. */
export function FileTreeSearchField({
  value,
  onChange,
  placeholder,
  searchLabel,
  refreshLabel,
  onRefresh,
  refreshing = false,
  showRefresh = true,
  className,
  inputProps,
  refreshProps
}: FileTreeSearchFieldProps) {
  return (
    <div className={cn('flex h-[30px] min-w-0 items-center gap-1.5 rounded-[8px] bg-[#f5f5f5] px-2.5', className)}>
      <label className="flex h-[30px] min-w-0 flex-1 items-center gap-1.5 bg-[#f5f5f5]">
        <Search className="size-3.5 shrink-0 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />
        <input
          {...inputProps}
          aria-label={searchLabel}
          className="min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] [&::-webkit-search-cancel-button]:hidden"
          placeholder={placeholder}
          spellCheck={false}
          type="search"
          value={value}
          onChange={(event) => onChange(event.currentTarget.value)}
        />
      </label>
      {showRefresh ? (
        <button
          {...refreshProps}
          type="button"
          aria-label={refreshLabel}
          aria-busy={refreshing || undefined}
          className="grid size-5 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-black/10"
          onClick={onRefresh}>
          <RefreshCw
            className={cn('size-3.5', refreshing && 'animate-spin')}
            style={refreshing ? { animationDuration: '0.7s' } : undefined}
            strokeWidth={1.7}
            aria-hidden
          />
        </button>
      ) : null}
    </div>
  )
}
