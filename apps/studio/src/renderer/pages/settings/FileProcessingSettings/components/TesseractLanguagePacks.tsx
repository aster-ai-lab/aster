import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'

import { Combobox, type ComboboxOption } from '@a3s-lab/ui'
import { SettingRow, SettingRowTitle } from '@renderer/components/SettingsPrimitives'

type TesseractLanguagePacksProps = {
  options: ComboboxOption[]
  selectedLanguages: string[]
  onChange: (value: string | string[]) => void
}

export function TesseractLanguagePacks({ options, selectedLanguages, onChange }: TesseractLanguagePacksProps) {
  const { t } = useTranslation()
  const renderSelectedLanguages = useCallback(
    (selectedValue: string | string[], availableOptions: ComboboxOption[]) => {
      const selectedValues = Array.isArray(selectedValue) ? selectedValue : []
      if (selectedValues.length === 0) return <span className="text-muted-foreground">{t('common.select')}</span>

      const firstValue = selectedValues[0]
      const firstOption = availableOptions.find((option) => option.value === firstValue)

      return (
        <div className="flex min-w-0 items-center gap-1">
          <span className="truncate rounded bg-secondary px-2 py-0.5 text-xs text-foreground">
            {firstOption?.label ?? firstValue}
          </span>
          {selectedValues.length > 1 && (
            <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
              +{selectedValues.length - 1}
            </span>
          )}
        </div>
      )
    },
    [t]
  )

  return (
    <div className="flex flex-col gap-3 border-t border-border-subtle pt-4">
      <SettingRow className="items-center gap-4 py-0">
        <SettingRowTitle className="w-24 shrink-0">
          {t('settings.tool.file_processing.fields.languages')}
        </SettingRowTitle>
        <div className="min-w-0 flex-1">
          <Combobox
            multiple
            width={220}
            value={selectedLanguages}
            options={options}
            onChange={onChange}
            renderValue={renderSelectedLanguages}
            searchable={false}
            placeholder={t('common.select')}
            emptyText={t('common.no_results')}
          />
        </div>
      </SettingRow>
    </div>
  )
}
