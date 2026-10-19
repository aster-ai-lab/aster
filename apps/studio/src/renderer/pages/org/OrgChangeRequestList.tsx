import {
  Dialog,
  DialogContent,
  DialogTitle,
  Input,
  rowMotionClassName,
  rowMotionStyle,
  SearchSelect,
  SegmentedControl,
  Textarea
} from '@a3s-lab/ui'
import { GitPullRequest, Plus } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import type { OrgMemberRow } from './orgApi'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName } from './orgUi'
import {
  CHANGE_REQUEST_STATES,
  listBranches,
  listChangeRequests,
  openChangeRequest,
  repoErrorKey,
  type ChangeRequestState,
  type RepositoryDetail
} from './repositoryApi'
import { LoadFailed, memberLabel, StateBadge, useRelativeTime } from './repositoryUi'

const PAGE_SIZE = 20

/** 合并请求列表：按状态分段，默认看开放中的。 */
export function OrgChangeRequestList({
  namespace,
  repository,
  members,
  onOpen
}: {
  namespace: string
  repository: RepositoryDetail
  members: OrgMemberRow[]
  onOpen: (number: number) => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const [state, setState] = useState<ChangeRequestState>('open')
  const [page, setPage] = useState(1)
  const [creating, setCreating] = useState(false)
  const { data, error, mutate } = useSWR(`studio/org-repo-crs:${namespace}/${repository.name}:${state}`, () =>
    listChangeRequests(namespace, repository.name, state)
  )
  const rows = data ?? []
  const pages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE))
  const current = Math.min(page, pages)
  const visible = rows.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE)
  const canOpen = repository.permissions.write && !repository.archived

  return (
    <div data-ui="org.repo.requests" className="grid gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl
          aria-label={t('org.repos.cr.states')}
          size="sm"
          className="h-[30px]"
          value={state}
          options={CHANGE_REQUEST_STATES.map((value) => ({ value, label: t(`org.repos.cr.state.${value}`) }))}
          onValueChange={(next) => {
            setState(next)
            setPage(1)
          }}
        />
        {canOpen ? (
          <button
            type="button"
            data-ui="org.repo.requests.new"
            className={cn(filledButtonClassName(30), 'ml-auto')}
            onClick={() => setCreating(true)}>
            <Plus className="size-3.5 text-current" strokeWidth={1.8} aria-hidden />
            {t('org.repos.cr.new')}
          </button>
        ) : null}
      </div>
      {error ? (
        <LoadFailed onRetry={() => void mutate()} />
      ) : !data ? (
        <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
      ) : rows.length === 0 ? (
        <div className="flex flex-col items-center gap-2 py-12 text-center">
          <GitPullRequest className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t(`org.repos.cr.empty.${state}`)}</p>
        </div>
      ) : (
        <div className="overflow-hidden rounded-[10px] border border-[#efefef]">
          {visible.map((request, index) => (
            <button
              key={request.number}
              type="button"
              data-ui="org.repo.requests.row"
              style={rowMotionStyle(index)}
              className={cn(
                'flex w-full items-center gap-3 border-0 border-b border-[#f3f3f3] bg-transparent px-3 py-2.5 text-left last:border-b-0 hover:bg-[#f7f7f7]',
                rowMotionClassName()
              )}
              onClick={() => onOpen(request.number)}>
              <span className="w-10 shrink-0 font-mono text-[12px] text-[#8a8a8a]">#{request.number}</span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] text-[#171717]">{request.title}</span>
                <span className="block truncate text-[12px] text-[#8a8a8a]">
                  {t('org.repos.cr.byline', {
                    author: memberLabel(members, request.author_member),
                    source: request.source,
                    target: request.target
                  })}
                </span>
              </span>
              <StateBadge state={request.state} />
              <span className="w-24 shrink-0 text-right text-[12px] text-[#8a8a8a]">{when(request.updated_at)}</span>
            </button>
          ))}
        </div>
      )}
      {rows.length > PAGE_SIZE ? (
        <CollectionPagination page={current} pageSize={PAGE_SIZE} total={rows.length} onPageChange={setPage} />
      ) : null}
      {creating ? (
        <OpenChangeRequestDialog
          namespace={namespace}
          repository={repository}
          onClose={() => setCreating(false)}
          onOpened={(number) => {
            setCreating(false)
            void mutate()
            onOpen(number)
          }}
        />
      ) : null}
    </div>
  )
}

function OpenChangeRequestDialog({
  namespace,
  repository,
  onClose,
  onOpened
}: {
  namespace: string
  repository: RepositoryDetail
  onClose: () => void
  onOpened: (number: number) => void
}) {
  const { t } = useTranslation()
  const { data: branches } = useSWR(`studio/org-repo-branches:${namespace}/${repository.name}`, () =>
    listBranches(namespace, repository.name)
  )
  const [source, setSource] = useState('')
  const [target, setTarget] = useState(repository.default_branch)
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const options = (branches ?? []).map((branch) => ({ value: branch.name, label: branch.name }))
  const ready = source !== '' && target !== '' && source !== target && title.trim() !== ''

  async function submit() {
    if (!ready || saving) return
    setSaving(true)
    setProblem(null)
    try {
      const opened = await openChangeRequest(namespace, repository.name, {
        title: title.trim(),
        body: body.trim(),
        source,
        target
      })
      toast.success(t('org.repos.cr.opened', { number: opened.number }))
      onOpened(opened.number)
    } catch (err) {
      setProblem(repoErrorKey(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !saving && onClose()}>
      <DialogContent className="max-w-[560px] gap-0 p-0">
        <form
          data-ui="org.repo.requests.dialog"
          className="grid gap-4 p-6 pr-12"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <DialogTitle className="text-[16px] font-semibold">{t('org.repos.cr.new')}</DialogTitle>
          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
              {t('org.repos.cr.source')}
              <SearchSelect
                aria-label={t('org.repos.cr.source')}
                allowClear={false}
                placeholder={t('org.repos.cr.pickBranch')}
                value={source}
                options={options.filter((option) => option.value !== target)}
                onChange={(next) => {
                  setSource(next)
                  setProblem(null)
                }}
              />
            </label>
            <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
              {t('org.repos.cr.target')}
              <SearchSelect
                aria-label={t('org.repos.cr.target')}
                allowClear={false}
                value={target}
                options={options.filter((option) => option.value !== source)}
                onChange={(next) => {
                  setTarget(next)
                  setProblem(null)
                }}
              />
            </label>
          </div>
          <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
            {t('org.repos.cr.titleLabel')}
            <Input
              aria-label={t('org.repos.cr.titleLabel')}
              value={title}
              maxLength={200}
              className={orgFieldClassName}
              onChange={(event) => setTitle(event.target.value)}
            />
          </label>
          <label className="grid gap-1 text-[12px] text-[#8a8a8a]">
            {t('org.repos.cr.body')}
            <Textarea.Input
              aria-label={t('org.repos.cr.body')}
              value={body}
              rows={5}
              maxLength={8000}
              onChange={(event) => setBody(event.target.value)}
            />
          </label>
          {problem ? (
            <p role="alert" className="m-0 text-[13px] text-[#c4321b]">
              {t(problem)}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <button type="button" disabled={saving} className={quietButtonClassName(36)} onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={!ready || saving} className={filledButtonClassName(36)}>
              {t('org.repos.cr.submit')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
