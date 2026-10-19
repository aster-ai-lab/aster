import { Button, Tooltip } from '@a3s-lab/ui'
import { CircleArrowUp, LayoutDashboard, Rocket, Search, Settings, Stethoscope } from 'lucide-react'
import { useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { usePersistCache } from '@data/hooks/useCache'
import { loggerService } from '@logger'
import { CommandTooltip } from '@renderer/components/command'
import { DoctorPopup } from '@renderer/components/doctor'
import GlobalSearchPopup from '@renderer/components/GlobalSearch/GlobalSearchPopup'
import { getSidebarLayout, type SidebarVisibleLayout } from '@renderer/components/Sidebar'
import { SidebarTooltip } from '@renderer/components/Sidebar/Tooltip'
import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { useTabs } from '@renderer/hooks/tab'
import { useAppUpdateState } from '@renderer/hooks/useAppUpdateState'
import { closeAppletLaunchpad, OrgAppletLaunchpad, type LaunchpadPhase } from '@renderer/pages/org/OrgAppletLaunchpad'
import { openSettingsTab } from '@renderer/services/mainWindowNavigation'

import { WindowControls } from '../WindowControls'
import { WorkInboxButton } from '../workInbox/WorkInboxPopup'

const logger = loggerService.withContext('ShellTabBarActions')

export function ShellTabBarActions() {
  const { t } = useTranslation()
  const [sidebarWidth] = usePersistCache('ui.sidebar.width')
  const { appUpdateState } = useAppUpdateState()
  const isSidebarHidden = getSidebarLayout(sidebarWidth) === 'hidden'
  const hasUpdateAction = Boolean(appUpdateState.available && appUpdateState.downloaded && appUpdateState.info)

  const handleSearchClick = () => {
    void GlobalSearchPopup.show()
  }

  const handleSettingsClick = () => {
    openSettingsTab()
  }

  const handleDiagnosticsClick = () => {
    void DoctorPopup.show({ initialPanel: 'checks' })
  }

  const handleUpdateClick = () => {
    const releaseInfo = appUpdateState.info
    if (!releaseInfo) return

    void import('@renderer/components/UpdateDialogPopup')
      .then(({ default: UpdateDialogPopup }) => UpdateDialogPopup.show({ releaseInfo }))
      .catch((error) => logger.error('Failed to open update dialog', error as Error))
  }

  const updateLabel = appUpdateState.info
    ? t('settings.about.updateAvailable', { version: appUpdateState.info.version })
    : t('button.update_available')

  return (
    <div className="pointer-events-none relative z-[1] flex h-full shrink-0 items-stretch [&_button]:pointer-events-auto">
      <div className="flex items-center gap-1 pr-2 [-webkit-app-region:no-drag]">
        {hasUpdateAction && (
          <Tooltip content={updateLabel} placement="bottom" delay={800}>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={updateLabel}
              onClick={handleUpdateClick}
              className="flex h-8 w-8 items-center justify-center rounded-[8px] transition-colors hover:bg-accent">
              <CircleArrowUp className="lucide-custom size-[18px] text-success" strokeWidth={1.8} />
            </Button>
          </Tooltip>
        )}
        {isSidebarHidden ? (
          <>
            <Tooltip content={t('settings.doctor.entry.title')} placement="bottom" delay={800}>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t('settings.doctor.entry.title')}
                onClick={handleDiagnosticsClick}
                className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground dark:text-muted-foreground">
                <Stethoscope size={16} strokeWidth={1.8} />
              </Button>
            </Tooltip>
            <Tooltip content={t('settings.title')} placement="bottom" delay={800}>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t('settings.title')}
                onClick={handleSettingsClick}
                className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground dark:text-muted-foreground">
                <Settings size={16} strokeWidth={1.8} />
              </Button>
            </Tooltip>
          </>
        ) : null}
        <CommandTooltip command="app.search" label={t('globalSearch.open')} placement="bottom" delay={800}>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t('globalSearch.open')}
            onClick={handleSearchClick}
            className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground dark:text-muted-foreground">
            <Search size={16} strokeWidth={1.8} />
          </Button>
        </CommandTooltip>
      </div>

      <WindowControls />
    </div>
  )
}

