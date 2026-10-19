import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@a3s-lab/ui'
import { useEffect, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { useOptionalTabsContext } from '@renderer/hooks/tab'
import { commandMod, desktopCapabilities } from '@renderer/host/desktopHost'
import {
  encodeAixWorkbenchRouteKey,
  publishAixCatalogue,
  readAixCatalogue,
  subscribeAixCatalogue
} from '@renderer/services/plugins'
import { toast } from '@renderer/services/toast'
import { pluginPageUrl } from '@renderer/utils/pluginNavigation'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

export function publishInstalledCatalogue(catalogue: AixWorkbenchCatalogue) {
  publishAixCatalogue(catalogue)
}

function eventChord(event: KeyboardEvent): string {
  const parts: string[] = []
  if (event.ctrlKey) parts.push('ctrl')
  if (event.altKey) parts.push('alt')
  if (event.shiftKey) parts.push('shift')
  if (event.metaKey) parts.push('cmd')
  parts.push(event.key.toLowerCase())
  return parts.join('+')
}

function bindingChords(key: string, mac?: string): string[] {
  const expand = (value: string) => value.replaceAll('mod', commandMod(desktopCapabilities().platform))
  return [expand(key), mac].filter((value): value is string => Boolean(value))
}

/** Command palette, keybindings, and custom-editor pages contributed by installed packages. */
export function ExtensionPointHost() {
  const { t } = useTranslation()
  const tabs = useOptionalTabsContext()
  const catalogue = useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
  const editors = catalogue.packages.flatMap((item) =>
    item.views
      .filter((view) => view.container === null && view.point === 'studio.editor.custom')
      .map((view) => ({
        key: `${item.slug}/${view.id}`,
        title: view.title || view.id,
        page: `aix.${item.slug}/view.${encodeAixWorkbenchRouteKey({
          package: item,
          kind: 'view',
          localId: view.id
        })}`
      }))
  )
  const [open, setOpen] = useState(false)
  const commands = catalogue.packages.flatMap((item) =>
    (item.commands ?? []).map((command) => ({
      id: `${item.slug}/${command.id}`,
      title: command.title,
      category: command.category
    }))
  )

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const chord = eventChord(event)
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'p') {
        event.preventDefault()
        setOpen(true)
        return
      }
      for (const item of catalogue.packages) {
        for (const binding of item.keybindings ?? []) {
          const command = item.commands?.find((entry) => entry.id === binding.command)
          if (!command || !bindingChords(binding.key, binding.mac).includes(chord)) continue
          event.preventDefault()
          toast.info(command.title)
          return
        }
      }
    }
    globalThis.window.addEventListener('keydown', onKey)
    return () => globalThis.window.removeEventListener('keydown', onKey)
  }, [catalogue])

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent closeLabel={t('common.close')} className="max-w-[420px]">
        <DialogHeader>
          <DialogTitle>扩展点</DialogTitle>
          <DialogDescription>已安装包贡献的命令和自定义编辑器。</DialogDescription>
        </DialogHeader>
        <div className="flex max-h-80 flex-col gap-1 overflow-y-auto">
          {commands.map((command) => (
            <button
              key={command.id}
              type="button"
              className="rounded-[8px] px-3 py-2 text-left text-[13px] hover:bg-[#f5f5f5]"
              onClick={() => {
                toast.info(command.title)
                setOpen(false)
              }}>
              <span className="text-[#8a8a8a]">{command.category ? `${command.category} · ` : ''}</span>
              {command.title}
            </button>
          ))}
          {editors.map((item) => (
            <button
              key={item.key}
              type="button"
              className="rounded-[8px] px-3 py-2 text-left text-[13px] hover:bg-[#f5f5f5]"
              onClick={() => {
                tabs?.openTab(pluginPageUrl(item.page), { title: item.title })
                setOpen(false)
              }}>
              {item.title}
            </button>
          ))}
          {commands.length === 0 && editors.length === 0 ? (
            <p className="m-0 px-3 py-2 text-[13px] text-[#8a8a8a]">还没有已安装的命令或自定义编辑器。</p>
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  )
}
