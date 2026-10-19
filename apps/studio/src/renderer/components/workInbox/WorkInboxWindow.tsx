import { DialogPortalContainerProvider } from '@a3s-lab/ui'
import { ClipboardList } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { AppletWindowBar } from '@renderer/pages/agents/AppletWindowBar'

import { InboxHome } from './InboxHome'
import { type WorkInboxNav, type WorkInboxViewProps } from './WorkInboxView'

/** Native mini-app window for the message-center approval list. */
export function WorkInboxWindow(props: WorkInboxViewProps) {
  const { t } = useTranslation()
  const title = t('work.inbox.entry')
  const actions = useRef<WorkInboxNav>({
    canBack: false,
    canForward: false,
    back: () => undefined,
    forward: () => undefined,
    home: () => undefined,
    reload: () => undefined
  })
  const [flags, setFlags] = useState({ canBack: false, canForward: false })
  const [portal, setPortal] = useState<HTMLElement | null>(null)
  const onNav = useCallback((nav: WorkInboxNav) => {
    actions.current = nav
    setFlags((current) =>
      current.canBack === nav.canBack && current.canForward === nav.canForward
        ? current
        : { canBack: nav.canBack, canForward: nav.canForward }
    )
  }, [])
  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = title
    return () => {
      document.title = previous
    }
  }, [title])
  return (
    <section
      aria-label={title}
      data-ui="work.inbox.window"
      data-applet="inbox"
      data-presentation="window"
      ref={setPortal}
      className="fixed inset-0 flex flex-col overflow-hidden bg-white text-[#171717]">
      {/* Absolute mail dialogs anchor here. This window is position:fixed, so the document body has no height. */}
      <DialogPortalContainerProvider container={portal}>
        <AppletWindowBar
          title={title}
          icon={<ClipboardList className="size-4" strokeWidth={1.7} aria-hidden />}
          canBack={flags.canBack}
          canForward={flags.canForward}
          onBack={() => actions.current.back()}
          onForward={() => actions.current.forward()}
          onHome={() => actions.current.home()}
          onRefresh={() => actions.current.reload()}
        />
        <InboxHome {...props} onNav={onNav} />
      </DialogPortalContainerProvider>
    </section>
  )
}
