import {
  Dialog,
  DialogContent,
  DialogTitle,
  EmptyState,
  Input,
  rowMotionClassName,
  rowMotionStyle,
  SearchSelect
} from '@a3s-lab/ui'
import { FolderGit2, Lock, Plus, RotateCw, Search, Users } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { cn } from '@renderer/utils/style'

import { OrgAdminFrame } from './OrgAdminFrame'
import type { OrgMemberRow } from './orgApi'
import { OrgChangeRequestDetail } from './OrgChangeRequestDetail'
import { OrgRepositoryDetail } from './OrgRepositoryDetail'
import { filledButtonClassName, ghostIconButtonClassName, orgFieldClassName, quietButtonClassName } from './orgUi'
import {
  createRepository,
  getTeamNamespace,
  isRepositoryName,
  LIST_VIEW,
  listRepositories,
  readRepoView,
  repoErrorKey,
  VISIBILITIES,
  writeRepoView,
  type Repository,
  type RepoView,
  type Visibility
} from './repositoryApi'
import { LoadFailed, useRelativeTime } from './repositoryUi'

const PAGE_SIZE = 20

/** 代码仓库：团队命名空间下自己能读的仓库。团队还没有命名空间时整页给出说明，不出工具条。 */
export function OrgRepositoriesPanel({
  orgId,
  members,
  onOpenTokens
}: {
  orgId: string | null
  members: OrgMemberRow[]
  onOpenTokens: () => void
}) {
  const { t } = useTranslation()
  const [view, setViewState] = useState<RepoView>(() => readRepoView(orgId))
  useEffect(() => setViewState(readRepoView(orgId)), [orgId])
  const setView = (next: RepoView) => {
    setViewState(next)
    writeRepoView(orgId, next)
  }
  const {
    data: team,
    error: teamError,
    mutate: reloadTeam
  } = useSWR(orgId ? `studio/org-repo-namespace:${orgId}` : null, () => getTeamNamespace(orgId!))
  const namespace = team?.namespace ?? null

  if (!orgId) return null
  if (teamError) {
    return (
      <OrgAdminFrame ui="org.repositories" title={t('org.nav.repositories')}>
        <LoadFailed onRetry={() => void reloadTeam()} />
      </OrgAdminFrame>
    )
  }
  if (!team) {
    return (
      <OrgAdminFrame ui="org.repositories" title={t('org.nav.repositories')}>
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.repositories" ready={false} />
        </div>
      </OrgAdminFrame>
    )
  }
  if (!namespace) {
    return (
      <OrgAdminFrame ui="org.repositories" title={t('org.nav.repositories')}>
        <EmptyState
          icon={FolderGit2}
          title={t('org.repos.noNamespaceTitle')}
          description={t('org.repos.noNamespace')}
        />
      </OrgAdminFrame>
    )
  }
  if (view.kind === 'request') {
    return (
      <OrgChangeRequestDetail
        namespace={namespace}
        name={view.name}
        number={view.number}
        members={members}
        onBack={(tab) =>
          setView({ kind: 'repo', name: view.name, tab, ref: null, path: '', blob: false, commit: null })
        }
        onList={() => setView(LIST_VIEW)}
      />
    )
  }
  if (view.kind === 'repo') {
    return (
      <OrgRepositoryDetail
        namespace={namespace}
        view={view}
        members={members}
        onView={setView}
        onList={() => setView(LIST_VIEW)}
        onOpenTokens={onOpenTokens}
      />
    )
  }
  return (
    <RepositoryList
      namespace={namespace}
      canCreate={team.permissions.admin}
      onOpen={(name) => setView({ kind: 'repo', name, tab: 'code', ref: null, path: '', blob: false, commit: null })}
    />
  )
}

