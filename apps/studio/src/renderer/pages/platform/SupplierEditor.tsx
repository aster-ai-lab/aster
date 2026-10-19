import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Textarea
} from '@a3s-lab/ui'
import { createContext, use, useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR, { useSWRConfig } from 'swr'

import { beginOptimistic, type OptimisticHandle } from '@renderer/components/collection/optimisticRows'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import type { PlatformItem } from '@renderer/data/platformCatalog'
import {
  createSupplier,
  deleteSupplier,
  readSupplier,
  readSupplierAccess,
  SUPPLIER_KINDS,
  SUPPLIER_PROTOCOLS,
  updateSupplier,
  type SupplierAccess,
  type SupplierRecord,
  type SupplierWrite
} from '@renderer/data/platformSuppliers'

import { SupplierCreateRecovery } from './SupplierCreateRecovery'
import { SupplierRelations } from './SupplierRelations'

type Operation = 'create' | 'edit' | 'delete' | 'read'
const titles = {
  create: 'platform.supplierForm.createTitle',
  edit: 'platform.supplierForm.editTitle',
  delete: 'platform.supplierForm.deleteTitle',
  read: 'platform.supplierForm.readTitle'
} as const
const labels = {
  name: 'platform.supplierForm.name',
  id: 'platform.supplierForm.id',
  kind: 'platform.supplierForm.kind',
  protocol: 'platform.supplierForm.protocol',
  baseUrl: 'platform.supplierForm.baseUrl',
  dataRegion: 'platform.supplierForm.dataRegion',
  retention: 'platform.supplierForm.retention',
  originVendors: 'platform.supplierForm.originVendors',
  models: 'platform.supplierForm.models',
  enabled: 'platform.supplierForm.enabled',
  credential: 'platform.supplierForm.credential',
  secret: 'platform.supplierForm.secret',
  revision: 'platform.supplierForm.revision'
} as const
const optionLabels = {
  vendor: 'platform.supplierForm.option.vendor',
  relay: 'platform.supplierForm.option.relay',
  self_hosted: 'platform.supplierForm.option.self_hosted',
  openai: 'platform.supplierForm.option.openai',
  anthropic: 'platform.supplierForm.option.anthropic',
  yes: 'platform.supplierForm.option.yes',
  no: 'platform.supplierForm.option.no',
  unknown: 'platform.supplierForm.option.unknown',
  keep: 'platform.supplierForm.option.keep',
  replace: 'platform.supplierForm.option.replace',
  clear: 'platform.supplierForm.option.clear'
} as const
type Option = keyof typeof optionLabels
/**
 * `closing` keeps a dismissed editor mounted until its exit animation ends; it is never resumed.
 * `writing` keeps it mounted past that while an optimistic write it closed for is still in flight,
 * so a rejected write can reopen it with the reader's draft; `closed` records that the exit has ended.
 */
type Editor = {
  mode: Operation
  id?: string
  generation: string
  open: boolean
  closing?: boolean
  writing?: boolean
  closed?: boolean
}
type Actions = {
  access?: SupplierAccess
  busy: boolean
  recovering: boolean
  resumable: boolean
  authorityError?: unknown
  retryAuthority: () => void
  open: (mode: Operation, id?: string) => void
}
const Context = createContext<Actions | null>(null)
export function useSupplierActions() {
  return use(Context)
}

function intentKey(scope: string) {
  return `studio/platform/supplier-create-intent/${scope}`
}
function pendingIntent(scope: string) {
  try {
    const value = localStorage.getItem(intentKey(scope))
    return value === null ? null : value || 'unavailable'
  } catch {
    return 'unavailable'
  }
}
/** The list collection whose rows supplier writes change optimistically; the catalog page uses the same ID. */
export function supplierCollection(scope: string) {
  return `studio/platform/${scope}/suppliers/page`
}
/** The row an accepted write produces, so the list can show it before the owner answers. Status stays owner-derived. */
function supplierRow(row: PlatformItem, input: SupplierWrite): PlatformItem {
  const values: Record<string, string> = {
    kind: input.kind,
    protocol: input.protocol,
    models: String(input.models.length),
    endpoint: input.baseUrl,
    region: input.dataRegion ?? '',
    retains: input.retainsData === null ? '' : String(input.retainsData),
    vendors: input.originVendors.join(', ')
  }
  if (input.credential.action !== 'keep') values.credential = input.credential.action === 'replace' ? 'set' : 'missing'
  return {
    ...row,
    name: input.name,
    fields: row.fields.map((field) =>
      Object.hasOwn(values, field.key) ? { ...field, value: values[field.key] } : field
    )
  }
}
function errorKey(error: unknown) {
  const status = error instanceof MemberOrgError ? error.status : undefined
  if (status === 401 || status === 404) return 'platform.supplierForm.unavailable'
  if (status === 409 || status === 412) return 'platform.supplierForm.conflict'
  if (status === 400 || status === 422) return 'platform.supplierForm.rejected'
  return 'platform.supplierForm.uncertain'
}

export function SupplierEditorProvider({
  scope,
  active = true,
  children
}: {
  scope: string
  active?: boolean
  children: ReactNode
}) {
  const { t } = useTranslation()
  const { mutate, cache } = useSWRConfig()
  const [editor, setEditor] = useState<Editor | null>(null)
  const [recovery, setRecovery] = useState<string | null>(null)
  const {
    data,
    error,
    mutate: reloadAccess
  } = useSWR(active || editor || recovery ? `studio/platform/${scope}/supplier-access` : null, () =>
    readSupplierAccess()
  )
  const [pending, setPending] = useState(() => pendingIntent(scope))
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState('')
  const editorAllowed =
    !editor ||
    Boolean(data && (editor.mode === 'read' ? data.read : editor.mode === 'edit' ? data.update : data[editor.mode]))
  const recoveryAllowed = !recovery || Boolean(data?.create)
  const alive = useRef(true)
  const container = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLElement | null>(null)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])
  const refresh = useCallback(
    async (revalidate = true) => {
      const prefix = `studio/platform/${scope}/`
      const affected = (key: unknown) => {
        if (typeof key !== 'string') return false
        if (key.startsWith(prefix)) return !key.endsWith('/supplier-access')
        if (
          [
            'studio/platform-overview',
            'studio/platform/suppliers',
            'studio/platform/models',
            'studio/platform/model-routes'
          ].includes(key)
        )
          return true
        if (key.startsWith('studio/platform/suppliers/')) return true
        if (!key.startsWith('[')) return false
        try {
          const parsed: unknown = JSON.parse(key)
          return Array.isArray(parsed) && typeof parsed[0] === 'string' && parsed[0].startsWith(prefix)
        } catch {
          return false
        }
      }
      await mutate(affected, undefined, { revalidate: false })
      if (revalidate) await mutate(affected)
      if (revalidate && [...cache.keys()].some((key) => affected(key) && cache.get(key)?.error))
        throw new MemberOrgError('refresh')
    },
    [mutate, cache, scope]
  )
  useEffect(() => {
    if (error || !editorAllowed || !recoveryAllowed) {
      setEditor(null)
      setRecovery(null)
      setBusy(false)
      setNotice('platform.supplierForm.unavailable')
      void refresh(!error).catch(() => undefined)
    }
  }, [error, editorAllowed, recoveryAllowed, refresh])
  const unavailable = useCallback(() => {
    setEditor(null)
    setRecovery(null)
    setBusy(false)
    setNotice('platform.supplierForm.unavailable')
    void refresh().catch(() => undefined)
    void reloadAccess()
  }, [refresh, reloadAccess])
  const saved = async (reconciled?: () => void) => {
    if (!alive.current) return reconciled?.()
    setNotice('platform.supplierForm.saved')
    try {
      await refresh()
    } catch {
      if (alive.current) setNotice('platform.supplierForm.refreshFailed')
    } finally {
      reconciled?.()
    }
  }
  const clearIntent = (expected: string) => {
    try {
      if (localStorage.getItem(intentKey(scope)) === expected) localStorage.removeItem(intentKey(scope))
      const current = localStorage.getItem(intentKey(scope))
      setPending(current)
      return current === null
    } catch {
      setNotice('platform.supplierForm.recoveryStorageFailed')
      return false
    }
  }
  const markIntent = (intent: string | null) => {
    if (intent) {
      const existing = localStorage.getItem(intentKey(scope))
      if (existing !== null && existing !== intent) throw new Error('another creation is unresolved')
      localStorage.setItem(intentKey(scope), intent)
      if (localStorage.getItem(intentKey(scope)) !== intent) throw new Error('intent unavailable')
    } else {
      if (editor) clearIntent(editor.generation)
      return
    }
    setPending(intent)
  }
  const restoreFocus = () => {
    const target = trigger.current?.isConnected
      ? trigger.current
      : container.current?.querySelector<HTMLElement>(
          '[data-ui="platform.supplier.create"]:not([disabled]), [data-ui="platform.list"] button:not([disabled]), input, button:not([disabled])'
        )
    target?.focus()
  }
  const retained = editor && !editor.closing ? editor : null
  const open = useCallback(
    (mode: Operation, id?: string) => {
      if (error || !data || busy) return
      const permitted = mode === 'read' ? data.read : mode === 'edit' ? data.update : data[mode]
      if (!permitted) return
      trigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
      if (retained) {
        setEditor({ ...retained, open: true })
        return
      }
      if (mode === 'create' && pending) {
        setNotice('platform.supplierForm.recoveryRequired')
        return
      }
      setEditor({ mode, id, generation: crypto.randomUUID(), open: true })
    },
    [error, data, busy, retained, pending]
  )
  const actions = useMemo<Actions>(
    () => ({
      access: error ? undefined : data,
      busy,
      recovering: Boolean(pending),
      resumable: Boolean(retained?.mode === 'create' && pending),
      authorityError: error,
      retryAuthority: () => void reloadAccess(),
      open
    }),
    [error, data, busy, pending, retained?.mode, reloadAccess, open]
  )
  return (
    <Context value={actions}>
      <div ref={container} className="flex h-full min-h-0 flex-1 flex-col">
        {children}
        {(active || retained?.open) && (notice || (pending && !retained?.open) || error) ? (
          <div
            role="status"
            className="flex flex-wrap items-center gap-3 border-t border-[#efefef] bg-white px-6 py-3 text-[13px] text-[#3f3f3f]">
            <span>
              {t(error ? 'platform.supplierForm.accessFailed' : notice || 'platform.supplierForm.recoveryRequired')}
            </span>
            {['platform.supplierForm.refreshFailed', 'platform.supplierForm.recoveryRefreshFailed'].includes(notice) ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  void refresh()
                    .then(() => setNotice(''))
                    .catch(() => undefined)
                }}>
                {t('common.retry')}
              </Button>
            ) : null}
            {error ? (
              <Button variant="outline" size="sm" onClick={() => void reloadAccess()}>
                {t('common.retry')}
              </Button>
            ) : null}
            {retained && !retained.open ? (
              <Button variant="outline" size="sm" onClick={() => setEditor({ ...retained, open: true })}>
                {t('platform.supplierForm.resume')}
              </Button>
            ) : null}
            {pending && data?.create && !error && !retained?.open ? (
              <Button
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => {
                  trigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
                  setRecovery(pending)
                }}>
                {t('platform.supplierForm.recoveryTitle')}
              </Button>
            ) : null}
          </div>
        ) : null}
        {editor && data && !error && editorAllowed ? (
          <SupplierEditor
            key={editor.generation}
            scope={scope}
            editor={editor}
            onBusy={setBusy}
            onIntent={markIntent}
            collection={supplierCollection(scope)}
            onSaved={(reconciled) => void saved(reconciled)}
            onDismiss={(retain) => {
              setBusy(false)
              setEditor({ ...editor, open: false, closing: !retain })
            }}
            onWriting={() => setEditor({ ...editor, open: false, closing: true, writing: true })}
            onWritten={(accepted) =>
              setEditor((current) => {
                if (current?.generation !== editor.generation) return current
                if (!accepted) return { ...current, open: true, closing: false, writing: false, closed: false }
                return current.closed ? null : { ...current, writing: false }
              })
            }
            onClosed={() =>
              setEditor((current) => {
                if (!current?.closing || current.generation !== editor.generation) return current
                return current.writing ? { ...current, closed: true } : null
              })
            }
            onUnavailable={unavailable}
            onRestoreFocus={restoreFocus}
          />
        ) : null}
        {recovery && data && !error && recoveryAllowed ? (
          <SupplierCreateRecovery
            key={recovery}
            intent={recovery}
            onBusy={setBusy}
            onUnavailable={unavailable}
            onClose={() => setRecovery(null)}
            onRestoreFocus={restoreFocus}
            onView={(id) => {
              setRecovery(null)
              open('read', id)
            }}
            onResolved={(outcome) => {
              const released = clearIntent(recovery)
              setEditor(null)
              setNotice(released ? '' : 'platform.supplierForm.recoveryStorageFailed')
              if (outcome.state === 'created')
                void refresh().catch(() => {
                  if (alive.current) setNotice('platform.supplierForm.recoveryRefreshFailed')
                })
              return released
            }}
          />
        ) : null}
      </div>
    </Context>
  )
}

