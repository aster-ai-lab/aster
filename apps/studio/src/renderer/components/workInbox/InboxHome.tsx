import { useEffect, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { useMailUnreadCount } from '@renderer/mail/mailUnread'
import { useTeamMailAccess } from '@renderer/mail/useTeamMailAccess'
import { teamMailOrgId } from '@renderer/pages/mail/mailCompose'
import MailPage from '@renderer/pages/mail/MailPage'
import { MailToBotDialog } from '@renderer/pages/mail/MailToBotDialog'
import { ToBotMark } from '@renderer/pages/messages/ToBotMark'

import { WorkInboxView, type WorkInboxViewProps } from './WorkInboxView'

export type InboxLane = 'tasks' | 'mail'

type InboxHomeProps = WorkInboxViewProps & {
  initialLane?: InboxLane
}

function LaneTab({
  selected,
  count,
  label,
  onSelect
}: {
  selected: boolean
  count: number
  label: string
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      onClick={onSelect}
      className={`relative flex h-11 items-center gap-1.5 px-3 text-[13px] ${
        selected ? 'font-medium text-[#171717]' : 'text-[#8a8a8a] hover:text-[#171717]'
      }`}>
      <span>{label}</span>
      {count > 0 ? <span className="text-[12px] text-[#8a8a8a]">{count}</span> : null}
      {selected ? <span className="absolute right-3 bottom-0 left-3 h-0.5 rounded-full bg-[#171717]" /> : null}
    </button>
  )
}

/** Tasks and mail share one window. Each lane keeps its own list and reading pane. */
export function InboxHome({ initialLane = 'tasks', onNav, ...view }: InboxHomeProps) {
  const { t } = useTranslation()
  const [lane, setLane] = useState<InboxLane>(initialLane)
  const [toBotOpen, setToBotOpen] = useState(false)
  const mailUnread = useMailUnreadCount()
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const access = useTeamMailAccess(teamMailOrgId(centerId))

  useEffect(() => {
    if (lane !== 'mail') return
    onNav?.({
      canBack: false,
      canForward: false,
      back: () => undefined,
      forward: () => undefined,
      home: () => setLane('tasks'),
      reload: () => window.dispatchEvent(new Event('a3s:mail-changed'))
    })
  }, [lane, onNav])

  return (
    <div data-ui="work.inbox.home" className="flex min-h-0 min-w-0 flex-1 flex-col bg-white text-[#171717]">
      <div className="flex h-11 shrink-0 items-center border-b border-[#efefef] px-2">
        <div role="tablist" aria-label={t('work.inbox.entry')} className="flex h-full items-end">
          <LaneTab
            selected={lane === 'tasks'}
            count={0}
            label={t('work.inbox.entry')}
            onSelect={() => setLane('tasks')}
          />
          <LaneTab
            selected={lane === 'mail'}
            count={mailUnread}
            label={t('mail.title')}
            onSelect={() => setLane('mail')}
          />
        </div>
        <button
          type="button"
          data-ui="mail.tobot"
          className="ml-auto inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-[13px] whitespace-nowrap text-[#171717] hover:bg-[#f2f3f5]"
          onClick={() => setToBotOpen(true)}>
          <ToBotMark size={16} />
          <span>{t('mail.tobot.menu')}</span>
        </button>
      </div>
      <MailToBotDialog open={toBotOpen} onOpenChange={setToBotOpen} mailReadable={access.read} />
      <div className="flex min-h-0 flex-1 flex-col">
        {lane === 'tasks' ? <WorkInboxView {...view} onNav={onNav} /> : <MailPage />}
      </div>
    </div>
  )
}
