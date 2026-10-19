import { Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { Blocks, Check, Plus, X } from 'lucide-react'
import { useDeferredValue, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { PageEntry } from '@renderer/components/layout/PageEntry'
import { useRestingDialog } from '@renderer/components/restingDialog'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { listCapabilities, type CapabilityEntry } from '@renderer/pages/capabilities/capabilitiesData'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  allowanceKey,
  getCapabilityPolicy,
  samePolicy,
  saveCapabilityPolicy,
  TEAM_CAPABILITY_ALLOWLIST_MAX,
  type CapabilityAllowance,
  type CapabilityAllowanceKind,
  type TeamCapabilityPolicy
} from './orgAdminApi'
import { OrgAdminFrame, OrgCard } from './OrgAdminFrame'
import { filledButtonClassName, quietButtonClassName, rowActionClassName } from './orgUi'

const KINDS: readonly CapabilityAllowanceKind[] = ['connector', 'skill', 'plugin']
/** The catalog's largest page; names past it fall back to the slug. */
const NAME_PAGE = 100
const PICK_PAGE = 24

type Draft = { restricted: boolean; allowed: CapabilityAllowance[] }
type PolicyKey = readonly ['studio/org-capability-policy', string]

function denied(err: unknown) {
  return err instanceof MemberOrgError && (err.status === 404 || err.status === 403)
}

function stale(err: unknown) {
  return err instanceof MemberOrgError && err.status === 409
}

async function catalogNames(): Promise<Map<string, string>> {
  const pages = await Promise.all(KINDS.map((kind) => listCapabilities({ kind, pageSize: NAME_PAGE })))
  return new Map(pages.flatMap((page) => page.items.map((item) => [allowanceKey(item), item.name] as const)))
}

/** 团队能力：团队决定成员可以安装哪些连接器、技能和插件。 */
export function OrgCapabilitiesPanel({ orgId }: { orgId: string }) {
  const { t } = useTranslation()
  const key: PolicyKey = ['studio/org-capability-policy', orgId]
  const {
    data: saved,
    error,
    mutate
  } = useSWR(key, ([, org]) => getCapabilityPolicy(org), {
    revalidateOnFocus: false
  })
  const { data: names } = useSWR(saved ? 'studio/org-capability-names' : null, catalogNames, {
    revalidateOnFocus: false
  })
  const [draft, setDraft] = useState<Draft | null>(null)
  const [saving, setSaving] = useState(false)
  const [picking, setPicking] = useState(false)
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set())
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const current: Draft | null = draft ?? (saved ? { restricted: saved.restricted, allowed: saved.allowed } : null)
  const dirty = !!draft && !!saved && !samePolicy(draft, saved)
  const savePresence = useRestingDialog(dirty)
  // Installs name only the slug, so one slug is allowed once whatever its kind.
  const chosen = new Set(current?.allowed.map((entry) => entry.slug) ?? [])
  const full = (current?.allowed.length ?? 0) >= TEAM_CAPABILITY_ALLOWLIST_MAX

  function edit(change: (base: Draft) => Draft) {
    if (!current || saving) return
    setDraft(change(current))
  }

  function toggle(entry: CapabilityAllowance) {
    const picked = chosen.has(entry.slug)
    edit((base) =>
      picked
        ? { ...base, allowed: base.allowed.filter((row) => row.slug !== entry.slug) }
        : { ...base, allowed: [...base.allowed, { kind: entry.kind, slug: entry.slug }] }
    )
    if (!picked) setAdded((prev) => new Set(prev).add(allowanceKey(entry)))
  }

  async function save() {
    if (!draft || !saved || saving || !dirty) return
    const previous = saved
    const optimistic: TeamCapabilityPolicy = {
      ...saved,
      restricted: draft.restricted,
      allowed: draft.allowed,
      revision: saved.revision + 1
    }
    setSaving(true)
    setDraft(null)
    await mutate(optimistic, { revalidate: false })
    try {
      const next = await saveCapabilityPolicy(orgId, { ...draft, expected_revision: previous.revision })
      await mutate(next, { revalidate: false })
      if (mounted.current) toast.success(t('org.capabilities.saved'))
    } catch (err) {
      if (stale(err)) {
        await mutate()
        if (mounted.current) toast.error(t('org.capabilities.stale'))
      } else {
        await mutate(previous, { revalidate: false })
        if (mounted.current) {
          setDraft(draft)
          toast.error(t(denied(err) ? 'org.capabilities.denied' : 'org.error.generic'))
        }
      }
    } finally {
      if (mounted.current) setSaving(false)
    }
  }

  const nameOf = (entry: CapabilityAllowance) => names?.get(allowanceKey(entry)) ?? entry.slug

  return (
    <OrgAdminFrame
      ui="org.capabilities"
      title={t('org.nav.capabilities')}
      description={t('org.capabilities.description')}
      className="gap-4">
      {error && !saved ? (
        <div data-ui="org.capabilities.error" className="flex flex-1 flex-col items-center justify-center gap-3 py-16">
          <p className="m-0 max-w-sm text-center text-[13px] text-[#8a8a8a]">
            {t(denied(error) ? 'org.capabilities.denied' : 'org.error.generic')}
          </p>
          {denied(error) ? null : (
            <button type="button" onClick={() => void mutate()} className={quietButtonClassName(30)}>
              {t('common.retry')}
            </button>
          )}
        </div>
      ) : !current ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.capabilities" ready={false} />
        </div>
      ) : (
        <>
          <OrgCard ui="org.capabilities.mode" title={t('org.capabilities.modeTitle')}>
            <div role="radiogroup" aria-label={t('org.capabilities.modeTitle')} className="grid gap-2 sm:grid-cols-2">
              {([false, true] as const).map((restricted) => {
                const mode = restricted ? 'restricted' : 'open'
                const checked = current.restricted === restricted
                return (
                  <button
                    key={mode}
                    type="button"
                    role="radio"
                    aria-checked={checked}
                    data-ui={`org.capabilities.mode.${mode}`}
                    disabled={saving}
                    onClick={() => edit((base) => ({ ...base, restricted }))}
                    className={cn(
                      'flex flex-col items-start gap-1 rounded-[10px] border px-4 py-3 text-left transition-colors outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                      checked ? 'border-[#171717] bg-[#fafafa]' : 'border-[#efefef] hover:bg-[#f7f7f7]'
                    )}>
                    <span className="flex items-center gap-2 text-[13px] font-medium text-[#171717]">
                      <span
                        aria-hidden
                        className={cn(
                          'flex size-3.5 items-center justify-center rounded-full border',
                          checked ? 'border-[#171717]' : 'border-[#c4c4c4]'
                        )}>
                        {checked ? <span className="size-1.5 rounded-full bg-[#171717]" /> : null}
                      </span>
                      {t(`org.capabilities.mode.${mode}`)}
                    </span>
                    <span className="pl-5.5 text-[12px] leading-5 text-[#8a8a8a]">
                      {t(`org.capabilities.mode.${mode}Hint`)}
                    </span>
                  </button>
                )
              })}
            </div>
          </OrgCard>

          <OrgCard
            ui="org.capabilities.list"
            title={
              <span className="flex items-center justify-between gap-3">
                <span>{t('org.capabilities.listTitle', { count: current.allowed.length })}</span>
                <button
                  type="button"
                  data-ui="org.capabilities.add"
                  disabled={saving || full}
                  title={full ? t('org.capabilities.full', { max: TEAM_CAPABILITY_ALLOWLIST_MAX }) : undefined}
                  onClick={() => setPicking(true)}
                  className={quietButtonClassName(30)}>
                  <Plus className="size-3.5" aria-hidden />
                  {t('org.capabilities.add')}
                </button>
              </span>
            }>
            {current.restricted ? null : (
              <p data-ui="org.capabilities.inactive" className="m-0 mb-3 text-[12px] leading-5 text-[#8a8a8a]">
                {t('org.capabilities.listInactive')}
              </p>
            )}
            {current.allowed.length === 0 ? (
              <div
                data-ui="org.capabilities.empty"
                className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <Blocks className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
                <p className="m-0 text-[13px] text-[#3f3f3f]">{t('org.capabilities.empty')}</p>
                {current.restricted ? (
                  <p className="m-0 max-w-sm text-[12px] leading-5 text-[#8a8a8a]">
                    {t('org.capabilities.emptyRestricted')}
                  </p>
                ) : null}
              </div>
            ) : (
              <ul
                aria-label={t('org.capabilities.listLabel')}
                className={cn('m-0 list-none p-0', !current.restricted && 'opacity-60')}>
                {current.allowed.map((entry) => {
                  const id = allowanceKey(entry)
                  const name = nameOf(entry)
                  return (
                    <li
                      key={id}
                      data-capability={id}
                      className={cn(
                        'group flex h-11 items-center gap-3 border-b border-[#f3f3f3] px-1 last:border-b-0',
                        added.has(id) && 'a3s-row-enter'
                      )}>
                      <span className="min-w-0 flex-1 truncate text-[13px] text-[#1a1a1a]">{name}</span>
                      <span className="shrink-0 rounded-[6px] bg-[#f2f3f5] px-1.5 py-0.5 text-[11px] text-[#3f3f3f]">
                        {t(`org.capabilities.kind.${entry.kind}`)}
                      </span>
                      <span className="w-40 shrink-0 truncate font-mono text-[12px] text-[#8a8a8a]">{entry.slug}</span>
                      <button
                        type="button"
                        aria-label={t('org.capabilities.remove', { name })}
                        title={t('org.capabilities.remove', { name })}
                        disabled={saving}
                        onClick={() => toggle(entry)}
                        className={rowActionClassName}>
                        <X className="size-3.5" aria-hidden />
                      </button>
                    </li>
                  )
                })}
              </ul>
            )}
          </OrgCard>

          {savePresence.present ? (
            <div className="pointer-events-none sticky bottom-4 mt-auto flex justify-center pt-2">
              <section
                data-ui="org.capabilities.save-bar"
                data-state={savePresence.state}
                aria-label={t('org.capabilities.unsaved')}
                className={`a3s-popup-rise flex items-center gap-2 rounded-[12px] border border-[#ececec] bg-white py-1.5 pr-1.5 pl-4 shadow-[0_12px_32px_rgba(0,0,0,0.12)] ${
                  savePresence.state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
                }`}>
                <span role="status" className="mr-2 text-[13px] whitespace-nowrap text-[#171717]">
                  {t('org.capabilities.unsaved')}
                </span>
                <button
                  type="button"
                  data-ui="org.capabilities.discard"
                  disabled={saving}
                  onClick={() => setDraft(null)}
                  className={quietButtonClassName(30)}>
                  {t('org.capabilities.discard')}
                </button>
                <button
                  type="button"
                  data-ui="org.capabilities.save"
                  disabled={saving}
                  aria-busy={saving || undefined}
                  onClick={() => void save()}
                  className={filledButtonClassName(30)}>
                  {t('org.capabilities.save')}
                </button>
              </section>
            </div>
          ) : null}

          <CapabilityPicker open={picking} onOpenChange={setPicking} chosen={chosen} full={full} onToggle={toggle} />
        </>
      )}
    </OrgAdminFrame>
  )
}

