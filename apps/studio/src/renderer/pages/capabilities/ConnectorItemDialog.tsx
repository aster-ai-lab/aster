import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@a3s-lab/ui'
import { Eye, EyeOff } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  capabilityCategoryLabelKey,
  type CapabilityConfigField,
  type CapabilityEntry,
  type CapabilityInstall
} from './capabilitiesData'
import { CapabilityMark } from './capabilityMark'
import { clearConnectorConfig, loadConnectorConfig, readConnectorConfig, saveConnectorConfig } from './connectorConfig'

const fieldClass =
  'h-9 w-full rounded-[8px] border border-transparent bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10 disabled:opacity-40'

function connectionText(install: CapabilityInstall | undefined): string {
  if (!install) return ''
  if (install.transport === 'stdio') return [install.command, ...(install.args ?? [])].filter(Boolean).join(' ')
  return install.url ?? ''
}

function transportKey(install: CapabilityInstall | undefined): string | undefined {
  if (!install) return undefined
  if (install.transport === 'stdio') return 'capabilities.connection.stdio'
  if (install.transport === 'sse') return 'capabilities.connection.sse'
  return 'capabilities.connection.http'
}

function sameValues(left: Record<string, string>, right: Record<string, string>): boolean {
  const keys = new Set([...Object.keys(left), ...Object.keys(right)])
  for (const key of keys) {
    if ((left[key] ?? '') !== (right[key] ?? '')) return false
  }
  return true
}

export function ConnectorItemDialog({
  entry,
  pending,
  onOpenChange,
  onInstall,
  onUninstall
}: {
  entry: CapabilityEntry | null
  pending: boolean
  onOpenChange: (open: boolean) => void
  onInstall: (entry: CapabilityEntry) => Promise<boolean>
  onUninstall: (entry: CapabilityEntry) => Promise<boolean>
}) {
  const { t } = useTranslation()
  const open = entry !== null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {entry ? (
        <DialogContent
          closeLabel={t('common.close')}
          overlayClassName="!opacity-100"
          className="flex max-h-[min(640px,calc(100vh-4rem))] w-full max-w-[min(480px,calc(100%-2rem))] !animate-none flex-col gap-0 overflow-hidden p-0 !opacity-100 sm:max-w-[min(480px,calc(100%-2rem))]">
          <ConnectorItemBody
            key={`${entry.slug}:${entry.installed ? 'on' : 'off'}`}
            entry={entry}
            pending={pending}
            onInstall={onInstall}
            onUninstall={onUninstall}
          />
        </DialogContent>
      ) : null}
    </Dialog>
  )
}

function ConnectorItemBody({
  entry,
  pending,
  onInstall,
  onUninstall
}: {
  entry: CapabilityEntry
  pending: boolean
  onInstall: (entry: CapabilityEntry) => Promise<boolean>
  onUninstall: (entry: CapabilityEntry) => Promise<boolean>
}) {
  const { t } = useTranslation()
  const categoryKey = capabilityCategoryLabelKey(entry.category)
  const transport = transportKey(entry.install)
  const connection = connectionText(entry.install)
  const status = [
    entry.installed ? t('capabilities.installed') : t('capabilities.tab.connectors'),
    categoryKey ? t(categoryKey) : entry.category,
    transport ? t(transport) : ''
  ]
    .filter(Boolean)
    .join(' · ')

  return (
    <div data-ui="capabilities.item-dialog" className="flex min-h-0 flex-1 flex-col">
      <DialogHeader className="shrink-0 gap-3 px-6 pt-5 pr-12 pb-4 text-left">
        <div className="flex items-center gap-3">
          <CapabilityMark entry={entry} />
          <div className="min-w-0">
            <DialogTitle className="truncate text-[15px] leading-none font-medium text-[#171717]">
              {entry.name}
            </DialogTitle>
            <p className="m-0 mt-1.5 truncate text-[12px] leading-4 text-[#8b8b8b]">{status}</p>
          </div>
        </div>
        {entry.description ? (
          <DialogDescription className="m-0 text-[13px] leading-5 text-[#3f3f3f]">
            {entry.description}
          </DialogDescription>
        ) : (
          <DialogDescription className="sr-only">{entry.name}</DialogDescription>
        )}
      </DialogHeader>
      {entry.installed ? (
        <ConfigureForm entry={entry} pending={pending} connection={connection} onUninstall={onUninstall} />
      ) : (
        <InstallPreview entry={entry} pending={pending} connection={connection} onInstall={onInstall} />
      )}
    </div>
  )
}