type Draft = Omit<SupplierWrite, 'originVendors' | 'models' | 'credential' | 'retainsData'> & {
  originVendors: string
  models: string
  retention: 'unknown' | 'yes' | 'no'
  credential: 'keep' | 'replace' | 'clear'
  secret: string
}
function draftFor(record?: SupplierRecord): Draft {
  return {
    name: record?.name ?? '',
    kind: record?.kind ?? 'vendor',
    protocol: record?.protocol ?? 'openai',
    baseUrl: record?.baseUrl ?? '',
    dataRegion: record?.dataRegion ?? '',
    enabled: record?.enabled ?? true,
    originVendors: record?.originVendors.join('\n') ?? '',
    models: record?.models.join('\n') ?? '',
    retention: record?.retainsData === true ? 'yes' : record?.retainsData === false ? 'no' : 'unknown',
    credential: 'keep',
    secret: ''
  }
}
function payload(draft: Draft): SupplierWrite {
  const lines = (value: string) => [
    ...new Set(
      value
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
    )
  ]
  return {
    name: draft.name.trim(),
    kind: draft.kind,
    protocol: draft.protocol,
    baseUrl: draft.baseUrl.trim(),
    dataRegion: draft.dataRegion?.trim() || null,
    retainsData: draft.retention === 'unknown' ? null : draft.retention === 'yes',
    originVendors: lines(draft.originVendors),
    models: lines(draft.models),
    enabled: draft.enabled,
    credential:
      draft.credential === 'replace' ? { action: 'replace', value: draft.secret } : { action: draft.credential }
  }
}

