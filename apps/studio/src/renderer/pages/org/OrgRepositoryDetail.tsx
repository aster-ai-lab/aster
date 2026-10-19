import { Popover, PopoverContent, PopoverTrigger, SearchSelect } from '@a3s-lab/ui'
import { ChevronDown, Copy, File, Folder, FolderGit2, GitBranch, Link2, Lock, Users } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { PageEntry } from '@renderer/components/layout/PageEntry'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import type { OrgMemberRow } from './orgApi'
import { readOrgOrigin } from './orgApi'
import { OrgChangeRequestList } from './OrgChangeRequestList'
import { OrgProtectionsCard } from './OrgProtectionsCard'
import { filledButtonClassName, quietButtonClassName } from './orgUi'
import {
  cloneUrl,
  getBlob,
  getCommit,
  getRepository,
  getTree,
  isNotFound,
  listBranches,
  listCommits,
  listTags,
  REPO_TABS,
  shortOid,
  type Commit,
  type RepositoryDetail,
  type RepoView
} from './repositoryApi'
import {
  DiffView,
  formatBytes,
  LoadFailed,
  RepoMissing,
  RepoShell,
  repoTabClassName,
  useRelativeTime
} from './repositoryUi'

type RepoViewState = Extract<RepoView, { kind: 'repo' }>

/** 仓库详情：代码、提交、分支与标签、合并请求。分支、路径、标签页写进视图状态，刷新可恢复。 */
export function OrgRepositoryDetail({
  namespace,
  view,
  members,
  onView,
  onList,
  onOpenTokens
}: {
  namespace: string
  view: RepoViewState
  members: OrgMemberRow[]
  onView: (next: RepoView) => void
  onList: () => void
  onOpenTokens: () => void
}) {
  const { t } = useTranslation()
  const { data, error, mutate } = useSWR(`studio/org-repo:${namespace}/${view.name}`, () =>
    getRepository(namespace, view.name)
  )
  const patch = (next: Partial<RepoViewState>) => onView({ ...view, ...next })

  const crumbs = [{ label: t('org.nav.repositories'), onClick: onList }, { label: view.name }]

  if (error) {
    return (
      <RepoShell ui="org.repo" crumbs={crumbs}>
        {isNotFound(error) ? <RepoMissing onBack={onList} /> : <LoadFailed onRetry={() => void mutate()} />}
      </RepoShell>
    )
  }
  if (!data) {
    return (
      <RepoShell ui="org.repo" crumbs={crumbs}>
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.repo" ready={false} />
        </div>
      </RepoShell>
    )
  }
  const ref = view.ref ?? data.default_branch
  return (
    <RepoShell
      ui="org.repo"
      crumbs={crumbs}
      title={
        <span className="inline-flex min-w-0 items-center gap-2">
          <span className="truncate">{data.name}</span>
          {data.visibility === 'private' ? (
            <Lock
              className="size-4 shrink-0 text-[#8a8a8a]"
              strokeWidth={1.7}
              aria-label={t('org.repos.visibility.private')}
            />
          ) : (
            <Users
              className="size-4 shrink-0 text-[#8a8a8a]"
              strokeWidth={1.7}
              aria-label={t('org.repos.visibility.team')}
            />
          )}
        </span>
      }
      actions={<ClonePanel repository={data} onOpenTokens={onOpenTokens} />}>
      <div role="tablist" aria-label={t('org.repos.tabs')} className="mb-4 flex gap-5 border-b border-[#efefef]">
        {REPO_TABS.map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={view.tab === tab}
            data-ui={`org.repo.tab.${tab}`}
            className={repoTabClassName(view.tab === tab)}
            onClick={() => patch({ tab, commit: null })}>
            {t(`org.repos.tab.${tab}`)}
          </button>
        ))}
      </div>
      {view.tab === 'code' ? (
        <CodeTab namespace={namespace} repository={data} gitRef={ref} view={view} onPatch={patch} />
      ) : null}
      {view.tab === 'commits' ? (
        <CommitsTab namespace={namespace} repository={data} gitRef={ref} view={view} onPatch={patch} />
      ) : null}
      {view.tab === 'refs' ? (
        <RefsTab
          namespace={namespace}
          repository={data}
          onBrowse={(next) => patch({ tab: 'code', ref: next, path: '', blob: false })}
        />
      ) : null}
      {view.tab === 'requests' ? (
        <OrgChangeRequestList
          namespace={namespace}
          repository={data}
          members={members}
          onOpen={(number) => onView({ kind: 'request', name: data.name, number })}
        />
      ) : null}
    </RepoShell>
  )
}

