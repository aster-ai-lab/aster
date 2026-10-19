import type { FC } from 'react'
import { useTranslation } from 'react-i18next'

import { Switch } from '@a3s-lab/ui/components'
import { useMultiplePreferences, usePreference } from '@data/hooks/usePreference'
import {
  SettingDivider,
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingsContentColumn,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'

import { ContextManagementSettings } from './ContextManagementSettings'
import { WorkspaceSettings } from './WorkspaceSettings'

const TRAY_PREFERENCE_KEYS = {
  enabled: 'app.tray.enabled',
  onClose: 'app.tray.on_close',
  onLaunch: 'app.tray.on_launch',
  clickTrayToShowQuickAssistant: 'feature.quick_assistant.click_tray_to_show'
} as const

const GeneralSettings: FC = () => {
  const { t } = useTranslation()
  const { theme } = useTheme()

  const [launchOnBoot, setLaunchOnBoot] = usePreference('app.launch_on_boot')
  const [trayPreferences, setTrayPreferences] = useMultiplePreferences(TRAY_PREFERENCE_KEYS)
  const { enabled: tray, onClose: trayOnClose, onLaunch: launchToTray } = trayPreferences
  const [preventSleepWhenBusy, setPreventSleepWhenBusy] = usePreference('app.power.prevent_sleep_when_busy')

  const updateTray = (isShowTray: boolean) => {
    void setTrayPreferences(
      isShowTray
        ? { enabled: true }
        : { enabled: false, onClose: false, onLaunch: false, clickTrayToShowQuickAssistant: false }
    )
  }

  const updateTrayOnClose = (isTrayOnClose: boolean) => {
    void setTrayPreferences(isTrayOnClose && !tray ? { enabled: true, onClose: true } : { onClose: isTrayOnClose })
  }

  const updateLaunchToTray = (isLaunchToTray: boolean) => {
    void setTrayPreferences(isLaunchToTray && !tray ? { enabled: true, onLaunch: true } : { onLaunch: isLaunchToTray })
  }

  return (
    <SettingsContentColumn theme={theme}>
      <SettingGroup theme={theme}>
        <SettingTitle>{t('settings.launch.title')}</SettingTitle>
        <SettingDivider />
        <SettingRow id="setting-general-launch-onboot" className="scroll-mt-6">
          <SettingRowTitle>{t('settings.launch.onboot')}</SettingRowTitle>
          <Switch checked={launchOnBoot} onCheckedChange={(checked) => void setLaunchOnBoot(checked)} />
        </SettingRow>
        <SettingDivider />
        <SettingRow id="setting-general-launch-totray" className="scroll-mt-6">
          <SettingRowTitle>{t('settings.launch.totray')}</SettingRowTitle>
          <Switch checked={launchToTray} onCheckedChange={(checked) => updateLaunchToTray(checked)} />
        </SettingRow>
        <SettingDivider />
        <SettingRow id="setting-general-tray-show" className="scroll-mt-6">
          <SettingRowTitle>{t('settings.tray.show')}</SettingRowTitle>
          <Switch checked={tray} onCheckedChange={(checked) => updateTray(checked)} />
        </SettingRow>
        <SettingDivider />
        <SettingRow id="setting-general-tray-onclose" className="scroll-mt-6">
          <SettingRowTitle>{t('settings.tray.onclose')}</SettingRowTitle>
          <Switch checked={trayOnClose} onCheckedChange={(checked) => updateTrayOnClose(checked)} />
        </SettingRow>
        <SettingDivider />
        <SettingRow id="setting-general-prevent-sleep-when-busy" className="scroll-mt-6">
          <SettingRowTitle>{t('settings.power.prevent_sleep_when_busy')}</SettingRowTitle>
          <Switch checked={preventSleepWhenBusy} onCheckedChange={(checked) => void setPreventSleepWhenBusy(checked)} />
        </SettingRow>
      </SettingGroup>

      <WorkspaceSettings />
      <ContextManagementSettings />
    </SettingsContentColumn>
  )
}

export default GeneralSettings
