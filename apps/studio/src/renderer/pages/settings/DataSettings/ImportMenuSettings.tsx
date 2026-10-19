import { Button, RowFlex } from '@a3s-lab/ui'
import type { FC } from 'react'
import { useTranslation } from 'react-i18next'

import {
  SettingDivider,
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'

import { dataQuietButtonClassName } from './dataChrome'
import ImportPopup from './ImportPopup'

const ImportMenuOptions: FC = () => {
  const { t } = useTranslation()
  const { theme } = useTheme()
  return (
    <SettingGroup theme={theme}>
      <SettingRow>
        <SettingTitle>{t('settings.data.import_settings.title')}</SettingTitle>
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-data-import-chatgpt" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.data.import_settings.chatgpt')}</SettingRowTitle>
        <RowFlex className="justify-between gap-1.25">
          <Button
            onClick={() => ImportPopup.show({ source: 'chatgpt' })}
            variant="outline"
            className={dataQuietButtonClassName}>
            {t('settings.data.import_settings.button')}
          </Button>
        </RowFlex>
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-data-import-claude" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.data.import_settings.claude')}</SettingRowTitle>
        <RowFlex className="justify-between gap-1.25">
          <Button
            onClick={() => ImportPopup.show({ source: 'claude' })}
            variant="outline"
            className={dataQuietButtonClassName}>
            {t('settings.data.import_settings.button')}
          </Button>
        </RowFlex>
      </SettingRow>
    </SettingGroup>
  )
}

export default ImportMenuOptions
