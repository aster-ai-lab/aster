import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

type PaletteItem = {
  id: string
  label: string
  disabled: boolean
  action: () => void
}

type WorkbenchCommandPaletteProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  canSettle: boolean
  onNewTask: () => void
  onSettle: () => void
  onFocus: () => void
  onCycleMode: () => void
}

/** Four workbench actions. The shortcut lives in the command registry, not in this dialog. */
export function WorkbenchCommandPalette({
  open,
  onOpenChange,
  canSettle,
  onNewTask,
  onSettle,
  onFocus,
  onCycleMode
}: WorkbenchCommandPaletteProps) {
  const { t } = useTranslation()
  const title = t('settings.shortcuts.workbench_palette')
  const run = (action: () => void) => {
    action()
    onOpenChange(false)
  }
  const items: PaletteItem[] = [
    { id: 'new-task', label: t('workbench.nav.new_task'), disabled: false, action: onNewTask },
    { id: 'settle', label: t('workbench.session.settle'), disabled: !canSettle, action: onSettle },
    { id: 'focus', label: t('settings.shortcuts.focus_input'), disabled: false, action: onFocus },
    { id: 'cycle', label: t('workbench.command.cycle_mode'), disabled: false, action: onCycleMode }
  ]

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={undefined}
        data-ui="workbench.command-palette"
        className="w-[320px] gap-0 overflow-hidden rounded-xl bg-white p-0">
        <DialogTitle className="px-3 pt-3 text-sm font-medium text-[#243348]">{title}</DialogTitle>
        <ul role="menu" aria-label={title} className="flex flex-col p-2">
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                role="menuitem"
                data-ui={`workbench.command.${item.id}`}
                disabled={item.disabled}
                onClick={() => {
                  if (item.disabled) return
                  run(item.action)
                }}
                className="flex h-9 w-full items-center rounded-[8px] px-2.5 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7] disabled:opacity-40">
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  )
}
