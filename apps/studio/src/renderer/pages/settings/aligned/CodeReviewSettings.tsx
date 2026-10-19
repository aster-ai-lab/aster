import { SearchSelect } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import {
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingsContentColumn
} from '@renderer/components/SettingsPrimitives'

import { CODE_REVIEW_MODES, getCodeReviewMode, setCodeReviewMode, type CodeReviewMode } from './codeReviewMode'

export function CodeReviewSettings() {
  const { t } = useTranslation()
  const [storedMode, setStoredMode] = usePreference('app.code_review.mode')
  const [mode, setMode] = useState<CodeReviewMode>(getCodeReviewMode)

  useEffect(() => {
    if (storedMode === 'inline' || storedMode === 'subagent') setMode(storedMode)
  }, [storedMode])

  return (
    <SettingsContentColumn>
      <SettingGroup id="setting-code-review-mode">
        <SettingRow>
          <SettingRowTitle>{t('settings.aligned.review.mode')}</SettingRowTitle>
          <SearchSelect
            size="sm"
            searchable={false}
            allowClear={false}
            aria-label={t('settings.aligned.review.mode')}
            value={mode}
            options={[
              { value: 'inline', label: t('settings.aligned.review.inline') },
              { value: 'subagent', label: t('settings.aligned.review.subagent') }
            ]}
            onChange={(raw) => {
              const next = raw as CodeReviewMode
              if (!CODE_REVIEW_MODES.includes(next)) return
              setMode(next)
              void setStoredMode(next)
              void setCodeReviewMode(next)
            }}
            className="w-[180px]"
          />
        </SettingRow>
      </SettingGroup>
    </SettingsContentColumn>
  )
}