function SupplierEditor({
  scope,
  editor,
  onBusy,
  collection,
  onIntent,
  onSaved,
  onDismiss,
  onWriting,
  onWritten,
  onClosed,
  onUnavailable,
  onRestoreFocus
}: {
  scope: string
  editor: Editor
  collection: string
  onBusy: (busy: boolean) => void
  onIntent: (intent: string | null) => void
  /** `reconciled` runs once the list has re-read the owner, releasing an optimistic overlay. */
  onSaved: (reconciled?: () => void) => void
  onDismiss: (retain: boolean) => void
  /** The dialog closes while an optimistic edit or delete is in flight; `onWritten` reports the outcome. */
  onWriting: () => void
  onWritten: (accepted: boolean) => void
  onClosed: () => void
  onUnavailable: () => void
  onRestoreFocus: () => void
}) {
  const { t } = useTranslation()
  const fieldId = useId()
  const [record, setRecord] = useState<SupplierRecord>()
  const [draft, setDraft] = useState(() => draftFor())
  const [baseline, setBaseline] = useState(() => draftFor())
  const [loading, setLoading] = useState(editor.mode !== 'create')
  const [pending, setPending] = useState(false)
  const [message, setMessage] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [discard, setDiscard] = useState(false)
  const [conflict, setConflict] = useState(false)
  const [latest, setLatest] = useState<SupplierRecord>()
  const [completed, setCompleted] = useState(false)
  const [uncertain, setUncertain] = useState(false)
  const [dependencyReview, setDependencyReview] = useState<{ revision: number; total: number } | null>(null)
  const canDelete = Boolean(record && dependencyReview?.revision === record.updatedAt && dependencyReview.total === 0)
  useEffect(() => {
    if (record && dependencyReview && dependencyReview.revision !== record.updatedAt) setConflict(true)
  }, [record, dependencyReview])
  const frozen = useRef<SupplierWrite | null>(null)
  const alive = useRef(true)
  const gate = useRef(false)
  const readGeneration = useRef(0)
  const summary = useRef<HTMLDivElement>(null)
  const safeButton = useRef<HTMLButtonElement>(null)
  const dirty =
    !completed &&
    editor.mode !== 'delete' &&
    editor.mode !== 'read' &&
    JSON.stringify(draft) !== JSON.stringify(baseline)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
      frozen.current = null
    }
  }, [])
  const load = useCallback(
    async (compare = false) => {
      if (!editor.id) return
      const generation = ++readGeneration.current
      setLoading(true)
      try {
        const current = await readSupplier(editor.id)
        if (!alive.current || generation !== readGeneration.current) return
        if (compare) setLatest(current)
        else {
          setRecord(current)
          const next = draftFor(current)
          setDraft(next)
          setBaseline(next)
          setMessage('')
        }
      } catch (error) {
        if (!alive.current || generation !== readGeneration.current) return
        if (error instanceof MemberOrgError && [401, 404].includes(error.status ?? 0)) onUnavailable()
        else setMessage('platform.supplierForm.loadFailed')
      } finally {
        if (alive.current && generation === readGeneration.current) setLoading(false)
      }
    },
    [editor.id, onUnavailable]
  )
  useEffect(() => {
    void load()
  }, [load])
  useEffect(() => {
    if (!dirty && !uncertain) return
    const guard = (event: BeforeUnloadEvent) => {
      event.preventDefault()
      event.returnValue = ''
    }
    window.addEventListener('beforeunload', guard)
    return () => window.removeEventListener('beforeunload', guard)
  }, [dirty, uncertain])
  useEffect(() => {
    if (Object.keys(errors).length) summary.current?.focus()
  }, [errors])
  useEffect(() => {
    if (discard) safeButton.current?.focus()
  }, [discard])
  const close = () => {
    if (gate.current) return
    if (uncertain && editor.mode === 'create') {
      onDismiss(true)
      return
    }
    if (dirty) setDiscard(true)
    else onDismiss(false)
  }
  const change = <K extends keyof Draft>(field: K, value: Draft[K]) =>
    setDraft((previous) => ({ ...previous, [field]: value }))
  const submit = async () => {
    if (gate.current || loading || conflict || completed || (editor.mode === 'delete' && !canDelete)) return
    const invalid: Record<string, string> = {}
    if (editor.mode !== 'delete') {
      if (!draft.name.trim()) invalid.name = 'platform.supplierForm.nameRequired'
      try {
        const url = new URL(draft.baseUrl.trim())
        if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password)
          invalid.baseUrl = 'platform.supplierForm.urlInvalid'
      } catch {
        invalid.baseUrl = 'platform.supplierForm.urlInvalid'
      }
      if (
        draft.credential === 'replace' &&
        (!draft.secret.trim() || new TextEncoder().encode(draft.secret).length > 16_384)
      )
        invalid.secret = 'platform.supplierForm.secretRequired'
    }
    setErrors(invalid)
    if (Object.keys(invalid).length) return
    gate.current = true
    setPending(true)
    onBusy(true)
    setMessage('')
    const wasUncertain = uncertain
    // Edits and deletes show in the list at once; a rejected write restores the rows and reopens this dialog.
    let optimistic: OptimisticHandle | null = null
    try {
      if (editor.mode === 'create') {
        const input = frozen.current ?? payload(draft)
        try {
          onIntent(editor.generation)
        } catch {
          setMessage('platform.supplierForm.intentUnavailable')
          return
        }
        frozen.current = input
        const outcome = await createSupplier(editor.generation, input)
        if (!alive.current) return
        frozen.current = null
        setUncertain(false)
        setCompleted(true)
        setDraft((previous) => ({ ...previous, secret: '' }))
        onIntent(null)
        onSaved()
        try {
          const created = await readSupplier(outcome.id)
          if (alive.current) setRecord(created)
        } catch {
          if (alive.current) setMessage('platform.supplierForm.savedUnavailable')
        }
      } else if (record && editor.mode === 'edit') {
        const input = payload(draft)
        optimistic = beginOptimistic<PlatformItem>(collection, {
          patch: { [record.id]: (row) => supplierRow(row, input) }
        })
        onWriting()
        const updated = await updateSupplier(record.id, record.updatedAt, input)
        optimistic.confirm()
        if (!alive.current) return optimistic.release()
        setRecord(updated)
        setDraft((previous) => ({ ...previous, secret: '' }))
        setCompleted(true)
        onSaved(optimistic.release)
        onWritten(true)
      } else if (record && editor.mode === 'delete') {
        optimistic = beginOptimistic<PlatformItem>(collection, { remove: [record.id] })
        onWriting()
        await deleteSupplier(record.id, record.updatedAt)
        optimistic.confirm()
        if (!alive.current) return optimistic.release()
        onSaved(optimistic.release)
        onWritten(true)
      }
    } catch (error) {
      optimistic?.rollback()
      if (!alive.current) return
      if (optimistic) onWritten(false)
      const status = error instanceof MemberOrgError ? error.status : undefined
      if (editor.mode === 'create') {
        const rejected = !wasUncertain && [400, 401, 404, 422].includes(status ?? 0)
        if (rejected) {
          frozen.current = null
          onIntent(null)
        } else setUncertain(true)
      } else if (![400, 401, 404, 422].includes(status ?? 0)) setConflict(true)
      if (status === 401 || status === 404) {
        frozen.current = null
        setDraft((previous) => ({ ...previous, secret: '' }))
        onUnavailable()
      } else setMessage(errorKey(error))
    } finally {
      if (alive.current) {
        gate.current = false
        setPending(false)
        onBusy(false)
      }
    }
  }
  const field = (name: keyof typeof labels, control: ReactNode, help?: string) => (
    <div className="grid gap-1.5">
      <Label htmlFor={`${fieldId}-${name}`}>
        {t(labels[name])}
        {['name', 'baseUrl', 'secret'].includes(name) ? <span aria-hidden> *</span> : null}
      </Label>
      {control}
      {help ? (
        <p id={`${fieldId}-${name}-help`} className="m-0 text-[12px] text-[#3f3f3f]">
          {t(help)}
        </p>
      ) : null}
      {errors[name] ? (
        <p id={`${fieldId}-${name}-error`} className="m-0 text-[12px] text-destructive">
          {t(errors[name])}
        </p>
      ) : null}
    </div>
  )
  const inputProps = (name: string) => ({
    id: `${fieldId}-${name}`,
    'aria-invalid': Boolean(errors[name]),
    'aria-describedby':
      [errors[name] ? `${fieldId}-${name}-error` : '', name === 'credential' ? `${fieldId}-${name}-help` : '']
        .filter(Boolean)
        .join(' ') || undefined
  })
  const select = (name: string, value: Option, options: readonly Option[], pick: (value: Option) => void) => (
    <Select
      value={value}
      onValueChange={(next) => {
        const selected = options.find((option) => option === next)
        if (selected) pick(selected)
      }}>
      <SelectTrigger {...inputProps(name)}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((value) => (
          <SelectItem key={value} value={value}>
            {t(optionLabels[value])}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
  const readOnly = editor.mode === 'read' || completed
  return (
    <Dialog
      open={editor.open}
      onOpenChange={(open) => {
        if (!open) close()
      }}>
      <DialogContent
        size="lg"
        className="top-[10dvh] max-h-[80dvh] translate-y-0 overflow-y-auto overscroll-contain [&_:focus-visible]:outline-2 [&_:focus-visible]:outline-offset-2 [&_:focus-visible]:outline-[#171717]"
        closeLabel={t('common.close')}
        showCloseButton={!pending}
        closeOnOverlayClick={!pending}
        onCloseAutoFocus={(event) => {
          event.preventDefault()
          onRestoreFocus()
          onClosed()
        }}
        onEscapeKeyDown={(event) => {
          if (pending) event.preventDefault()
        }}
        onOpenAutoFocus={(event) => {
          if (editor.mode === 'delete') {
            event.preventDefault()
            safeButton.current?.focus()
          }
        }}>
        <DialogHeader>
          <DialogTitle>
            {t(
              discard
                ? 'platform.supplierForm.discardTitle'
                : completed
                  ? 'platform.supplierForm.saved'
                  : titles[editor.mode],
              { name: record?.name ?? '' }
            )}
          </DialogTitle>
          <DialogDescription>
            {t(
              discard
                ? 'platform.supplierForm.discardDescription'
                : editor.mode === 'delete'
                  ? 'platform.supplierForm.deleteDescription'
                  : 'platform.supplierForm.description'
            )}
          </DialogDescription>
        </DialogHeader>
        {discard ? (
          <DialogFooter>
            <Button ref={safeButton} variant="outline" onClick={() => setDiscard(false)}>
              {t('platform.supplierForm.keepEditing')}
            </Button>
            <Button variant="destructive" onClick={() => onDismiss(false)}>
              {t('platform.supplierForm.discard')}
            </Button>
          </DialogFooter>
        ) : (
          <>
            {message ? (
              <p role="alert" className="m-0 text-[13px] text-[#3f3f3f]">
                {t(message)}
              </p>
            ) : null}
            {loading || (completed && pending && !record) ? (
              <p role="status" className={record || editor.mode === 'create' ? undefined : 'sr-only'}>
                {t('platform.supplierForm.loading')}
              </p>
            ) : null}
            {loading && !record && editor.mode !== 'create' ? <SummarySkeleton /> : null}
            {completed || editor.mode === 'read' ? (
              record ? (
                <>
                  <SupplierSummary record={record} includeModels={false} />
                  <SupplierRelations
                    key={`${record.id}:${record.updatedAt}`}
                    supplierId={record.id}
                    scope={scope}
                    reviewKey={`${editor.generation}/${record.updatedAt}`}
                    authorized
                    onUnavailable={onUnavailable}
                  />
                </>
              ) : null
            ) : editor.mode === 'delete' ? (
              record ? (
                <>
                  <SupplierSummary record={record} includeModels={false} />
                  <SupplierRelations
                    key={`${record.id}:${record.updatedAt}`}
                    supplierId={record.id}
                    scope={scope}
                    reviewKey={`${editor.generation}/${record.updatedAt}`}
                    authorized
                    initialKind="dependencies"
                    onReview={setDependencyReview}
                    onUnavailable={onUnavailable}
                  />
                  <p id={`${fieldId}-delete-eligibility`} role="status">
                    {t(
                      !dependencyReview
                        ? 'platform.supplierRelations.reviewPending'
                        : dependencyReview.revision !== record.updatedAt
                          ? 'platform.supplierRelations.stale'
                          : dependencyReview.total
                            ? 'platform.supplierRelations.deleteBlocked'
                            : 'platform.supplierRelations.deleteReady'
                    )}
                  </p>
                </>
              ) : null
            ) : !loading && (editor.mode === 'create' || record) ? (
              <form
                id={`${fieldId}-form`}
                onSubmit={(event) => {
                  event.preventDefault()
                  void submit()
                }}
                noValidate>
                {Object.keys(errors).length ? (
                  <div
                    ref={summary}
                    role="alert"
                    tabIndex={-1}
                    className="mb-4 rounded-[8px] border border-destructive p-3 text-[13px]">
                    <p className="mt-0 font-medium">{t('platform.supplierForm.checkFields')}</p>
                    <ul>
                      {Object.entries(errors).map(([name, error]) => (
                        <li key={name}>
                          <a
                            href={`#${fieldId}-${name}`}
                            onClick={(event) => {
                              event.preventDefault()
                              document.getElementById(`${fieldId}-${name}`)?.focus()
                            }}>
                            {t(error)}
                          </a>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                <fieldset
                  disabled={pending || uncertain}
                  className="m-0 grid min-w-0 grid-cols-1 gap-4 border-0 p-0 sm:grid-cols-2">
                  {field(
                    'name',
                    <Input
                      {...inputProps('name')}
                      value={draft.name}
                      required
                      onChange={(event) => change('name', event.target.value)}
                    />
                  )}
                  {field(
                    'kind',
                    select('kind', draft.kind, SUPPLIER_KINDS, (value) => change('kind', value as Draft['kind']))
                  )}
                  {field(
                    'protocol',
                    select('protocol', draft.protocol, SUPPLIER_PROTOCOLS, (value) =>
                      change('protocol', value as Draft['protocol'])
                    )
                  )}
                  {field(
                    'enabled',
                    select('enabled', draft.enabled ? 'yes' : 'no', ['yes', 'no'], (value) =>
                      change('enabled', value === 'yes')
                    )
                  )}
                  <div className="sm:col-span-2">
                    {field(
                      'baseUrl',
                      <Input
                        {...inputProps('baseUrl')}
                        value={draft.baseUrl}
                        required
                        type="url"
                        onChange={(event) => change('baseUrl', event.target.value)}
                      />
                    )}
                  </div>
                  {field(
                    'dataRegion',
                    <Input
                      {...inputProps('dataRegion')}
                      value={draft.dataRegion ?? ''}
                      onChange={(event) => change('dataRegion', event.target.value)}
                    />
                  )}
                  {field(
                    'retention',
                    select('retention', draft.retention, ['unknown', 'yes', 'no'], (value) =>
                      change('retention', value as Draft['retention'])
                    )
                  )}
                  {field(
                    'originVendors',
                    <Textarea.Input
                      {...inputProps('originVendors')}
                      value={draft.originVendors}
                      onValueChange={(value) => change('originVendors', value)}
                      aria-describedby={`${fieldId}-originVendors-help`}
                    />,
                    'platform.supplierForm.linesHelp'
                  )}
                  {field(
                    'models',
                    <Textarea.Input
                      {...inputProps('models')}
                      value={draft.models}
                      onValueChange={(value) => change('models', value)}
                      aria-describedby={`${fieldId}-models-help`}
                    />,
                    'platform.supplierForm.linesHelp'
                  )}
                  {field(
                    'credential',
                    select(
                      'credential',
                      draft.credential,
                      record?.hasKey ? ['keep', 'replace', 'clear'] : ['keep', 'replace'],
                      (value) => {
                        change('credential', value as Draft['credential'])
                        if (value !== 'replace') change('secret', '')
                      }
                    ),
                    record?.hasKey ? 'platform.supplierForm.keyPresent' : 'platform.supplierForm.keyAbsent'
                  )}
                  {draft.credential === 'replace'
                    ? field(
                        'secret',
                        <Input
                          {...inputProps('secret')}
                          type="password"
                          autoComplete="new-password"
                          required
                          value={draft.secret}
                          onChange={(event) => change('secret', event.target.value)}
                        />
                      )
                    : null}
                  {draft.credential === 'clear' ? (
                    <p role="note" className="m-0 text-[13px] text-destructive sm:col-span-2">
                      {t('platform.supplierForm.clearWarning')}
                    </p>
                  ) : null}
                </fieldset>
              </form>
            ) : null}
            {conflict ? (
              <section
                className="grid gap-2 rounded-[8px] bg-[#f5f5f5] p-3 text-[13px]"
                aria-label={t('platform.supplierForm.review')}>
                <p className="m-0">{t('platform.supplierForm.reviewDescription')}</p>
                {latest ? (
                  <>
                    <SupplierSummary record={latest} />
                    <Button
                      variant="outline"
                      onClick={() => {
                        setRecord(latest)
                        setDependencyReview(null)
                        setLatest(undefined)
                        setConflict(false)
                        setMessage('')
                      }}>
                      {t('platform.supplierForm.acceptRevision')}
                    </Button>
                  </>
                ) : (
                  <Button variant="outline" disabled={loading} onClick={() => void load(true)}>
                    {t('platform.supplierForm.readLatest')}
                  </Button>
                )}
              </section>
            ) : null}
            <DialogFooter>
              <Button ref={safeButton} variant="outline" disabled={pending} onClick={close}>
                {t(
                  uncertain && editor.mode === 'create'
                    ? 'platform.supplierForm.leavePending'
                    : readOnly
                      ? 'common.close'
                      : 'common.cancel'
                )}
              </Button>
              {!readOnly && !loading && (record || editor.mode === 'create') ? (
                <Button
                  variant={editor.mode === 'delete' ? 'destructive' : 'default'}
                  disabled={pending || conflict || (editor.mode === 'delete' && !canDelete)}
                  aria-describedby={editor.mode === 'delete' ? `${fieldId}-delete-eligibility` : undefined}
                  type={editor.mode === 'delete' ? 'button' : 'submit'}
                  form={`${fieldId}-form`}
                  onClick={editor.mode === 'delete' ? () => void submit() : undefined}>
                  {t(
                    pending
                      ? 'platform.supplierForm.submitting'
                      : uncertain
                        ? 'platform.supplierForm.retryIntent'
                        : editor.mode === 'delete'
                          ? 'platform.supplierForm.confirmDelete'
                          : 'platform.supplierForm.save'
                  )}
                </Button>
              ) : null}
              {!record && editor.mode !== 'create' && !loading && !completed ? (
                <Button variant="outline" onClick={() => void load()}>
                  {t('common.retry')}
                </Button>
              ) : null}
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

const SUMMARY_ROWS = 11

/** Holds the summary's height while the record loads, so the dialog does not resize under its entry animation. */
function SummarySkeleton() {
  return (
    <div aria-hidden className="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)] gap-x-4 gap-y-2">
      {Array.from({ length: SUMMARY_ROWS }, (_, index) => (
        <div key={index} className="contents">
          <span className="my-1 h-3 w-16 animate-pulse rounded-full bg-[#f2f3f5]" />
          <span className="my-1 h-3 w-1/2 animate-pulse rounded-full bg-[#f2f3f5]" />
        </div>
      ))}
    </div>
  )
}

function SupplierSummary({ record, includeModels = true }: { record: SupplierRecord; includeModels?: boolean }) {
  const { t } = useTranslation()
  const fields = {
    name: record.name,
    id: record.id,
    kind: t(optionLabels[record.kind]),
    protocol: t(optionLabels[record.protocol]),
    baseUrl: record.baseUrl,
    dataRegion: record.dataRegion ?? '—',
    retention: t(optionLabels[record.retainsData === null ? 'unknown' : record.retainsData ? 'yes' : 'no']),
    originVendors: record.originVendors.join('\n') || '—',
    ...(includeModels ? { models: record.models.join('\n') || '—' } : {}),
    enabled: t(optionLabels[record.enabled ? 'yes' : 'no']),
    credential: t(record.hasKey ? 'platform.supplierForm.keyPresent' : 'platform.supplierForm.keyAbsent'),
    revision: String(record.updatedAt)
  }
  return (
    <dl className="m-0 grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)] gap-x-4 gap-y-2 text-[13px]">
      {Object.entries(fields).map(([key, value]) => (
        <div key={key} className="contents">
          <dt className="text-[#3f3f3f]">{t(labels[key as keyof typeof labels])}</dt>
          <dd className="m-0 break-words whitespace-pre-wrap text-[#171717]">{value}</dd>
        </div>
      ))}
    </dl>
  )
}