function ClonePanel({ repository, onOpenTokens }: { repository: RepositoryDetail; onOpenTokens: () => void }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const { data: origin } = useSWR(open ? 'studio/org-origin' : null, () => readOrgOrigin().catch(() => ''))
  const url = origin ? cloneUrl(origin, repository) : ''

  async function copy() {
    if (!url) return
    try {
      await navigator.clipboard.writeText(url)
      toast.success(t('org.repos.cloneCopied'))
    } catch {
      toast.error(t('org.invite.copyFailed'))
    }
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" data-ui="org.repo.clone" className={filledButtonClassName(30)}>
          {t('org.repos.clone')}
          <ChevronDown className="size-3.5 text-current" strokeWidth={1.7} aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[380px] p-3">
        <div data-ui="org.repo.clone-panel" className="grid gap-2">
          <p className="m-0 text-[12px] text-[#8a8a8a]">HTTPS</p>
          <div className="flex items-center gap-2">
            <input
              readOnly
              value={url}
              aria-label={t('org.repos.cloneUrl')}
              className="h-8 min-w-0 flex-1 rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 font-mono text-[12px] outline-none"
              onFocus={(event) => event.currentTarget.select()}
            />
            <button
              type="button"
              disabled={!url}
              aria-label={t('common.copy')}
              title={t('common.copy')}
              className={quietButtonClassName(30)}
              onClick={() => void copy()}>
              <Copy className="size-3.5 text-current" strokeWidth={1.7} />
            </button>
          </div>
          <p className="m-0 text-[12px] leading-5 text-[#3f3f3f]">{t('org.repos.cloneHint')}</p>
          <button
            type="button"
            className="inline-flex w-fit items-center gap-1 border-0 bg-transparent p-0 text-[12px] text-[#2563eb] outline-none hover:underline focus-visible:underline"
            onClick={() => {
              setOpen(false)
              onOpenTokens()
            }}>
            <Link2 className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('org.repos.goTokens')}
          </button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

function BranchPicker({
  namespace,
  repository,
  value,
  onChange
}: {
  namespace: string
  repository: RepositoryDetail
  value: string
  onChange: (next: string) => void
}) {
  const { t } = useTranslation()
  const { data } = useSWR(`studio/org-repo-branches:${namespace}/${repository.name}`, () =>
    listBranches(namespace, repository.name)
  )
  const options = (data ?? []).map((branch) => ({ value: branch.name, label: branch.name }))
  if (!options.some((option) => option.value === value)) options.unshift({ value, label: value })
  return (
    <div className="w-[200px]">
      <SearchSelect
        size="sm"
        allowClear={false}
        aria-label={t('org.repos.branch')}
        value={value}
        options={options}
        onChange={(next) => {
          if (next) onChange(next)
        }}
      />
    </div>
  )
}

function PathCrumbs({ name, path, onPath }: { name: string; path: string; onPath: (path: string) => void }) {
  const parts = path ? path.split('/') : []
  return (
    <nav className="flex min-w-0 flex-wrap items-center gap-1 font-mono text-[12px] text-[#3f3f3f]">
      <button
        type="button"
        className="border-0 bg-transparent p-0 text-[#2563eb] hover:underline"
        onClick={() => onPath('')}>
        {name}
      </button>
      {parts.map((part, index) => {
        const target = parts.slice(0, index + 1).join('/')
        const last = index === parts.length - 1
        return (
          <span key={target} className="inline-flex items-center gap-1">
            <span className="text-[#c4c4c4]">/</span>
            {last ? (
              <span className="text-[#171717]">{part}</span>
            ) : (
              <button
                type="button"
                className="border-0 bg-transparent p-0 text-[#2563eb] hover:underline"
                onClick={() => onPath(target)}>
                {part}
              </button>
            )}
          </span>
        )
      })}
    </nav>
  )
}

function CodeTab({
  namespace,
  repository,
  gitRef,
  view,
  onPatch
}: {
  namespace: string
  repository: RepositoryDetail
  gitRef: string
  view: RepoViewState
  onPatch: (next: Partial<RepoViewState>) => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const { data: latest } = useSWR(`studio/org-repo-latest:${namespace}/${repository.name}@${gitRef}`, () =>
    listCommits(namespace, repository.name, gitRef).then((page) => page.commits[0] ?? null)
  )
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <BranchPicker
          namespace={namespace}
          repository={repository}
          value={gitRef}
          onChange={(next) => onPatch({ ref: next, path: '', blob: false })}
        />
        <PathCrumbs name={repository.name} path={view.path} onPath={(path) => onPatch({ path, blob: false })} />
        {latest ? (
          <span className="ml-auto truncate text-[12px] text-[#8a8a8a]">
            {t('org.repos.latestCommit')} <span className="font-mono">{shortOid(latest.oid)}</span> ·{' '}
            {when(latest.committer.at)}
          </span>
        ) : null}
      </div>
      {view.blob ? (
        <BlobPanel namespace={namespace} repository={repository} gitRef={gitRef} path={view.path} />
      ) : (
        <TreePanel
          namespace={namespace}
          repository={repository}
          gitRef={gitRef}
          path={view.path}
          onOpen={(path, blob) => onPatch({ path, blob })}
        />
      )}
    </div>
  )
}

function TreePanel({
  namespace,
  repository,
  gitRef,
  path,
  onOpen
}: {
  namespace: string
  repository: RepositoryDetail
  gitRef: string
  path: string
  onOpen: (path: string, blob: boolean) => void
}) {
  const { t } = useTranslation()
  const { data, error, mutate } = useSWR(`studio/org-repo-tree:${namespace}/${repository.name}@${gitRef}:${path}`, () =>
    getTree(namespace, repository.name, gitRef, path)
  )
  const readme = data?.entries.find((entry) => entry.type === 'blob' && /^readme(\.md|\.txt)?$/i.test(entry.name))
  if (error) {
    return isNotFound(error) ? (
      <p className="m-0 py-8 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.pathMissing')}</p>
    ) : (
      <LoadFailed onRetry={() => void mutate()} />
    )
  }
  if (!data) return <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
  const entries = [...data.entries].sort((a, b) =>
    a.type === b.type ? a.name.localeCompare(b.name) : a.type === 'tree' ? -1 : b.type === 'tree' ? 1 : 0
  )
  return (
    <>
      <div data-ui="org.repo.tree" className="overflow-hidden rounded-[10px] border border-[#efefef]">
        {entries.length === 0 ? (
          <p className="m-0 px-3 py-6 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.emptyTree')}</p>
        ) : (
          entries.map((entry) => {
            const isTree = entry.type === 'tree'
            const Icon = isTree ? Folder : entry.type === 'submodule' ? FolderGit2 : File
            return (
              <button
                key={entry.path}
                type="button"
                disabled={entry.type === 'submodule'}
                className="flex h-9 w-full items-center gap-2 border-0 border-b border-[#f3f3f3] bg-transparent px-3 text-left text-[13px] text-[#171717] last:border-b-0 hover:bg-[#f7f7f7] disabled:text-[#8a8a8a]"
                onClick={() => onOpen(entry.path, !isTree)}>
                <Icon className="size-4 shrink-0 text-[#8a8a8a]" strokeWidth={1.6} aria-hidden />
                <span className="truncate">{entry.name}</span>
              </button>
            )
          })
        )}
      </div>
      {readme ? (
        <BlobPanel namespace={namespace} repository={repository} gitRef={gitRef} path={readme.path} titled />
      ) : null}
    </>
  )
}

function BlobPanel({
  namespace,
  repository,
  gitRef,
  path,
  titled = false
}: {
  namespace: string
  repository: RepositoryDetail
  gitRef: string
  path: string
  titled?: boolean
}) {
  const { t } = useTranslation()
  const { data, error, mutate } = useSWR(`studio/org-repo-blob:${namespace}/${repository.name}@${gitRef}:${path}`, () =>
    getBlob(namespace, repository.name, gitRef, path)
  )
  if (error) {
    return isNotFound(error) ? (
      <p className="m-0 py-8 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.pathMissing')}</p>
    ) : (
      <LoadFailed onRetry={() => void mutate()} />
    )
  }
  if (!data) return <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
  return (
    <section data-ui="org.repo.blob" className="overflow-hidden rounded-[10px] border border-[#efefef]">
      <header className="flex items-center justify-between gap-3 border-b border-[#efefef] bg-[#fafafa] px-3 py-2">
        <span className="truncate font-mono text-[12px] text-[#171717]">
          {titled ? data.path.split('/').pop() : data.path}
        </span>
        <span className="shrink-0 text-[12px] text-[#8a8a8a]">{formatBytes(data.size)}</span>
      </header>
      {data.content !== null ? (
        <pre className="m-0 max-h-[560px] overflow-auto px-3 py-2 font-mono text-[12px] leading-5 whitespace-pre text-[#171717]">
          {data.content}
        </pre>
      ) : (
        <p className="m-0 px-3 py-6 text-center text-[13px] text-[#8a8a8a]">
          {t(`org.repos.omitted.${data.omitted ?? 'binary'}`, { size: formatBytes(data.size) })}
        </p>
      )}
    </section>
  )
}

function CommitsTab({
  namespace,
  repository,
  gitRef,
  view,
  onPatch
}: {
  namespace: string
  repository: RepositoryDetail
  gitRef: string
  view: RepoViewState
  onPatch: (next: Partial<RepoViewState>) => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const [pages, setPages] = useState<{ commits: Commit[]; next: string | null }[]>([])
  const [error, setError] = useState(false)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    let live = true
    setPages([])
    setError(false)
    setBusy(true)
    listCommits(namespace, repository.name, gitRef)
      .then((page) => live && setPages([page]))
      .catch(() => live && setError(true))
      .finally(() => live && setBusy(false))
    return () => {
      live = false
    }
  }, [namespace, repository.name, gitRef])

  if (view.commit) {
    return (
      <CommitPanel
        namespace={namespace}
        repository={repository}
        oid={view.commit}
        onBack={() => onPatch({ commit: null })}
      />
    )
  }
  const commits = pages.flatMap((page) => page.commits)
  const next = pages.at(-1)?.next ?? null

  async function more() {
    if (!next || busy) return
    setBusy(true)
    try {
      const page = await listCommits(namespace, repository.name, gitRef, next)
      setPages((current) => [...current, page])
    } catch {
      toast.error(t('org.error.generic'))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="grid gap-3">
      <BranchPicker
        namespace={namespace}
        repository={repository}
        value={gitRef}
        onChange={(next) => onPatch({ ref: next, commit: null })}
      />
      {error ? (
        <LoadFailed onRetry={() => onPatch({ ref: gitRef })} />
      ) : commits.length === 0 && busy ? (
        <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
      ) : (
        <div data-ui="org.repo.commits" className="overflow-hidden rounded-[10px] border border-[#efefef]">
          {commits.map((commit) => (
            <button
              key={commit.oid}
              type="button"
              className="flex w-full items-center gap-3 border-0 border-b border-[#f3f3f3] bg-transparent px-3 py-2.5 text-left last:border-b-0 hover:bg-[#f7f7f7]"
              onClick={() => onPatch({ commit: commit.oid })}>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] text-[#171717]">{commit.subject}</span>
                <span className="block truncate text-[12px] text-[#8a8a8a]">
                  {commit.author.name} · {when(commit.author.at)}
                </span>
              </span>
              <span className="shrink-0 font-mono text-[12px] text-[#3f3f3f]">{shortOid(commit.oid)}</span>
            </button>
          ))}
        </div>
      )}
      {next ? (
        <button
          type="button"
          disabled={busy}
          className={cn(quietButtonClassName(30), 'w-fit')}
          onClick={() => void more()}>
          {t('org.repos.moreCommits')}
        </button>
      ) : null}
    </div>
  )
}

function CommitPanel({
  namespace,
  repository,
  oid,
  onBack
}: {
  namespace: string
  repository: RepositoryDetail
  oid: string
  onBack: () => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const { data, error, mutate } = useSWR(`studio/org-repo-commit:${namespace}/${repository.name}:${oid}`, () =>
    getCommit(namespace, repository.name, oid)
  )
  return (
    <div data-ui="org.repo.commit" className="grid gap-3">
      <button type="button" className={cn(quietButtonClassName(30), 'w-fit')} onClick={onBack}>
        {t('org.repos.backToCommits')}
      </button>
      {error ? (
        isNotFound(error) ? (
          <p className="m-0 py-8 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.commitMissing')}</p>
        ) : (
          <LoadFailed onRetry={() => void mutate()} />
        )
      ) : !data ? (
        <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
      ) : (
        <>
          <div className="rounded-[10px] border border-[#efefef] px-4 py-3">
            <p className="m-0 text-[14px] font-medium text-[#171717]">{data.commit.subject}</p>
            {data.commit.message.trim() !== data.commit.subject.trim() ? (
              <pre className="m-0 mt-2 font-sans text-[13px] leading-5 whitespace-pre-wrap text-[#3f3f3f]">
                {data.commit.message.trim().slice(data.commit.subject.trim().length).trim()}
              </pre>
            ) : null}
            <p className="m-0 mt-2 text-[12px] text-[#8a8a8a]">
              {data.commit.author.name} · {when(data.commit.author.at)} ·{' '}
              <span className="font-mono">{data.commit.oid}</span>
            </p>
          </div>
          <DiffView changes={data.changes} />
        </>
      )}
    </div>
  )
}

function RefsTab({
  namespace,
  repository,
  onBrowse
}: {
  namespace: string
  repository: RepositoryDetail
  onBrowse: (ref: string) => void
}) {
  const { t } = useTranslation()
  const branches = useSWR(`studio/org-repo-branches:${namespace}/${repository.name}`, () =>
    listBranches(namespace, repository.name)
  )
  const tags = useSWR(`studio/org-repo-tags:${namespace}/${repository.name}`, () =>
    listTags(namespace, repository.name)
  )
  return (
    <div className="grid gap-5">
      <section>
        <h3 className="m-0 mb-2 text-[14px] font-medium text-[#171717]">{t('org.repos.branches')}</h3>
        {branches.error ? (
          <LoadFailed onRetry={() => void branches.mutate()} />
        ) : (
          <div data-ui="org.repo.branches" className="overflow-hidden rounded-[10px] border border-[#efefef]">
            {(branches.data ?? []).map((branch) => (
              <div
                key={branch.ref}
                className="flex h-10 items-center gap-2 border-b border-[#f3f3f3] px-3 text-[13px] last:border-b-0">
                <GitBranch className="size-3.5 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
                <button
                  type="button"
                  className="truncate border-0 bg-transparent p-0 font-mono text-[12px] text-[#171717] hover:underline"
                  onClick={() => onBrowse(branch.name)}>
                  {branch.name}
                </button>
                {branch.default ? (
                  <span className="rounded-full bg-[#f2f2f2] px-2 py-0.5 text-[11px] text-[#6b6b6b]">
                    {t('org.repos.defaultBranch')}
                  </span>
                ) : null}
                <span className="ml-auto font-mono text-[12px] text-[#8a8a8a]">{shortOid(branch.oid)}</span>
              </div>
            ))}
            {branches.data?.length === 0 ? (
              <p className="m-0 px-3 py-4 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.noBranches')}</p>
            ) : null}
          </div>
        )}
      </section>
      <section>
        <h3 className="m-0 mb-2 text-[14px] font-medium text-[#171717]">{t('org.repos.tags')}</h3>
        <div data-ui="org.repo.tags" className="overflow-hidden rounded-[10px] border border-[#efefef]">
          {(tags.data ?? []).map((tag) => (
            <div key={tag.ref} className="flex h-10 items-center gap-2 border-b border-[#f3f3f3] px-3 last:border-b-0">
              <span className="truncate font-mono text-[12px] text-[#171717]">{tag.name}</span>
              <span className="ml-auto font-mono text-[12px] text-[#8a8a8a]">{shortOid(tag.commit ?? tag.oid)}</span>
            </div>
          ))}
          {tags.data?.length === 0 ? (
            <p className="m-0 px-3 py-4 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.noTags')}</p>
          ) : null}
        </div>
      </section>
      <OrgProtectionsCard namespace={namespace} repository={repository} />
    </div>
  )
}