function InstallPreview({
  entry,
  pending,
  connection,
  onInstall
}: {
  entry: CapabilityEntry
  pending: boolean
  connection: string
  onInstall: (entry: CapabilityEntry) => Promise<boolean>
}) {
  const { t } = useTranslation()
  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pb-5">
        {connection ? <ConnectionBlock text={connection} /> : null}
        {entry.config.length > 0 ? (
          <div>
            <h3 className="m-0 mb-2 text-[13px] font-medium text-[#171717]">
              {t('capabilities.config.install_needs')}
            </h3>
            <ul className="m-0 flex list-none flex-col gap-2 p-0">
              {entry.config.map((field) => (
                <li key={field.key} className="text-[13px] leading-5 text-[#3f3f3f]">
                  <span className="font-medium text-[#171717]">{field.key}</span>
                  {field.required ? (
                    <span className="ml-2 text-[12px] text-[#8b8b8b]">{t('capabilities.config.required')}</span>
                  ) : null}
                  {field.secret ? (
                    <span className="ml-2 text-[12px] text-[#8b8b8b]">{t('capabilities.config.secret')}</span>
                  ) : null}
                  {field.description && field.description !== field.key ? (
                    <span className="mt-0.5 block text-[12px] leading-5 text-[#8b8b8b]">{field.description}</span>
                  ) : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
      <div className="flex shrink-0 justify-end border-t border-[#efefef] px-6 py-4">
        <button
          type="button"
          disabled={pending}
          aria-busy={pending || undefined}
          onClick={() => void onInstall(entry)}
          className="inline-flex h-9 items-center justify-center rounded-[8px] bg-[#171717] px-4 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          {pending ? t('capabilities.installing') : t('capabilities.install')}
        </button>
      </div>
    </>
  )
}

function ConfigureForm({
  entry,
  pending,
  connection,
  onUninstall
}: {
  entry: CapabilityEntry
  pending: boolean
  connection: string
  onUninstall: (entry: CapabilityEntry) => Promise<boolean>
}) {
  const { t } = useTranslation()
  const [values, setValues] = useState<Record<string, string>>(() => readConnectorConfig(entry.slug))
  const [baseline, setBaseline] = useState<Record<string, string>>(() => readConnectorConfig(entry.slug))
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [revealed, setRevealed] = useState<Record<string, boolean>>({})
  const [saving, setSaving] = useState(false)
  const [confirming, setConfirming] = useState(false)
  const dirtyRef = useRef(false)
  const fields = entry.config
  const dirty = !sameValues(values, baseline)

  useEffect(() => {
    let cancelled = false
    void loadConnectorConfig(entry.slug).then((next) => {
      if (cancelled || dirtyRef.current) return
      setValues(next)
      setBaseline(next)
    })
    return () => {
      cancelled = true
    }
  }, [entry.slug])

  const setField = (key: string, value: string) => {
    dirtyRef.current = true
    setValues((current) => ({ ...current, [key]: value }))
    setErrors((current) => {
      if (!current[key]) return current
      const next = { ...current }
      delete next[key]
      return next
    })
  }

  const save = async () => {
    const missing = fields.filter((field) => field.required && !(values[field.key] ?? '').trim())
    if (missing.length > 0) {
      setErrors(
        Object.fromEntries(missing.map((field) => [field.key, t('capabilities.config.missing', { name: field.key })]))
      )
      return
    }
    setSaving(true)
    try {
      const clean = Object.fromEntries(
        fields.map((field) => [field.key, (values[field.key] ?? '').trim()]).filter(([, value]) => value)
      )
      await saveConnectorConfig(entry.slug, clean)
      dirtyRef.current = false
      setValues(clean)
      setBaseline(clean)
      setErrors({})
      toast.success(t('common.saved'))
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

  const uninstall = async () => {
    const removed = await onUninstall(entry)
    if (!removed) return
    await clearConnectorConfig(entry.slug)
    setConfirming(false)
  }

  return (
    <>
      <form
        className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pb-5"
        onSubmit={(event) => {
          event.preventDefault()
          void save()
        }}>
        {fields.length > 0 ? (
          <div className="flex flex-col gap-3">
            <div className="flex items-baseline justify-between gap-3">
              <h3 className="m-0 text-[13px] font-medium text-[#171717]">{t('capabilities.config')}</h3>
              <span className="text-[12px] text-[#8b8b8b]">{t('capabilities.config.local')}</span>
            </div>
            {fields.map((field, index) => (
              <ConfigField
                key={field.key}
                field={field}
                value={values[field.key] ?? ''}
                error={errors[field.key]}
                revealed={revealed[field.key] === true}
                autoFocus={index === 0}
                disabled={pending || saving}
                onChange={(value) => setField(field.key, value)}
                onToggle={() => setRevealed((current) => ({ ...current, [field.key]: !current[field.key] }))}
              />
            ))}
          </div>
        ) : (
          <p className="m-0 text-[13px] leading-5 text-[#8b8b8b]">{t('capabilities.config.empty')}</p>
        )}
        {connection ? <ConnectionBlock text={connection} /> : null}
      </form>
      <div className="flex shrink-0 items-center justify-between gap-3 border-t border-[#efefef] px-6 py-4">
        {confirming ? (
          <>
            <p className="m-0 min-w-0 flex-1 text-[13px] text-[#171717]">
              {t('capabilities.uninstall_confirm', { name: entry.name })}
            </p>
            <div className="flex shrink-0 gap-2">
              <QuietButton disabled={pending} onClick={() => setConfirming(false)}>
                {t('common.cancel')}
              </QuietButton>
              <button
                type="button"
                disabled={pending}
                onClick={() => void uninstall()}
                className="inline-flex h-9 items-center justify-center rounded-[8px] border border-[#e5e5e5] bg-white px-4 text-[13px] text-[#b42318] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                {t('capabilities.uninstall')}
              </button>
            </div>
          </>
        ) : (
          <>
            <QuietButton disabled={pending || saving} onClick={() => setConfirming(true)}>
              {t('capabilities.uninstall')}
            </QuietButton>
            {fields.length > 0 ? (
              <button
                type="button"
                disabled={pending || saving || !dirty}
                onClick={() => void save()}
                className="inline-flex h-9 items-center justify-center rounded-[8px] bg-[#171717] px-4 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                {t('common.save')}
              </button>
            ) : (
              <span />
            )}
          </>
        )}
      </div>
    </>
  )
}

function ConfigField({
  field,
  value,
  error,
  revealed,
  autoFocus,
  disabled,
  onChange,
  onToggle
}: {
  field: CapabilityConfigField
  value: string
  error?: string
  revealed: boolean
  autoFocus: boolean
  disabled: boolean
  onChange: (value: string) => void
  onToggle: () => void
}) {
  const { t } = useTranslation()
  const id = `connector-config-${field.key}`
  const errorId = `${id}-error`
  const secret = field.secret && !revealed
  return (
    <div>
      <label htmlFor={id} className="mb-1.5 flex items-center gap-2 text-[13px] text-[#171717]">
        <span className="min-w-0 truncate font-medium">{field.key}</span>
        {field.required ? (
          <span className="shrink-0 text-[12px] font-normal text-[#8b8b8b]">{t('capabilities.config.required')}</span>
        ) : null}
      </label>
      <div className="relative">
        <input
          id={id}
          autoFocus={autoFocus}
          disabled={disabled}
          value={value}
          type={secret ? 'password' : 'text'}
          autoComplete="off"
          spellCheck={false}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          onChange={(event) => onChange(event.target.value)}
          className={cn(fieldClass, field.secret && 'pr-10')}
        />
        {field.secret ? (
          <button
            type="button"
            disabled={disabled}
            aria-label={t(revealed ? 'capabilities.config.hide' : 'capabilities.config.show', { name: field.key })}
            onClick={onToggle}
            className="absolute top-1/2 right-1.5 grid size-7 -translate-y-1/2 place-items-center rounded-[6px] text-[#8a8a8a] outline-none hover:bg-[#ececec] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
            {revealed ? (
              <EyeOff className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
            ) : (
              <Eye className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
            )}
          </button>
        ) : null}
      </div>
      {field.description && field.description !== field.key ? (
        <p className="m-0 mt-1 text-[12px] leading-5 text-[#8b8b8b]">{field.description}</p>
      ) : null}
      {error ? (
        <p id={errorId} className="m-0 mt-1 text-[12px] leading-5 text-[#b42318]">
          {error}
        </p>
      ) : null}
    </div>
  )
}

function ConnectionBlock({ text }: { text: string }) {
  const { t } = useTranslation()
  return (
    <div>
      <h3 className="m-0 mb-1.5 text-[13px] font-medium text-[#171717]">{t('capabilities.connection')}</h3>
      <p className="m-0 rounded-[8px] bg-[#f5f5f5] px-3 py-2 text-[13px] leading-5 break-all text-[#3f3f3f]">{text}</p>
    </div>
  )
}

function QuietButton({ children, disabled, onClick }: { children: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="inline-flex h-9 items-center justify-center rounded-[8px] border border-[#e5e5e5] bg-white px-4 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
      {children}
    </button>
  )
}
