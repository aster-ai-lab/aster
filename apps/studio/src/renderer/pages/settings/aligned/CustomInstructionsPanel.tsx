import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { Button } from '@a3s-lab/ui'
import { SettingDescription, SettingGroup, SettingTitle } from '@renderer/components/SettingsPrimitives'

import { settingsFieldClassName } from './controlClass'
import { CustomInstructionTooLargeError, loadCustomInstructions, saveCustomInstructions } from './customInstructions'

/** Loads, edits, and saves the device custom instructions. A failed retry keeps the draft. */
export function CustomInstructionsPanel() {
  const { t } = useTranslation()
  const [draft, setDraft] = useState('')
  const [dirty, setDirty] = useState(false)
  const [loadError, setLoadError] = useState(false)
  const [saveError, setSaveError] = useState<'size' | 'save' | null>(null)
  const [saving, setSaving] = useState(false)

  const load = async () => {
    try {
      const text = await loadCustomInstructions()
      setLoadError(false)
      if (!dirty) setDraft(text)
    } catch {
      setLoadError(true)
    }
  }

  useEffect(() => {
    void load()
    // The first load is the page start. Later loads go through retry.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const retry = () => {
    void load()
  }

  const save = async () => {
    setSaving(true)
    setSaveError(null)
    try {
      await saveCustomInstructions(draft)
      setDirty(false)
    } catch (error) {
      setSaveError(error instanceof CustomInstructionTooLargeError ? 'size' : 'save')
    } finally {
      setSaving(false)
    }
  }

  return (
    <SettingGroup id="setting-prompts-custom-instructions" className="mb-4 shrink-0">
      <SettingTitle>{t('settings.aligned.custom.title')}</SettingTitle>
      <SettingDescription>{t('settings.aligned.custom.desc')}</SettingDescription>
      <textarea
        className={`${settingsFieldClassName} mt-3 min-h-28 py-2 leading-5`}
        aria-label={t('settings.aligned.custom.title')}
        value={draft}
        onChange={(event) => {
          setDraft(event.target.value)
          setDirty(true)
        }}
      />
      <div className="mt-3 flex justify-end">
        <Button type="button" size="sm" onClick={() => void save()} disabled={saving}>
          {t('settings.aligned.custom.save')}
        </Button>
      </div>
      {loadError ? (
        <p role="alert" className="mt-2 text-[13px] text-[#b42318]">
          {t('settings.aligned.custom.load_failed')}
          <Button type="button" size="sm" variant="outline" className="ml-2" onClick={retry}>
            {t('settings.aligned.custom.retry')}
          </Button>
        </p>
      ) : null}
      {saveError === 'size' ? (
        <p role="alert" className="mt-2 text-[13px] text-[#b42318]">
          {t('settings.aligned.custom.too_large')}
        </p>
      ) : null}
      {saveError === 'save' ? (
        <p role="alert" className="mt-2 text-[13px] text-[#b42318]">
          {t('settings.aligned.custom.save_failed')}
        </p>
      ) : null}
    </SettingGroup>
  )
}
