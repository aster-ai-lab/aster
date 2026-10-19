import { Checkbox, Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { Plus, ShieldCheck, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { OrgCard } from './OrgAdminFrame'
import { filledButtonClassName, ghostIconButtonClassName, orgFieldClassName, quietButtonClassName } from './orgUi'
import {
  listProtections,
  MAX_APPROVALS,
  parseCheckNames,
  protectionProblem,
  repoErrorKey,
  setProtections,
  type Protection,
  type RepositoryDetail
} from './repositoryApi'
import { LoadFailed } from './repositoryUi'

const FLAGS = ['no_force_push', 'no_delete', 'require_change_request'] as const

/** 保护规则：所有能读的人可见，仓库管理员可改。 */
export function OrgProtectionsCard({ namespace, repository }: { namespace: string; repository: RepositoryDetail }) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const { data, error, mutate } = useSWR(`studio/org-repo-protections:${namespace}/${repository.name}`, () =>
    listProtections(namespace, repository.name)
  )
  const canEdit = repository.permissions.admin && !repository.archived
  return (
    <OrgCard
      ui="org.repo.protections"
      title={
        <span className="flex items-center justify-between gap-3">
          <span>{t('org.repos.protect.title')}</span>
          {canEdit && data ? (
            <button
              type="button"
              data-ui="org.repo.protections.edit"
              className={quietButtonClassName(30)}
              onClick={() => setEditing(true)}>
              {t('org.repos.protect.edit')}
            </button>
          ) : null}
        </span>
      }>
      {error ? (
        <LoadFailed onRetry={() => void mutate()} />
      ) : !data ? (
        <div className="h-16 animate-pulse rounded-[8px] bg-[#fafafa]" />
      ) : data.length === 0 ? (
        <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.repos.protect.none')}</p>
      ) : (
        <ul className="m-0 grid list-none gap-2 p-0">
          {data.map((rule) => (
            <li key={rule.pattern} className="flex flex-wrap items-center gap-2 text-[13px]">
              <ShieldCheck className="size-3.5 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
              <span className="font-mono text-[12px] text-[#171717]">{rule.pattern}</span>
              {describe(rule).map((label) => (
                <span key={label.key} className="rounded-full bg-[#f2f2f2] px-2 py-0.5 text-[11px] text-[#6b6b6b]">
                  {t(label.key, label.values)}
                </span>
              ))}
            </li>
          ))}
        </ul>
      )}
      {editing && data ? (
        <ProtectionsDialog
          initial={data}
          onClose={() => setEditing(false)}
          onSave={async (next) => {
            const saved = await setProtections(namespace, repository.name, next)
            await mutate(saved, { revalidate: false })
          }}
        />
      ) : null}
    </OrgCard>
  )
}

function describe(rule: Protection) {
  const labels: { key: string; values?: Record<string, unknown> }[] = []
  for (const flag of FLAGS) if (rule[flag]) labels.push({ key: `org.repos.protect.${flag}` })
  if (rule.require_approvals > 0) {
    labels.push({ key: 'org.repos.protect.approvalsN', values: { count: rule.require_approvals } })
  }
  if (rule.require_checks.length > 0) {
    labels.push({ key: 'org.repos.protect.checksN', values: { checks: rule.require_checks.join(', ') } })
  }
  return labels
}

type Draft = Omit<Protection, 'require_checks' | 'require_approvals'> & { checks: string; approvals: string }

const toDraft = (rule: Protection): Draft => ({
  pattern: rule.pattern,
  no_force_push: rule.no_force_push,
  no_delete: rule.no_delete,
  require_change_request: rule.require_change_request,
  checks: rule.require_checks.join(', '),
  approvals: String(rule.require_approvals)
})

const fromDraft = (draft: Draft): Protection => ({
  pattern: draft.pattern.trim(),
  no_force_push: draft.no_force_push,
  no_delete: draft.no_delete,
  require_change_request: draft.require_change_request,
  require_checks: parseCheckNames(draft.checks),
  require_approvals: draft.approvals.trim() === '' ? 0 : Number(draft.approvals)
})

function ProtectionsDialog({
  initial,
  onClose,
  onSave
}: {
  initial: Protection[]
  onClose: () => void
  onSave: (next: Protection[]) => Promise<void>
}) {
  const { t } = useTranslation()
  const [drafts, setDrafts] = useState<Draft[]>(() => initial.map(toDraft))
  const [problem, setProblem] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const update = (index: number, next: Partial<Draft>) => {
    setProblem(null)
    setDrafts((current) => current.map((draft, at) => (at === index ? { ...draft, ...next } : draft)))
  }

  async function save() {
    const next = drafts.map(fromDraft)
    const local = protectionProblem(next)
    if (local) {
      setProblem(local)
      return
    }
    setSaving(true)
    try {
      await onSave(next)
      toast.success(t('org.repos.protect.saved'))
      onClose()
    } catch (err) {
      setProblem(repoErrorKey(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !saving && onClose()}>
      <DialogContent className="max-w-[640px] gap-0 p-0">
        <div data-ui="org.repo.protections.dialog" className="grid gap-4 p-6 pr-12">
          <div>
            <DialogTitle className="text-[16px] font-semibold">{t('org.repos.protect.title')}</DialogTitle>
            <p className="m-0 mt-1 text-[13px] text-[#8a8a8a]">{t('org.repos.protect.hint')}</p>
          </div>
          <div className="grid max-h-[420px] gap-3 overflow-y-auto">
            {drafts.map((draft, index) => (
              <div key={index} className="grid gap-2 rounded-[10px] border border-[#efefef] p-3">
                <div className="flex items-center gap-2">
                  <Input
                    aria-label={t('org.repos.protect.pattern')}
                    placeholder="refs/heads/main"
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                    value={draft.pattern}
                    className={cn(orgFieldClassName, 'flex-1 font-mono text-[12px]')}
                    onChange={(event) => update(index, { pattern: event.target.value })}
                  />
                  <button
                    type="button"
                    aria-label={t('org.repos.protect.remove')}
                    title={t('org.repos.protect.remove')}
                    className={ghostIconButtonClassName}
                    onClick={() => setDrafts((current) => current.filter((_, at) => at !== index))}>
                    <Trash2 className="size-4" strokeWidth={1.6} />
                  </button>
                </div>
                <div className="flex flex-wrap gap-x-4 gap-y-2">
                  {FLAGS.map((flag) => (
                    <label key={flag} className="flex items-center gap-2 text-[13px] text-[#3f3f3f]">
                      <Checkbox
                        checked={draft[flag]}
                        onCheckedChange={(checked) => update(index, { [flag]: checked === true })}
                      />
                      {t(`org.repos.protect.${flag}`)}
                    </label>
                  ))}
                </div>
                <div className="grid grid-cols-[140px_1fr] gap-2">
                  <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
                    {t('org.repos.protect.approvals', { max: MAX_APPROVALS })}
                    <Input
                      aria-label={t('org.repos.protect.approvals', { max: MAX_APPROVALS })}
                      inputMode="numeric"
                      value={draft.approvals}
                      className={orgFieldClassName}
                      onChange={(event) => update(index, { approvals: event.target.value.replace(/[^0-9]/g, '') })}
                    />
                  </label>
                  <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
                    {t('org.repos.protect.checks')}
                    <Input
                      aria-label={t('org.repos.protect.checks')}
                      placeholder="build, test"
                      autoCapitalize="off"
                      autoCorrect="off"
                      spellCheck={false}
                      value={draft.checks}
                      className={cn(orgFieldClassName, 'font-mono text-[12px]')}
                      onChange={(event) => update(index, { checks: event.target.value })}
                    />
                  </label>
                </div>
              </div>
            ))}
            <button
              type="button"
              className={cn(quietButtonClassName(30), 'w-fit')}
              onClick={() =>
                setDrafts((current) => [
                  ...current,
                  {
                    pattern: '',
                    no_force_push: true,
                    no_delete: true,
                    require_change_request: false,
                    checks: '',
                    approvals: '0'
                  }
                ])
              }>
              <Plus className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('org.repos.protect.add')}
            </button>
          </div>
          {problem ? (
            <p role="alert" className="m-0 text-[13px] text-[#c4321b]">
              {t(problem)}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <button type="button" disabled={saving} className={quietButtonClassName(36)} onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button
              type="button"
              data-ui="org.repo.protections.save"
              disabled={saving}
              className={filledButtonClassName(36)}
              onClick={() => void save()}>
              {t('common.save')}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
