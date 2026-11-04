import { useEffect } from 'react'

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem as PrimitiveCommandItem,
  CommandList,
  CommandShortcut,
} from '../primitives/command'

export type CommandItem = {
  id: string
  label: string
  hint?: string
  keywords?: string
  onSelect: () => void
}

export type CommandPaletteProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  items: CommandItem[]
  placeholder?: string
  emptyText?: string
}

/**
 * Console command palette on shared Arc CommandDialog / cmdk primitives.
 * Prefer composing `CommandDialog` directly for new Studio surfaces.
 */
export function CommandPalette({
  open,
  onOpenChange,
  items,
  placeholder = 'Type a command…',
  emptyText = 'No commands',
}: CommandPaletteProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        onOpenChange(!open)
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, onOpenChange])

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Command palette"
      description={placeholder}
      showCloseButton={false}
      className="ui-command-panel overflow-hidden p-0 sm:max-w-lg"
    >
      <CommandInput placeholder={placeholder} />
      <CommandList className="max-h-72">
        <CommandEmpty>{emptyText}</CommandEmpty>
        <CommandGroup>
          {items.map((item) => (
            <PrimitiveCommandItem
              key={item.id}
              value={`${item.label} ${item.hint ?? ''} ${item.keywords ?? ''}`}
              onSelect={() => {
                item.onSelect()
                onOpenChange(false)
              }}
            >
              <span>{item.label}</span>
              {item.hint ? <CommandShortcut>{item.hint}</CommandShortcut> : null}
            </PrimitiveCommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  )
}
