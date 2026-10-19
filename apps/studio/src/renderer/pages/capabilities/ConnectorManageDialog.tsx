import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@a3s-lab/ui'
import {
  Braces,
  Check,
  Globe,
  KeyRound,
  Link2,
  List,
  Plug,
  Plus,
  Radio,
  SquarePen,
  Tag,
  Terminal,
  type LucideIcon
} from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { MemberOrgError } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'

import { listCustomConnectors, removeCustomConnector, saveCustomConnector } from './capabilitiesData'
import { formatConnectorPaste, parseConnectorPaste } from './connectorPaste'
import {
  customConnectorBody,
  customConnectorIssues,
  type CustomConnectorBody,
  type CustomConnectorField,
  type CustomTransport
} from './customConnector'

const FIELD =
  'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-black/10'

const FIELD_LABEL = 'flex items-center gap-1.5 text-[12px] leading-4 text-[#8a8a8a]'

const TRANSPORTS: CustomTransport[] = ['stdio', 'sse', 'streamable_http']

type FieldRow = CustomConnectorField & { id: string }

type Draft = {
  name: string
  transport: CustomTransport
  command: string
  args: string
  url: string
  fields: FieldRow[]
  editing: boolean
}

const ERRORS = [
  'invalid_name',
  'missing_command',
  'missing_url',
  'unknown_transport',
  'connector_exists',
  'invalid_url',
  'invalid_config',
  'unavailable',
  'failed'
] as const

function blank(): Draft {
  return { name: '', transport: 'stdio', command: '', args: '', url: '', fields: [], editing: false }
}

function transportLabel(transport: CustomTransport): string {
  if (transport === 'stdio') return 'capabilities.connection.stdio'
  if (transport === 'sse') return 'capabilities.connection.sse'
  return 'capabilities.connection.http'
}

function errorKey(error: unknown): string {
  const code = error instanceof MemberOrgError ? error.message : ''
  return ERRORS.includes(code as (typeof ERRORS)[number]) ? code : 'failed'
}

const TRANSPORT_ICONS: Record<CustomTransport, LucideIcon> = {
  stdio: Terminal,
  sse: Radio,
  streamable_http: Globe
}

function FieldLabel({
  icon: Icon,
  children,
  className = 'mb-1.5'
}: {
  icon: LucideIcon
  children: ReactNode
  className?: string
}) {
  return (
    <span className={`${FIELD_LABEL} ${className}`}>
      <Icon aria-hidden className="size-4 shrink-0" strokeWidth={1.7} />
      {children}
    </span>
  )
}

function TransportIcon({ transport }: { transport: CustomTransport }) {
  const Icon = TRANSPORT_ICONS[transport]
  return <Icon aria-hidden className="size-4 shrink-0" strokeWidth={1.7} />
}

const TAB_TRIGGER =
  'gap-1.5 text-[13px] font-medium text-[#6f6f6f] focus-visible:bg-transparent focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset data-[state=active]:text-[#171717]'

function segmentClass(selected: boolean): string {
  return [
    'inline-flex h-7 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[6px] px-2 text-[13px] font-medium whitespace-nowrap outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-white text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.06)]' : 'text-[#6f6f6f] hover:text-[#171717]'
  ].join(' ')
}

