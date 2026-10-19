import { Button, InfoTooltip, Input, RowFlex, Switch } from '@a3s-lab/ui'
import type { FC } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { loggerService } from '@logger'
import {
  SettingDivider,
  SettingGroup,
  SettingHelpText,
  SettingRow,
  SettingRowTitle,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'
import { toast } from '@renderer/services/toast'
import { openExternalWebsite } from '@renderer/services/website'
import { formatErrorMessage } from '@renderer/utils/error'

import { dataCheckButtonClassName, dataFieldClassName, dataHelpClassName } from './dataChrome'

const logger = loggerService.withContext('JoplinSettings')

const JoplinSettings: FC = () => {
  const [joplinToken, setJoplinToken] = usePreference('data.integration.joplin.token')
  const [joplinUrl, setJoplinUrl] = usePreference('data.integration.joplin.url')
  const [joplinExportReasoning, setJoplinExportReasoning] = usePreference('data.integration.joplin.export_reasoning')

  const { t } = useTranslation()
  const { theme } = useTheme()

  const handleJoplinTokenChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    void setJoplinToken(e.target.value)
  }

  const handleJoplinUrlChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    void setJoplinUrl(e.target.value)
  }

  const handleJoplinUrlBlur = (e: React.FocusEvent<HTMLInputElement>) => {
    let url = e.target.value
    if (url && !url.endsWith('/')) {
      url = `${url}/`
      void setJoplinUrl(url)
    }
  }

  const handleJoplinConnectionCheck = async () => {
    try {
      if (!joplinToken) {
        toast.error(t('settings.data.joplin.check.empty_token'))
        return
      }
      if (!joplinUrl) {
        toast.error(t('settings.data.joplin.check.empty_url'))
        return
      }

      const response = await fetch(`${joplinUrl}notes?limit=1&token=${joplinToken}`)

      const data = await response.json()

      if (!response.ok || data?.error) {
        toast.error(t('settings.data.joplin.check.fail'))
        return
      }

      toast.success(t('settings.data.joplin.check.success'))
    } catch (error) {
      logger.error('Failed to check Joplin connection', error as Error)
      toast.error(`${t('settings.data.joplin.check.fail')}: ${formatErrorMessage(error)}`)
    }
  }

  const handleToggleJoplinExportReasoning = (checked: boolean) => {
    void setJoplinExportReasoning(checked)
  }

  const handleJoplinHelpClick = () => {
    void openExternalWebsite('https://joplinapp.org/help/apps/clipper')
  }

  return (
    <SettingGroup theme={theme}>
      <SettingTitle>{t('settings.data.joplin.title')}</SettingTitle>
      <SettingDivider />
      <SettingRow id="setting-data-joplin-url" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.data.joplin.url')}</SettingRowTitle>
        <RowFlex className="max-w-full min-w-0 items-center gap-1.25">
          <Input
            type="text"
            value={joplinUrl || ''}
            onChange={handleJoplinUrlChange}
            onBlur={handleJoplinUrlBlur}
            className={dataFieldClassName}
            placeholder={t('settings.data.joplin.url_placeholder')}
          />
        </RowFlex>
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-data-joplin-token" className="scroll-mt-6">
        <SettingRowTitle className="flex items-center">
          <span>{t('settings.data.joplin.token')}</span>
          <InfoTooltip
            content={t('settings.data.joplin.help')}
            placement="left"
            iconProps={{ className: 'text-text-2 cursor-pointer ml-1' }}
            onClick={handleJoplinHelpClick}
          />
        </SettingRowTitle>
        <RowFlex className="max-w-full min-w-0 items-center gap-1.25">
          <Input
            type="password"
            value={joplinToken || ''}
            onChange={handleJoplinTokenChange}
            onBlur={handleJoplinTokenChange}
            placeholder={t('settings.data.joplin.token_placeholder')}
            className={dataFieldClassName}
          />
          <Button onClick={handleJoplinConnectionCheck} variant="outline" className={dataCheckButtonClassName}>
            {t('settings.data.joplin.check.button')}
          </Button>
        </RowFlex>
      </SettingRow>
      <SettingDivider />
      <SettingRow>
        <SettingRowTitle>{t('settings.data.joplin.export_reasoning.title')}</SettingRowTitle>
        <Switch checked={joplinExportReasoning} onCheckedChange={handleToggleJoplinExportReasoning} />
      </SettingRow>
      <SettingRow>
        <SettingHelpText className={dataHelpClassName}>
          {t('settings.data.joplin.export_reasoning.help')}
        </SettingHelpText>
      </SettingRow>
    </SettingGroup>
  )
}

export default JoplinSettings
