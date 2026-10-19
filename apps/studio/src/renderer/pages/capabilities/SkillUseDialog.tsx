import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@a3s-lab/ui'
import { Download, Lightbulb, List, MessageSquare } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import type { CapabilityEntry } from './capabilitiesData'
import { CapabilityMark } from './capabilityMark'

function versionLabel(version: string | undefined): string {
  const value = version?.trim() ?? ''
  if (!value) return '—'
  return /^v/i.test(value) ? value : `v${value}`
}

export function SkillUseDialog({
  entry,
  pending,
  onOpenChange,
  onInstall,
  onUsePrompt
}: {
  entry: CapabilityEntry | null
  pending: boolean
  onOpenChange: (open: boolean) => void
  onInstall: (entry: CapabilityEntry) => Promise<boolean>
  onUsePrompt: (entry: CapabilityEntry, prompt: string) => void
}) {
  const { t } = useTranslation()
  const open = entry !== null
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {entry ? (
        <DialogContent
          closeLabel={t('common.close')}
          overlayClassName="!opacity-100"
          className="flex max-h-[min(720px,calc(100vh-4rem))] w-full max-w-[min(440px,calc(100%-2rem))] !animate-none flex-col gap-0 overflow-hidden p-0 !opacity-100 sm:max-w-[min(440px,calc(100%-2rem))]">
          <SkillUseBody entry={entry} pending={pending} onInstall={onInstall} onUsePrompt={onUsePrompt} />
        </DialogContent>
      ) : null}
    </Dialog>
  )
}

function SkillUseBody({
  entry,
  pending,
  onInstall,
  onUsePrompt
}: {
  entry: CapabilityEntry
  pending: boolean
  onInstall: (entry: CapabilityEntry) => Promise<boolean>
  onUsePrompt: (entry: CapabilityEntry, prompt: string) => void
}) {
  const { t } = useTranslation()
  const prompts = entry.prompts ?? []
  return (
    <div data-ui="capabilities.skill-dialog" className="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 pt-5 pb-6">
      <div className="flex items-start gap-3 pr-8">
        <CapabilityMark entry={entry} />
        <div className="min-w-0 flex-1">
          <DialogTitle className="truncate text-[16px] leading-6 font-medium text-[#171717]">{entry.name}</DialogTitle>
          {entry.installed ? null : (
            <button
              type="button"
              disabled={pending}
              onClick={() => void onInstall(entry)}
              className="mt-2 inline-flex h-[30px] items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#262626] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
              <Download className="lucide-custom size-3.5 text-white" strokeWidth={1.7} aria-hidden />
              {t(pending ? 'capabilities.installing' : 'capabilities.install')}
            </button>
          )}
        </div>
      </div>
      {entry.description ? (
        <DialogDescription className="m-0 mt-4 text-left text-[14px] leading-5 text-[#3f3f3f]">
          {entry.description}
        </DialogDescription>
      ) : (
        <DialogDescription className="sr-only">{entry.name}</DialogDescription>
      )}
      {prompts.length > 0 ? (
        <section className="mt-5">
          <h3 className="m-0 flex items-center gap-1.5 text-[14px] font-medium text-[#171717]">
            <Lightbulb className="size-4" strokeWidth={1.7} aria-hidden />
            {t('capabilities.skill.try')}
          </h3>
          <div className="mt-3 flex flex-col gap-2">
            {prompts.map((prompt) => (
              <button
                key={prompt}
                type="button"
                disabled={pending}
                onClick={() => onUsePrompt(entry, prompt)}
                className="flex w-full items-center gap-3 rounded-[12px] bg-[#f5f5f5] px-4 py-3 text-left text-[14px] leading-5 text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                <span className="min-w-0 flex-1">{prompt}</span>
                <MessageSquare className="size-4 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
              </button>
            ))}
          </div>
        </section>
      ) : null}
      <section className="mt-5">
        <h3 className="m-0 flex items-center gap-1.5 text-[14px] font-medium text-[#171717]">
          <List className="size-4" strokeWidth={1.7} aria-hidden />
          {t('capabilities.skill.about')}
        </h3>
        <div className="mt-3 flex items-center gap-2 rounded-[12px] bg-[#f5f5f5] px-4 py-3 text-[14px] leading-5">
          <span className="text-[#8a8a8a]">{t('capabilities.skill.version')}</span>
          <span className="font-medium text-[#171717]">{versionLabel(entry.version)}</span>
        </div>
      </section>
    </div>
  )
}