function AppLaunchpadButton() {
  const { t } = useTranslation()
  const { openTab } = useTabs()
  const openLaunchpad = () => {
    openTab('/app/launchpad', { title: t('title.launchpad'), forceNew: true })
  }

  return (
    <Button
      type="button"
      variant="ghost"
      aria-label={t('title.launchpad')}
      data-launchpad-button
      onClick={openLaunchpad}
      className="flex w-full items-center justify-start gap-2.5 rounded-lg px-2.5 py-1.75 text-[13px] text-foreground transition-colors hover:bg-accent/60">
      <Rocket size={16} strokeWidth={1.6} />
      <span>{t('title.launchpad')}</span>
    </Button>
  )
}

function OrgWorkbenchButton({ layout, onOpen }: { layout: SidebarVisibleLayout; onOpen: (origin: string) => void }) {
  const { t } = useTranslation()
  const anchorRef = useRef<HTMLSpanElement>(null)
  const openWorkbench = () => {
    const rect = anchorRef.current?.getBoundingClientRect()
    onOpen(rect ? `${rect.left + rect.width / 2}px ${rect.top + rect.height / 2}px` : '50% 72%')
  }

  const button =
    layout === 'icon' ? (
      <SidebarTooltip content={t('org.nav.workbench')}>
        <Button
          type="button"
          variant="ghost"
          size="icon-lg"
          aria-label={t('org.nav.workbench')}
          data-org-workbench-button
          onClick={openWorkbench}
          className="activity-bar-item size-9 bg-transparent text-muted-foreground shadow-none hover:bg-transparent hover:text-foreground dark:text-muted-foreground [&_svg]:!size-[1.375rem]">
          <LayoutDashboard size={22} strokeWidth={1.6} />
        </Button>
      </SidebarTooltip>
    ) : (
      <Button
        type="button"
        variant="ghost"
        aria-label={t('org.nav.workbench')}
        data-org-workbench-button
        onClick={openWorkbench}
        className="flex w-full items-center justify-start gap-2.5 rounded-lg px-2.5 py-1.75 text-[13px] text-foreground transition-colors hover:bg-accent/60">
        <LayoutDashboard size={16} strokeWidth={1.6} />
        <span>{t('org.nav.workbench')}</span>
      </Button>
    )

  return (
    <span ref={anchorRef} className={layout === 'icon' ? 'inline-flex' : 'block w-full'}>
      {button}
    </span>
  )
}

export function SidebarShellActions({
  layout,
  onSettingsClick
}: {
  layout: SidebarVisibleLayout
  onSettingsClick: () => void
}) {
  const { t } = useTranslation()
  const orgId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const [launcherPhase, setLauncherPhase] = useState<LaunchpadPhase>('closed')
  const [launchOrigin, setLaunchOrigin] = useState('50% 72%')
  const closeLauncher = () => setLauncherPhase((current) => closeAppletLaunchpad(current))
  const openLauncher = (origin: string) => {
    setLaunchOrigin(origin)
    setLauncherPhase('open')
  }

  return (
    <>
      {layout === 'icon' ? (
        <>
          <OrgWorkbenchButton layout="icon" onOpen={openLauncher} />
          <WorkInboxButton layout="icon" />
          <SidebarTooltip content={t('settings.title')}>
            <Button
              type="button"
              variant="ghost"
              size="icon-lg"
              aria-label={t('settings.title')}
              onClick={onSettingsClick}
              className="activity-bar-item size-9 bg-transparent text-muted-foreground shadow-none hover:bg-transparent hover:text-foreground dark:text-muted-foreground [&_svg]:!size-[1.375rem]">
              <Settings size={22} strokeWidth={1.6} />
            </Button>
          </SidebarTooltip>
        </>
      ) : (
        <>
          <AppLaunchpadButton />
          <OrgWorkbenchButton layout="full" onOpen={openLauncher} />
          <WorkInboxButton layout="full" />
          <Button
            type="button"
            variant="ghost"
            aria-label={t('settings.title')}
            onClick={onSettingsClick}
            className="flex w-full items-center justify-start gap-2.5 rounded-lg px-2.5 py-1.75 text-[13px] text-foreground transition-colors hover:bg-accent/60">
            <Settings size={16} strokeWidth={1.6} />
            <span>{t('settings.title')}</span>
          </Button>
        </>
      )}
      <OrgAppletLaunchpad
        orgId={orgId}
        phase={launcherPhase}
        origin={launchOrigin}
        onClose={closeLauncher}
        onExited={() => setLauncherPhase((current) => (current === 'closing' ? 'closed' : current))}
      />
    </>
  )
}