export function ConnectorManageDialog({
  open,
  onOpenChange
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useTranslation()
  const fieldSeq = useRef(0)
  const busyRef = useRef(false)
  const jsonRef = useRef<HTMLTextAreaElement>(null)
  const focusJson = useRef(false)
  const selectSeed = useRef(false)
  const [rows, setRows] = useState<CustomConnectorBody[]>([])
  const [draft, setDraft] = useState<Draft>(blank)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [touched, setTouched] = useState<Record<string, boolean>>({})
  const [attempted, setAttempted] = useState(false)
  const [jsonOpen, setJsonOpen] = useState(false)
  const [jsonText, setJsonText] = useState('')
  const [jsonError, setJsonError] = useState<string | null>(null)

  const setBusyState = (value: boolean) => {
    busyRef.current = value
    setBusy(value)
  }

  const nextField = (key = '', value = ''): FieldRow => {
    fieldSeq.current += 1
    return { id: `field-${fieldSeq.current}`, key, value }
  }

  const fillFrom = (row: CustomConnectorBody, editing: boolean) => {
    setDraft({
      name: row.name,
      transport: row.transport,
      command: row.command ?? '',
      args: (row.args ?? []).join('\n'),
      url: row.url ?? '',
      fields: Object.entries(row.values).map(([key, value]) => nextField(key, value)),
      editing
    })
    setConfirmDelete(false)
    setError(null)
    setTouched({})
    setAttempted(false)
  }

  const applyRecord = (row: CustomConnectorBody) => {
    fillFrom(row, true)
    setJsonText(formatConnectorPaste(row))
    setJsonError(null)
  }

  useEffect(() => {
    if (!jsonOpen || !focusJson.current) return
    focusJson.current = false
    const field = jsonRef.current
    if (!field) return
    field.focus()
    if (!selectSeed.current) return
    selectSeed.current = false
    field.setSelectionRange(0, field.value.length)
  }, [jsonOpen, jsonText])

  useEffect(() => {
    if (!open) return
    let cancelled = false
    setDraft(blank())
    setRows([])
    setConfirmDelete(false)
    setError(null)
    setTouched({})
    setAttempted(false)
    setJsonOpen(false)
    setJsonText('')
    setJsonError(null)
    setBusyState(false)
    void listCustomConnectors()
      .then((next) => {
        if (!cancelled) setRows(next)
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(errorKey(err))
      })
    return () => {
      cancelled = true
    }
  }, [open])

  const connectorInput = {
    name: draft.name,
    transport: draft.transport,
    command: draft.command,
    args: draft.args,
    url: draft.url,
    values: draft.fields
  }
  const body = customConnectorBody(connectorInput)
  const issues = customConnectorIssues(connectorInput)
  const reveal = (key: string) => setTouched((current) => (current[key] ? current : { ...current, [key]: true }))
  const shown = (key: string) => Boolean(touched[key] || attempted)

  const patch = (next: Partial<Draft>) => {
    setDraft((current) => ({ ...current, ...next }))
    setConfirmDelete(false)
  }

  const save = async () => {
    if (!body || busyRef.current) return
    setBusyState(true)
    setError(null)
    try {
      await saveCustomConnector(body)
      const next = await listCustomConnectors()
      const saved = next.find((row) => row.name === body.name) ?? body
      setRows(next)
      applyRecord(saved)
      toast.success(t('capabilities.connector.custom.saved'))
    } catch (err) {
      setError(errorKey(err))
    } finally {
      setBusyState(false)
    }
  }

  const pasteOptions = () => {
    const name = draft.name.trim()
    return name ? { name } : undefined
  }

  const acceptPasted = (text: string) => {
    const result = parseConnectorPaste(text, pasteOptions())
    if (!result.ok) {
      setJsonError(result.code)
      return
    }
    setJsonError(null)
    if (result.connectors.length !== 1) return
    const row = result.connectors[0]
    const editing = rows.some((item) => item.name === row.name) || (draft.editing && draft.name === row.name)
    fillFrom(row, editing)
  }

  const importMany = async (connectors: CustomConnectorBody[]) => {
    if (busyRef.current) return
    setBusyState(true)
    setError(null)
    let failed: unknown = null
    for (const item of connectors) {
      try {
        await saveCustomConnector(item)
      } catch (err) {
        failed = err
        break
      }
    }
    try {
      const next = await listCustomConnectors()
      setRows(next)
      const saved = next.find((row) => row.name === connectors[0]?.name)
      if (saved) applyRecord(saved)
    } catch (err) {
      if (!failed) failed = err
    }
    if (failed) setError(errorKey(failed))
    else {
      setJsonOpen(false)
      toast.success(t('capabilities.connector.custom.json_imported', { count: connectors.length }))
    }
    setBusyState(false)
  }

  const applyJson = () => {
    const result = parseConnectorPaste(jsonText, pasteOptions())
    if (!result.ok) {
      setJsonError(result.code)
      return
    }
    setJsonError(null)
    if (result.connectors.length <= 1) {
      const row = result.connectors[0]
      if (!row) return
      const editing = rows.some((item) => item.name === row.name) || (draft.editing && draft.name === row.name)
      fillFrom(row, editing)
      setJsonOpen(false)
      return
    }
    void importMany(result.connectors)
  }

  const showJson = () => {
    if (jsonOpen) return
    const seeded = !jsonText.trim() && body ? formatConnectorPaste(body) : null
    if (seeded) {
      selectSeed.current = true
      setJsonText(seeded)
    }
    focusJson.current = true
    setJsonError(null)
    setJsonOpen(true)
  }

  const pasted = jsonOpen && jsonText.trim() ? parseConnectorPaste(jsonText, pasteOptions()) : null
  const importCount = pasted && pasted.ok && pasted.connectors.length > 1 ? pasted.connectors.length : 0

  const remove = async () => {
    if (!draft.editing || busyRef.current) return
    setBusyState(true)
    setError(null)
    try {
      await removeCustomConnector(draft.name)
      const next = await listCustomConnectors()
      setRows(next)
      setDraft(blank())
      setConfirmDelete(false)
    } catch (err) {
      setError(errorKey(err))
    } finally {
      setBusyState(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (busyRef.current) return
        onOpenChange(next)
      }}>
      {open ? (
        <DialogContent
          closeLabel={t('common.close')}
          closeOnOverlayClick={!busy}
          overlayClassName="!opacity-100"
          className="flex max-h-[min(640px,calc(100vh-4rem))] w-full max-w-[min(420px,calc(100%-2rem))] !animate-none flex-col gap-0 overflow-hidden bg-white p-0 !opacity-100 sm:max-w-[min(420px,calc(100%-2rem))]">
          <form
            data-ui="capabilities.connector.custom"
            className="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 pt-5 pb-6"
            onSubmit={(event) => {
              event.preventDefault()
              setAttempted(true)
              void save()
            }}>
            <div className="pr-8">
              <DialogTitle className="text-[15px] leading-6 font-medium text-[#171717]">
                {t('capabilities.connector.custom.title')}
              </DialogTitle>
            </div>
            <DialogDescription className="sr-only">{t('capabilities.connector.custom.hint')}</DialogDescription>
            {rows.length > 0 ? (
              <ul className="mt-4 flex flex-col gap-0.5" data-ui="capabilities.connector.custom.list">
                {rows.map((row) => (
                  <li key={row.name} className="flex h-9 items-center gap-2.5">
                    <button
                      type="button"
                      aria-pressed={draft.editing && draft.name === row.name}
                      onClick={() => applyRecord(row)}
                      className="inline-flex h-9 min-w-0 flex-1 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10">
                      <Plug aria-hidden className="size-4 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} />
                      <span className="min-w-0 truncate">{row.name}</span>
                    </button>
                    <span className="inline-flex shrink-0 items-center gap-1.5 text-[12px] text-[#8a8a8a]">
                      <TransportIcon transport={row.transport} />
                      {t(transportLabel(row.transport))}
                    </span>
                  </li>
                ))}
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setDraft(blank())
                      setJsonText('')
                      setJsonError(null)
                      setConfirmDelete(false)
                      setError(null)
                      setTouched({})
                      setAttempted(false)
                    }}
                    className="inline-flex h-9 items-center gap-1.5 rounded-[8px] px-2.5 text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
                    <Plus aria-hidden className="size-4" strokeWidth={1.7} />
                    {t('capabilities.connector.custom.new')}
                  </button>
                </li>
              </ul>
            ) : null}
            <Tabs
              value={jsonOpen ? 'json' : 'form'}
              onValueChange={(value) => {
                if (value === 'json') showJson()
                else setJsonOpen(false)
              }}
              className="mt-4 gap-4">
              <TabsList aria-label={t('capabilities.connector.custom.mode')} className="w-full">
                <TabsTrigger value="form" className={TAB_TRIGGER}>
                  <SquarePen aria-hidden className="size-4" strokeWidth={1.7} />
                  {t('capabilities.connector.custom.form')}
                </TabsTrigger>
                <TabsTrigger value="json" data-ui="capabilities.connector.custom.json" className={TAB_TRIGGER}>
                  <Braces aria-hidden className="size-4" strokeWidth={1.7} />
                  {t('capabilities.connector.custom.json')}
                </TabsTrigger>
              </TabsList>
              <TabsContent value="form">
                <label className="block">
                  <FieldLabel icon={Tag}>{t('capabilities.connector.custom.name')}</FieldLabel>
                  <input
                    aria-label={t('capabilities.connector.custom.name')}
                    aria-required="true"
                    aria-invalid={shown('name') && issues.name ? true : undefined}
                    aria-describedby={shown('name') && issues.name ? 'custom-connector-name-error' : undefined}
                    value={draft.name}
                    disabled={draft.editing || busy}
                    autoComplete="off"
                    autoCapitalize="off"
                    spellCheck={false}
                    placeholder={t('capabilities.connector.custom.name_placeholder')}
                    onBlur={() => reveal('name')}
                    onChange={(event) => patch({ name: event.target.value })}
                    className={`${FIELD} disabled:text-[#8a8a8a]`}
                  />
                  {shown('name') && issues.name ? (
                    <p id="custom-connector-name-error" className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                      {t(`capabilities.connector.custom.${issues.name}`)}
                    </p>
                  ) : null}
                </label>
                <div
                  role="radiogroup"
                  aria-label={t('capabilities.connector.custom.transport')}
                  className="mt-4 flex rounded-[8px] bg-[#f5f5f5] p-1">
                  {TRANSPORTS.map((transport) => {
                    const selected = draft.transport === transport
                    return (
                      <button
                        key={transport}
                        type="button"
                        role="radio"
                        aria-checked={selected}
                        disabled={busy}
                        onClick={() => patch({ transport })}
                        className={segmentClass(selected)}>
                        <TransportIcon transport={transport} />
                        {t(transportLabel(transport))}
                      </button>
                    )
                  })}
                </div>
                {draft.transport === 'stdio' ? (
                  <>
                    <label className="mt-4 block">
                      <FieldLabel icon={Terminal}>{t('capabilities.connector.custom.command')}</FieldLabel>
                      <input
                        aria-label={t('capabilities.connector.custom.command')}
                        aria-required="true"
                        aria-invalid={shown('command') && issues.command ? true : undefined}
                        aria-describedby={
                          shown('command') && issues.command ? 'custom-connector-command-error' : undefined
                        }
                        value={draft.command}
                        disabled={busy}
                        autoComplete="off"
                        autoCapitalize="off"
                        spellCheck={false}
                        placeholder={t('capabilities.connector.custom.command_placeholder')}
                        onBlur={() => reveal('command')}
                        onChange={(event) => patch({ command: event.target.value })}
                        className={FIELD}
                      />
                      {shown('command') && issues.command ? (
                        <p
                          id="custom-connector-command-error"
                          className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                          {t(`capabilities.connector.custom.${issues.command}`)}
                        </p>
                      ) : null}
                    </label>
                    <label className="mt-4 block">
                      <FieldLabel icon={List}>{t('capabilities.connector.custom.args')}</FieldLabel>
                      <textarea
                        aria-label={t('capabilities.connector.custom.args')}
                        aria-invalid={shown('args') && issues.args ? true : undefined}
                        aria-describedby={
                          shown('args') && issues.args
                            ? 'custom-connector-args-hint custom-connector-args-error'
                            : 'custom-connector-args-hint'
                        }
                        value={draft.args}
                        disabled={busy}
                        rows={3}
                        spellCheck={false}
                        placeholder={t('capabilities.connector.custom.args_placeholder')}
                        onBlur={() => reveal('args')}
                        onChange={(event) => patch({ args: event.target.value })}
                        className="min-h-9 w-full resize-y rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 py-2 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-black/10"
                      />
                      <span id="custom-connector-args-hint" className="mt-1 block text-[12px] leading-5 text-[#8a8a8a]">
                        {t('capabilities.connector.custom.args_hint')}
                      </span>
                      {shown('args') && issues.args ? (
                        <p id="custom-connector-args-error" className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                          {t(`capabilities.connector.custom.${issues.args}`)}
                        </p>
                      ) : null}
                    </label>
                  </>
                ) : (
                  <label className="mt-4 block">
                    <FieldLabel icon={Link2}>{t('capabilities.connector.custom.url')}</FieldLabel>
                    <input
                      aria-label={t('capabilities.connector.custom.url')}
                      aria-required="true"
                      aria-invalid={shown('url') && issues.url ? true : undefined}
                      aria-describedby={shown('url') && issues.url ? 'custom-connector-url-error' : undefined}
                      value={draft.url}
                      disabled={busy}
                      autoComplete="off"
                      autoCapitalize="off"
                      spellCheck={false}
                      placeholder={t(
                        draft.transport === 'sse'
                          ? 'capabilities.connector.custom.sse_placeholder'
                          : 'capabilities.connector.custom.url_placeholder'
                      )}
                      onBlur={() => reveal('url')}
                      onChange={(event) => patch({ url: event.target.value })}
                      className={FIELD}
                    />
                    {shown('url') && issues.url ? (
                      <p id="custom-connector-url-error" className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                        {t(`capabilities.connector.custom.${issues.url}`)}
                      </p>
                    ) : null}
                  </label>
                )}
                <div className="mt-4">
                  <div className="mb-2 flex items-center justify-between">
                    <FieldLabel icon={KeyRound} className="">
                      {t(
                        draft.transport === 'stdio'
                          ? 'capabilities.connector.custom.env'
                          : 'capabilities.connector.custom.headers'
                      )}
                    </FieldLabel>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => patch({ fields: [...draft.fields, nextField()] })}
                      className="inline-flex h-9 items-center gap-1.5 rounded-[8px] px-2 text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                      <Plus className="size-4 text-current" strokeWidth={1.7} aria-hidden />
                      {t('capabilities.connector.custom.add_field')}
                    </button>
                  </div>
                  {draft.fields.map((field, index) => {
                    const fieldIssue = shown(field.id) ? issues.fields[index] : null
                    const secret = draft.transport !== 'stdio'
                    return (
                      <div key={field.id} className="mb-2">
                        <div className="flex gap-2">
                          <input
                            aria-label={t('capabilities.connector.custom.key')}
                            aria-invalid={fieldIssue ? true : undefined}
                            aria-describedby={fieldIssue ? `custom-connector-field-${field.id}` : undefined}
                            value={field.key}
                            disabled={busy}
                            autoComplete="off"
                            autoCapitalize="off"
                            spellCheck={false}
                            placeholder={t(
                              secret
                                ? 'capabilities.connector.custom.header_key_placeholder'
                                : 'capabilities.connector.custom.key_placeholder'
                            )}
                            onBlur={() => reveal(field.id)}
                            onChange={(event) =>
                              patch({
                                fields: draft.fields.map((item) =>
                                  item.id === field.id ? { ...item, key: event.target.value } : item
                                )
                              })
                            }
                            className={FIELD}
                          />
                          <input
                            aria-label={t('capabilities.connector.custom.value')}
                            aria-invalid={fieldIssue ? true : undefined}
                            aria-describedby={fieldIssue ? `custom-connector-field-${field.id}` : undefined}
                            type="password"
                            value={field.value}
                            disabled={busy}
                            autoComplete="off"
                            spellCheck={false}
                            placeholder={t(
                              secret
                                ? 'capabilities.connector.custom.header_value_placeholder'
                                : 'capabilities.connector.custom.value_placeholder'
                            )}
                            onBlur={() => reveal(field.id)}
                            onChange={(event) =>
                              patch({
                                fields: draft.fields.map((item) =>
                                  item.id === field.id ? { ...item, value: event.target.value } : item
                                )
                              })
                            }
                            className={FIELD}
                          />
                        </div>
                        {fieldIssue ? (
                          <p
                            id={`custom-connector-field-${field.id}`}
                            className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                            {t(`capabilities.connector.custom.${fieldIssue}`)}
                          </p>
                        ) : null}
                      </div>
                    )
                  })}
                  {attempted && issues.config ? (
                    <p className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                      {t(`capabilities.connector.custom.${issues.config}`)}
                    </p>
                  ) : null}
                  <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('capabilities.config.local')}</p>
                </div>
              </TabsContent>
              <TabsContent value="json">
                <textarea
                  ref={jsonRef}
                  aria-label={t('capabilities.connector.custom.json')}
                  aria-invalid={jsonError ? true : undefined}
                  aria-describedby="custom-connector-json-hint"
                  data-ui="capabilities.connector.custom.json-input"
                  value={jsonText}
                  disabled={busy}
                  spellCheck={false}
                  autoCapitalize="off"
                  autoCorrect="off"
                  placeholder={t('capabilities.connector.custom.json_placeholder')}
                  onChange={(event) => {
                    setJsonText(event.target.value)
                    setJsonError(null)
                  }}
                  onPaste={(event) => {
                    const clip =
                      event.clipboardData?.getData('text/plain') || event.clipboardData?.getData('text') || ''
                    if (!clip) return
                    event.preventDefault()
                    const field = event.currentTarget
                    const start = field.selectionStart ?? jsonText.length
                    const end = field.selectionEnd ?? start
                    const next = field.value.slice(0, start) + clip + field.value.slice(end)
                    const caret = start + clip.length
                    setJsonText(next)
                    acceptPasted(next)
                    requestAnimationFrame(() => {
                      const node = jsonRef.current
                      if (!node) return
                      node.setSelectionRange(caret, caret)
                    })
                  }}
                  className="min-h-[140px] w-full resize-y rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 py-2 font-mono text-[13px] leading-5 text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-black/10"
                />
                <div className="mt-1.5 flex items-start justify-between gap-3">
                  <p
                    id="custom-connector-json-hint"
                    role={jsonError ? 'alert' : undefined}
                    className="m-0 min-w-0 flex-1 text-[12px] leading-5 text-[#8a8a8a]">
                    {jsonError
                      ? t(`capabilities.connector.custom.${jsonError}`)
                      : t('capabilities.connector.custom.json_hint')}
                  </p>
                  <button
                    type="button"
                    data-ui="capabilities.connector.custom.json-apply"
                    disabled={busy || !jsonText.trim()}
                    onClick={applyJson}
                    className="inline-flex shrink-0 items-center gap-1.5 text-[13px] text-[#3f3f3f] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    <Check aria-hidden className="size-4" strokeWidth={1.7} />
                    {importCount > 1
                      ? t('capabilities.connector.custom.json_import', { count: importCount })
                      : t('capabilities.connector.custom.json_apply')}
                  </button>
                </div>
              </TabsContent>
            </Tabs>
            {error ? (
              <p className="m-0 mt-3 text-[12px] leading-5 text-[#8a8a8a]" role="alert">
                {t(`capabilities.connector.custom.${error}`)}
              </p>
            ) : null}
            <div className="mt-4 flex items-center justify-between gap-3">
              {draft.editing ? (
                confirmDelete ? (
                  <button
                    type="button"
                    data-ui="capabilities.connector.custom.remove-confirm"
                    disabled={busy}
                    onClick={() => void remove()}
                    className="text-[13px] text-[#b42318] outline-none focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('capabilities.connector.custom.remove_confirm')}
                  </button>
                ) : (
                  <button
                    type="button"
                    data-ui="capabilities.connector.custom.remove"
                    disabled={busy}
                    onClick={() => setConfirmDelete(true)}
                    className="text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('capabilities.connector.custom.remove')}
                  </button>
                )
              ) : (
                <span />
              )}
              <span
                className="inline-flex"
                onClick={() => {
                  if (body === null) setAttempted(true)
                }}>
                <button
                  type="submit"
                  disabled={body === null || busy}
                  onClick={() => setAttempted(true)}
                  className={`inline-flex h-9 items-center justify-center rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40 ${body === null ? 'pointer-events-none' : ''}`}>
                  {t(busy ? 'capabilities.connector.custom.saving' : 'capabilities.connector.custom.save')}
                </button>
              </span>
            </div>
          </form>
        </DialogContent>
      ) : null}
    </Dialog>
  )
}
