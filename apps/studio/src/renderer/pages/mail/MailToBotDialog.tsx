import { Dialog, DialogContent, DialogTitle, Switch } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { publishMailToBotPolicy, publishWorkToBotPolicy } from '@renderer/data/studioAgentPlane'
import { ToBotMark } from '@renderer/pages/messages/ToBotMark'
import {
  loadWorkToBotPolicy,
  saveWorkToBotPolicy,
  WORK_TOBOT_KINDS,
  workToBotKindLabel,
  type WorkToBotKind,
  type WorkToBotPolicy
} from '@renderer/workInbox/workToBot'

import {
  loadMailToBotPolicy,
  MAIL_TOBOT_FOLDERS,
  saveMailToBotPolicy,
  type MailToBotFolder,
  type MailToBotPolicy
} from './mailToBot'

export function MailToBotDialog({
  open,
  onOpenChange,
  mailReadable = true
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  mailReadable?: boolean
}) {
  const { t } = useTranslation()
  const [tasks, setTasks] = useState<WorkToBotPolicy>(() => loadWorkToBotPolicy())
  const [mail, setMail] = useState<MailToBotPolicy>(() => loadMailToBotPolicy())

  useEffect(() => {
    if (!open) return
    const nextTasks = loadWorkToBotPolicy()
    const nextMail = loadMailToBotPolicy()
    setTasks(nextTasks)
    setMail(nextMail)
    void publishWorkToBotPolicy(nextTasks).catch(() => undefined)
    void publishMailToBotPolicy(nextMail).catch(() => undefined)
  }, [open])

  const commitTasks = (next: WorkToBotPolicy) => {
    const saved = saveWorkToBotPolicy(next)
    setTasks(saved)
    void publishWorkToBotPolicy(saved).catch(() => undefined)
  }

  const commitMail = (next: MailToBotPolicy) => {
    const saved = saveMailToBotPolicy(next)
    setMail(saved)
    void publishMailToBotPolicy(saved).catch(() => undefined)
  }

  const toggleKind = (kind: WorkToBotKind) => {
    const has = tasks.kinds.includes(kind)
    if (has && tasks.kinds.length === 1) return
    const kinds = has ? tasks.kinds.filter((item) => item !== kind) : [...tasks.kinds, kind]
    commitTasks({ ...tasks, kinds })
  }

  const toggleFolder = (folder: MailToBotFolder) => {
    const has = mail.folders.includes(folder)
    if (has && mail.folders.length === 1) return
    const folders = has ? mail.folders.filter((item) => item !== folder) : [...mail.folders, folder]
    commitMail({ ...mail, folders })
  }

  const taskDetails = tasks.allow
  const mailDetails = mail.allow && mailReadable
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        data-ui="mail.tobot.dialog"
        closeLabel={t('common.close')}
        overlayClassName="absolute z-[120]"
        className="absolute z-[120] max-h-[min(640px,calc(100%-32px))] gap-0 overflow-hidden bg-white p-0 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div className="flex items-center gap-3 px-5 pt-4 pr-14">
          <ToBotMark size={28} />
          <DialogTitle className="m-0 text-[16px] font-medium">{t('work.tobot.title')}</DialogTitle>
        </div>
        <div className="max-h-[min(560px,calc(100vh-96px))] overflow-y-auto">
          <section className="px-3 pt-3 pb-1">
            <p className="m-0 px-2 text-[12px] text-[#8a8a8a]">{t('work.inbox.entry')}</p>
            <p className="m-0 px-2 pt-1 text-[13px] leading-5 text-[#8a8a8a]">{t('work.tobot.limit')}</p>
            <PolicyRow
              label={t('work.tobot.allow')}
              hint={t('work.tobot.allow_hint')}
              checked={tasks.allow}
              onCheckedChange={(allow) => commitTasks({ ...tasks, allow })}
            />
            <PolicyRow
              label={t('work.tobot.list')}
              checked={tasks.list}
              disabled={!taskDetails}
              onCheckedChange={(list) => commitTasks({ ...tasks, list, read: list ? tasks.read : false })}
            />
            <PolicyRow
              label={t('work.tobot.read')}
              checked={tasks.read && tasks.list}
              disabled={!taskDetails || !tasks.list}
              onCheckedChange={(read) => commitTasks({ ...tasks, read })}
            />
            <p className="m-0 px-2 pt-2 text-[12px] text-[#8a8a8a]">{t('work.tobot.kinds')}</p>
            <div className="flex flex-wrap gap-1 px-2 pb-1">
              {WORK_TOBOT_KINDS.map((kind) => {
                const selected = tasks.kinds.includes(kind)
                return (
                  <button
                    key={kind}
                    type="button"
                    aria-pressed={selected}
                    disabled={!taskDetails || !tasks.list}
                    className={`h-8 rounded-lg px-2.5 text-[13px] ${selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'} disabled:pointer-events-none disabled:opacity-40`}
                    onClick={() => toggleKind(kind)}>
                    {t(workToBotKindLabel(kind))}
                  </button>
                )
              })}
            </div>
          </section>
          <section className="px-3 pt-2 pb-3">
            <p className="m-0 px-2 text-[12px] text-[#8a8a8a]">{t('mail.title')}</p>
            <p className="m-0 px-2 pt-1 text-[13px] leading-5 text-[#8a8a8a]">{t('mail.tobot.limit')}</p>
            <PolicyRow
              label={t('mail.tobot.allow')}
              hint={t('mail.tobot.allow_hint')}
              checked={mail.allow}
              disabled={!mailReadable}
              onCheckedChange={(allow) => commitMail({ ...mail, allow })}
            />
            <PolicyRow
              label={t('mail.tobot.list')}
              checked={mail.list}
              disabled={!mailDetails}
              onCheckedChange={(list) => commitMail({ ...mail, list, read: list ? mail.read : false })}
            />
            <PolicyRow
              label={t('mail.tobot.read')}
              hint={t('mail.tobot.read_hint')}
              checked={mail.read && mail.list}
              disabled={!mailDetails || !mail.list}
              onCheckedChange={(read) => commitMail({ ...mail, read })}
            />
            <p className="m-0 px-2 pt-2 text-[12px] text-[#8a8a8a]">{t('mail.tobot.folders')}</p>
            <div className="flex flex-wrap gap-1 px-2 pb-1">
              {MAIL_TOBOT_FOLDERS.map((folder) => {
                const selected = mail.folders.includes(folder)
                return (
                  <button
                    key={folder}
                    type="button"
                    aria-pressed={selected}
                    disabled={!mailDetails || !mail.list}
                    className={`h-8 rounded-lg px-2.5 text-[13px] ${selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'} disabled:pointer-events-none disabled:opacity-40`}
                    onClick={() => toggleFolder(folder)}>
                    {t(`mail.folder.${folder}`)}
                  </button>
                )
              })}
            </div>
          </section>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function PolicyRow({
  label,
  hint,
  checked,
  disabled,
  onCheckedChange
}: {
  label: string
  hint?: string
  checked: boolean
  disabled?: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg px-2 py-2">
      <div className="min-w-0">
        <p className="m-0 text-[13px] text-[#171717]">{label}</p>
        {hint ? <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{hint}</p> : null}
      </div>
      <Switch size="sm" aria-label={label} checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
    </div>
  )
}
