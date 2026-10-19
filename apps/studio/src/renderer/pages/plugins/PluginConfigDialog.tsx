import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, Switch } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { PLUGIN_CONFIG_EVENT } from '@renderer/services/plugins'
import { toast } from '@renderer/services/toast'
import { getVisibleSidebarShortcutItems } from '@renderer/utils/sidebar'
import { cn } from '@renderer/utils/style'

import type { CapabilityConfigField, CapabilityEntry } from '../capabilities/capabilitiesData'
import { loadConnectorConfig, readConnectorConfig, saveConnectorConfig } from '../capabilities/connectorConfig'

export { PLUGIN_CONFIG_EVENT }

const fieldClass =
  'h-9 w-full rounded-[8px] border border-transparent bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10 disabled:opacity-40'

function sameValues(left: Record<string, string>, right: Record<string, string>): boolean {
  const keys = new Set([...Object.keys(left), ...Object.keys(right)])
  for (const key of keys) {
    if ((left[key] ?? '') !== (right[key] ?? '')) return false
  }
  return true
}

export function PluginConfigDialog({
  entry,
  hasActivityBar = false,
  onOpenChange
}: {
  entry: CapabilityEntry | null
  hasActivityBar?: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useTranslation()
  return (
    <Dialog open={entry !== null} onOpenChange={onOpenChange}>
      {entry ? (
        <DialogContent
          closeLabel={t('common.close')}
          overlayClassName="!opacity-100"
          className="flex max-h-[min(640px,calc(100vh-4rem))] w-full max-w-[min(480px,calc(100%-2rem))] !animate-none flex-col gap-0 overflow-hidden p-0 !opacity-100 sm:max-w-[min(480px,calc(100%-2rem))]">
          <PluginConfigBody
            key={entry.slug}
            entry={entry}
            hasActivityBar={hasActivityBar}
            onClose={() => onOpenChange(false)}
          />
        </DialogContent>
      ) : null}
    </Dialog>
  )
}

function PluginConfigBody({
  entry,
  hasActivityBar,
  onClose
}: {
  entry: CapabilityEntry
  hasActivityBar: boolean
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [choices, setChoices] = usePreference('plugins.activity_bar', { optimistic: false })
  const [shortcuts] = usePreference('ui.sidebar_shortcut')
  const pluginId = `aix.${entry.slug}`
  const configured =
    getVisibleSidebarShortcutItems(shortcuts).some(
      (item) =>
        item.target.locator.providerId === 'studio.plugin-page' &&
        item.target.locator.resourceId.startsWith(`${pluginId}/`)
    ) ?? false
  const activityBaseline = choices?.[pluginId] ?? configured
  const [activityChoice, setActivityChoice] = useState<boolean | null>(null)
  const [values, setValues] = useState<Record<string, string>>(() => readConnectorConfig(entry.slug))
  const [baseline, setBaseline] = useState<Record<string, string>>(() => readConnectorConfig(entry.slug))
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [saving, setSaving] = useState(false)
  const dirtyRef = useRef(false)
  const configDirty = !sameValues(values, baseline)
  const activityDirty = hasActivityBar && activityChoice !== null && activityChoice !== activityBaseline
  const dirty = configDirty || activityDirty

  useEffect(() => {
    if (entry.config.length === 0) return undefined
    let cancelled = false
    void loadConnectorConfig(entry.slug).then((next) => {
      if (cancelled || dirtyRef.current) return
      setValues(next)
      setBaseline(next)
    })
    return () => {
      cancelled = true
    }
  }, [entry.slug, entry.config.length])

  const save = async () => {
    const missing = entry.config.filter((field) => field.required && !(values[field.key] ?? '').trim())
    if (missing.length > 0) {
      setErrors(
        Object.fromEntries(missing.map((field) => [field.key, t('capabilities.config.missing', { name: field.key })]))
      )
      return
    }
    setSaving(true)
    try {
      const clean = Object.fromEntries(
        entry.config.map((field) => [field.key, (values[field.key] ?? '').trim()]).filter(([, value]) => value)
      )
      if (configDirty) await saveConnectorConfig(entry.slug, clean)
      if (activityDirty) await setChoices({ ...choices, [pluginId]: activityChoice })
      dirtyRef.current = false
      setBaseline(clean)
      setValues(clean)
      globalThis.window.dispatchEvent(new CustomEvent(PLUGIN_CONFIG_EVENT, { detail: { slug: entry.slug } }))
      toast.success(t('common.saved'))
      onClose()
    } catch (error) {
      toast.error(
        t('capabilities.config.save_failed', {
          reason: error instanceof Error && error.message ? error.message : 'HTTP error'
        })
      )
    } finally {
      setSaving(false)
    }
  }

  return (
    <form
      data-ui="plugins.config.dialog"
      className="flex min-h-0 flex-1 flex-col"
      onSubmit={(event) => {
        event.preventDefault()
        void save()
      }}>
      <DialogHeader className="shrink-0 gap-2 px-6 pt-5 pr-12 pb-4 text-left">
        <DialogTitle className="truncate text-[15px] leading-none font-medium text-[#171717]">
          {t('plugins.okf.configure')}
        </DialogTitle>
        <DialogDescription className="m-0 text-[13px] leading-5 text-[#3f3f3f]">{entry.name}</DialogDescription>
      </DialogHeader>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-6 pb-5">
        {hasActivityBar ? (
          <div className="flex items-center justify-between gap-4 rounded-[8px] bg-[#f5f5f5] p-3">
            <div>
              <label htmlFor="plugin-activity-bar" className="text-[13px] font-medium text-[#171717]">
                {t('plugins.activity_bar.label')}
              </label>
              <p className="m-0 mt-1 text-[12px] leading-5 text-[#8b8b8b]">{t('plugins.activity_bar.description')}</p>
            </div>
            <Switch
              id="plugin-activity-bar"
              checked={activityChoice ?? activityBaseline}
              disabled={saving}
              onCheckedChange={setActivityChoice}
            />
          </div>
        ) : null}
        {entry.config.length > 0 ? (
          <p className="m-0 text-[12px] leading-5 text-[#8b8b8b]">{t('capabilities.config.local')}</p>
        ) : null}
        {entry.config.map((field, index) => (
          <ConfigField
            key={field.key}
            field={field}
            value={values[field.key] ?? ''}
            error={errors[field.key]}
            autoFocus={index === 0}
            disabled={saving}
            onChange={(value) => {
              dirtyRef.current = true
              setValues((current) => ({ ...current, [field.key]: value }))
              setErrors((current) => {
                if (!current[field.key]) return current
                const next = { ...current }
                delete next[field.key]
                return next
              })
            }}
          />
        ))}
      </div>
      <div className="flex shrink-0 justify-end gap-2 border-t border-[#efefef] px-6 py-4">
        <button
          type="button"
          disabled={saving}
          onClick={onClose}
          className="inline-flex h-9 items-center justify-center rounded-[8px] border border-[#e5e5e5] bg-white px-4 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          {t('common.cancel')}
        </button>
        <button
          type="submit"
          disabled={saving || !dirty}
          className="inline-flex h-9 items-center justify-center rounded-[8px] bg-[#171717] px-4 text-[13px] text-white outline-none hover:bg-[#262626] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          {t('common.save')}
        </button>
      </div>
    </form>
  )
}

function ConfigField({
  field,
  value,
  error,
  autoFocus,
  disabled,
  onChange
}: {
  field: CapabilityConfigField
  value: string
  error?: string
  autoFocus: boolean
  disabled: boolean
  onChange: (value: string) => void
}) {
  const id = `plugin-config-${field.key}`
  const errorId = `${id}-error`
  return (
    <div>
      <label htmlFor={id} className="mb-1.5 block truncate text-[13px] font-medium text-[#171717]">
        {field.key}
      </label>
      <input
        id={id}
        autoFocus={autoFocus}
        disabled={disabled}
        value={value}
        type={field.secret ? 'password' : 'text'}
        autoComplete="off"
        spellCheck={false}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        onChange={(event) => onChange(event.target.value)}
        className={cn(fieldClass)}
      />
      {field.description ? <p className="m-0 mt-1 text-[12px] leading-5 text-[#8b8b8b]">{field.description}</p> : null}
      {error ? (
        <p id={errorId} className="m-0 mt-1 text-[12px] leading-5 text-[#b42318]">
          {error}
        </p>
      ) : null}
    </div>
  )
}