function CapabilityPicker({
  open,
  onOpenChange,
  chosen,
  full,
  onToggle
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  chosen: ReadonlySet<string>
  full: boolean
  onToggle: (entry: CapabilityEntry) => void
}) {
  const { t } = useTranslation()
  const [kind, setKind] = useState<CapabilityAllowanceKind>('connector')
  const [query, setQuery] = useState('')
  const q = useDeferredValue(query.trim())
  const { data, error, isLoading } = useSWR(
    open ? (['studio/org-capability-pick', kind, q] as const) : null,
    ([, k, text]) => listCapabilities({ kind: k, q: text, pageSize: PICK_PAGE }),
    { revalidateOnFocus: false, keepPreviousData: true }
  )

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} data-ui="org.capabilities.picker" className="sm:max-w-[520px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.capabilities.picker.title')}</DialogTitle>
        <div role="tablist" aria-label={t('org.capabilities.picker.kinds')} className="flex gap-1">
          {KINDS.map((option) => (
            <button
              key={option}
              type="button"
              role="tab"
              aria-selected={kind === option}
              data-ui={`org.capabilities.picker.kind.${option}`}
              onClick={() => setKind(option)}
              className={cn(
                'h-[30px] rounded-[8px] px-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                kind === option ? 'bg-[#171717] text-white' : 'text-[#3f3f3f] hover:bg-[#f2f3f5]'
              )}>
              {t(`org.capabilities.kind.${option}`)}
            </button>
          ))}
        </div>
        <Input
          data-ui="org.capabilities.picker.search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('org.capabilities.picker.search')}
          aria-label={t('org.capabilities.picker.search')}
          className="h-9"
        />
        <ul
          aria-label={t(`org.capabilities.kind.${kind}`)}
          aria-busy={isLoading || undefined}
          className="m-0 max-h-[360px] min-h-40 list-none overflow-auto p-0">
          {error ? (
            <li className="py-10 text-center text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</li>
          ) : data && data.items.length === 0 ? (
            <li className="py-10 text-center text-[13px] text-[#8a8a8a]">{t('org.capabilities.picker.empty')}</li>
          ) : (
            (data?.items ?? []).map((item) => {
              const picked = chosen.has(item.slug)
              return (
                <li
                  key={allowanceKey(item)}
                  data-capability={allowanceKey(item)}
                  className="flex h-12 items-center gap-3 border-b border-[#f3f3f3] px-1 last:border-b-0">
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-[13px] text-[#1a1a1a]">{item.name}</span>
                    <span className="truncate font-mono text-[11px] text-[#8a8a8a]">{item.slug}</span>
                  </span>
                  <button
                    type="button"
                    aria-pressed={picked}
                    aria-label={t(picked ? 'org.capabilities.picker.unpick' : 'org.capabilities.picker.pick', {
                      name: item.name
                    })}
                    disabled={!picked && full}
                    onClick={() => onToggle(item)}
                    className={cn(quietButtonClassName(30), picked && 'bg-[#f2f3f5] text-[#171717]')}>
                    {picked ? <Check className="size-3.5" aria-hidden /> : <Plus className="size-3.5" aria-hidden />}
                    {t(picked ? 'org.capabilities.picker.added' : 'org.capabilities.picker.add')}
                  </button>
                </li>
              )
            })
          )}
        </ul>
        <div className="flex justify-end">
          <button
            type="button"
            data-ui="org.capabilities.picker.done"
            onClick={() => onOpenChange(false)}
            className={filledButtonClassName(30)}>
            {t('org.capabilities.picker.done')}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