function RepositoryList({
  namespace,
  canCreate,
  onOpen
}: {
  namespace: string
  canCreate: boolean
  onOpen: (name: string) => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const { data, error, isValidating, mutate } = useSWR(`studio/org-repos:${namespace}`, () =>
    listRepositories(namespace)
  )
  const [search, setSearch] = useState('')
  const [visibility, setVisibility] = useState<Visibility | ''>('')
  const [page, setPage] = useState(1)
  const [creating, setCreating] = useState(false)
  const rows = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (data ?? []).filter(
      (row) => (!needle || row.name.includes(needle)) && (!visibility || row.visibility === visibility)
    )
  }, [data, search, visibility])
  useEffect(() => setPage(1), [search, visibility])
  const visible = rows.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE)

  return (
    <OrgAdminFrame
      ui="org.repositories"
      title={t('org.nav.repositories')}
      description={t('org.repos.description', { namespace })}
      actions={
        canCreate ? (
          <button
            type="button"
            data-ui="org.repos.create"
            className={filledButtonClassName(30)}
            onClick={() => setCreating(true)}>
            <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
            {t('org.repos.create')}
          </button>
        ) : null
      }>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <label className="relative w-[240px]">
          <Search
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-[#8a8a8a]"
            aria-hidden
          />
          <Input
            value={search}
            aria-label={t('org.repos.search')}
            placeholder={t('org.repos.search')}
            className={cn(orgFieldClassName, 'h-[30px] pl-8')}
            onChange={(event) => setSearch(event.target.value)}
          />
        </label>
        <div className="w-[140px]">
          <SearchSelect
            size="sm"
            searchable={false}
            aria-label={t('org.repos.col.visibility')}
            placeholder={t('org.repos.col.visibility')}
            value={visibility}
            options={VISIBILITIES.map((value) => ({ value, label: t(`org.repos.visibility.${value}`) }))}
            onChange={(next) =>
              setVisibility((VISIBILITIES as readonly string[]).includes(next) ? (next as Visibility) : '')
            }
          />
        </div>
        <span className="ml-auto text-[12px] text-[#8a8a8a]">{t('org.repos.count', { count: rows.length })}</span>
        <button
          type="button"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          disabled={isValidating}
          className={ghostIconButtonClassName}
          onClick={() => void mutate()}>
          <RotateCw className={cn('size-3.5', isValidating && 'animate-spin')} strokeWidth={1.7} />
        </button>
      </div>
      {error ? (
        <LoadFailed onRetry={() => void mutate()} />
      ) : !data ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.repos" ready={false} />
        </div>
      ) : rows.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 py-16 text-center">
          <FolderGit2 className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
          <p className="m-0 text-[13px] text-[#3f3f3f]">
            {data.length === 0 ? t('org.repos.empty') : t('org.repos.noMatch')}
          </p>
          {data.length === 0 ? (
            <p className="m-0 max-w-sm text-[12px] leading-5 text-[#8a8a8a]">
              {canCreate ? t('org.repos.emptyAdmin') : t('org.repos.emptyHint')}
            </p>
          ) : null}
        </div>
      ) : (
        <>
          <table
            data-ui="org.repos-table"
            aria-label={t('org.nav.repositories')}
            className="w-full min-w-[640px] table-fixed border-collapse text-left">
            <colgroup>
              <col style={{ width: '40%' }} />
              <col style={{ width: '18%' }} />
              <col style={{ width: '20%' }} />
              <col style={{ width: '22%' }} />
            </colgroup>
            <thead className="sticky top-0 z-[1] bg-white">
              <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                {['name', 'visibility', 'defaultBranch', 'created'].map((column, index) => (
                  <th key={column} scope="col" className={cn('h-9 font-normal', index === 0 && 'pl-2.5')}>
                    {t(`org.repos.col.${column}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visible.map((row, index) => (
                <RepositoryRow key={row.name} row={row} index={index} when={when} onOpen={onOpen} />
              ))}
            </tbody>
          </table>
          {rows.length > PAGE_SIZE ? (
            <CollectionPagination page={page} pageSize={PAGE_SIZE} total={rows.length} onPageChange={setPage} />
          ) : null}
        </>
      )}
      <CreateRepositoryDialog
        namespace={namespace}
        open={creating}
        onOpenChange={setCreating}
        onCreated={async (row) => {
          setCreating(false)
          await mutate()
          onOpen(row.name)
        }}
      />
    </OrgAdminFrame>
  )
}

function RepositoryRow({
  row,
  index,
  when,
  onOpen
}: {
  row: Repository
  index: number
  when: (iso: string) => string
  onOpen: (name: string) => void
}) {
  const { t } = useTranslation()
  const VisibilityIcon = row.visibility === 'private' ? Lock : Users
  return (
    <tr
      style={rowMotionStyle(index)}
      className={cn('group cursor-pointer hover:bg-[#f7f7f7]', rowMotionClassName())}
      onClick={() => onOpen(row.name)}>
      <td className="h-12 truncate pl-2.5">
        <button
          type="button"
          className="truncate border-0 bg-transparent p-0 text-left text-[13px] font-medium text-[#1a1a1a] outline-none focus-visible:underline"
          onClick={(event) => {
            event.stopPropagation()
            onOpen(row.name)
          }}>
          {row.name}
        </button>
        {row.archived ? <span className="ml-2 text-[12px] text-[#8a8a8a]">{t('org.repos.archived')}</span> : null}
      </td>
      <td className="h-12 text-[13px] text-[#3f3f3f]">
        <span className="inline-flex items-center gap-1.5">
          <VisibilityIcon className="size-3.5 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
          {t(`org.repos.visibility.${row.visibility}`)}
        </span>
      </td>
      <td className="h-12 truncate font-mono text-[12px] text-[#3f3f3f]">{row.default_branch}</td>
      <td className="h-12 text-[13px] whitespace-nowrap text-[#8a8a8a]" title={row.created_at}>
        {when(row.created_at)}
      </td>
    </tr>
  )
}

function CreateRepositoryDialog({
  namespace,
  open,
  onOpenChange,
  onCreated
}: {
  namespace: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated: (row: Repository) => Promise<void>
}) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [visibility, setVisibility] = useState<Visibility>('team')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!open) return
    setName('')
    setVisibility('team')
    setError(null)
  }, [open])
  const trimmed = name.trim()
  const valid = isRepositoryName(trimmed)

  async function submit() {
    if (busy || !valid) return
    setBusy(true)
    setError(null)
    try {
      const row = await createRepository({ namespace, name: trimmed, visibility })
      await onCreated(row)
    } catch (err) {
      setError(t(repoErrorKey(err)))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-[420px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.repos.create')}</DialogTitle>
        <form
          data-ui="org.repos.create-dialog"
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-repo-name">
            {t('org.repos.col.name')}
            <div className="flex items-center gap-1.5">
              <span className="shrink-0 font-mono text-[13px] text-[#8a8a8a]">{namespace} /</span>
              <Input
                id="org-repo-name"
                aria-label={t('org.repos.col.name')}
                value={name}
                autoFocus
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                maxLength={64}
                disabled={busy}
                placeholder="web"
                className={cn(orgFieldClassName, 'font-mono')}
                onChange={(event) => setName(event.target.value)}
              />
            </div>
            <span className={cn('text-[12px]', trimmed && !valid ? 'text-[#b42318]' : 'text-[#8a8a8a]')}>
              {t('org.repos.nameRule')}
            </span>
          </label>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
            {t('org.repos.col.visibility')}
            <SearchSelect
              searchable={false}
              allowClear={false}
              aria-label={t('org.repos.col.visibility')}
              value={visibility}
              options={VISIBILITIES.map((value) => ({
                value,
                label: `${t(`org.repos.visibility.${value}`)} · ${t(`org.repos.visibilityHint.${value}`)}`
              }))}
              onChange={(next) => {
                if ((VISIBILITIES as readonly string[]).includes(next)) setVisibility(next as Visibility)
              }}
            />
          </label>
          <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.repos.createHint')}</p>
          {error ? (
            <p role="alert" className="m-0 text-[12px] text-[#b42318]">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onOpenChange(false)}
              className={quietButtonClassName(36)}>
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={busy || !valid} className={filledButtonClassName(36)}>
              {t('org.repos.create')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
