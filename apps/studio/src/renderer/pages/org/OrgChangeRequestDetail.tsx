import { Textarea } from '@a3s-lab/ui'
import { Check, CircleDashed, GitMerge, X } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR, { useSWRConfig } from 'swr'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { OrgCard } from './OrgAdminFrame'
import type { OrgMemberRow } from './orgApi'
import { filledButtonClassName, quietButtonClassName } from './orgUi'
import {
  actOnChangeRequest,
  commentOnChangeRequest,
  compareRefs,
  getChangeRequest,
  getCommit,
  getRepository,
  isNotFound,
  refusalMessage,
  repoErrorKey,
  reviewChangeRequest,
  shortOid,
  type ChangeRequestAction,
  type ChangeRequestDetail,
  type RepositoryDetail,
  type RepoTab,
  type ReviewVerdict
} from './repositoryApi'
import {
  DiffView,
  LoadFailed,
  memberLabel,
  RepoMissing,
  RepoShell,
  repoTabClassName,
  StateBadge,
  useRelativeTime
} from './repositoryUi'

const TABS = ['conversation', 'changes', 'commits'] as const
type Tab = (typeof TABS)[number]

/** 合并请求详情：对话、变更、提交，右侧是合并条件和生命周期操作。 */
export function OrgChangeRequestDetail({
  namespace,
  name,
  number,
  members,
  onBack,
  onList
}: {
  namespace: string
  name: string
  number: number
  members: OrgMemberRow[]
  onBack: (tab: RepoTab) => void
  onList: () => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const [tab, setTab] = useState<Tab>('conversation')
  const repo = useSWR(`studio/org-repo:${namespace}/${name}`, () => getRepository(namespace, name))
  const request = useSWR(`studio/org-repo-cr:${namespace}/${name}#${number}`, () =>
    getChangeRequest(namespace, name, number)
  )
  const crumbs = [
    { label: t('org.nav.repositories'), onClick: onList },
    { label: name, onClick: () => onBack('requests') },
    { label: `#${number}` }
  ]
  const error = repo.error ?? request.error
  if (error) {
    return (
      <RepoShell ui="org.cr" crumbs={crumbs}>
        {isNotFound(error) ? (
          <RepoMissing onBack={() => (repo.error ? onList() : onBack('requests'))} />
        ) : (
          <LoadFailed
            onRetry={() => {
              void repo.mutate()
              void request.mutate()
            }}
          />
        )}
      </RepoShell>
    )
  }
  if (!repo.data || !request.data) {
    return (
      <RepoShell ui="org.cr" crumbs={crumbs}>
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.cr" ready={false} />
        </div>
      </RepoShell>
    )
  }
  const detail = request.data
  return (
    <RepoShell
      ui="org.cr"
      crumbs={crumbs}
      title={
        <span className="inline-flex min-w-0 items-center gap-2">
          <span className="truncate">{detail.title}</span>
          <span className="shrink-0 text-[#8a8a8a]">#{detail.number}</span>
        </span>
      }>
      <div className="mb-3 flex flex-wrap items-center gap-2 text-[13px] text-[#3f3f3f]">
        <StateBadge state={detail.state} />
        <span>
          {t('org.repos.cr.summary', {
            author: memberLabel(members, detail.author_member),
            source: detail.source,
            target: detail.target,
            when: when(detail.created_at)
          })}
        </span>
      </div>
      <div className="grid min-h-0 gap-5 lg:grid-cols-[minmax(0,1fr)_300px]">
        <div className="min-w-0">
          <div role="tablist" aria-label={t('org.repos.cr.tabs')} className="mb-4 flex gap-5 border-b border-[#efefef]">
            {TABS.map((item) => (
              <button
                key={item}
                type="button"
                role="tab"
                aria-selected={tab === item}
                data-ui={`org.cr.tab.${item}`}
                className={repoTabClassName(tab === item)}
                onClick={() => setTab(item)}>
                {t(`org.repos.cr.tab.${item}`)}
              </button>
            ))}
          </div>
          {tab === 'conversation' ? (
            <Conversation
              namespace={namespace}
              repository={repo.data}
              detail={detail}
              members={members}
              onChanged={() => void request.mutate()}
            />
          ) : (
            <ChangesPanel namespace={namespace} name={name} detail={detail} commits={tab === 'commits'} />
          )}
        </div>
        <MergePanel
          namespace={namespace}
          repository={repo.data}
          detail={detail}
          onChanged={() => void request.mutate()}
        />
      </div>
    </RepoShell>
  )
}

type Entry =
  | { kind: 'comment'; at: string; who: string; body: string; key: string }
  | { kind: 'review'; at: string; who: string; verdict: ReviewVerdict; oid: string; key: string }

function Conversation({
  namespace,
  repository,
  detail,
  members,
  onChanged
}: {
  namespace: string
  repository: RepositoryDetail
  detail: ChangeRequestDetail
  members: OrgMemberRow[]
  onChanged: () => void
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const profile = useMemberProfile()
  const [draft, setDraft] = useState('')
  const [busy, setBusy] = useState<null | 'comment' | ReviewVerdict>(null)
  const isAuthor = Boolean(profile?.id) && profile?.id === detail.author_member
  const canReview = repository.permissions.review && !isAuthor && detail.state === 'open' && !repository.archived
  const entries: Entry[] = [
    ...detail.comments.map((comment) => ({
      kind: 'comment' as const,
      at: comment.created_at,
      who: comment.author_member,
      body: comment.path ? `${comment.path}${comment.line ? `:${comment.line}` : ''}\n${comment.body}` : comment.body,
      key: `c${comment.id}`
    })),
    ...detail.reviews.map((review) => ({
      kind: 'review' as const,
      at: review.created_at,
      who: review.reviewer_member,
      verdict: review.verdict,
      oid: review.reviewed_oid,
      key: `r${review.id}`
    }))
  ].sort((a, b) => a.at.localeCompare(b.at))

  async function comment() {
    if (!draft.trim() || busy) return
    setBusy('comment')
    try {
      await commentOnChangeRequest(namespace, repository.name, detail.number, draft.trim())
      setDraft('')
      onChanged()
    } catch (err) {
      toast.error(t(repoErrorKey(err)))
    } finally {
      setBusy(null)
    }
  }

  async function review(verdict: ReviewVerdict) {
    if (busy) return
    setBusy(verdict)
    try {
      await reviewChangeRequest(namespace, repository.name, detail.number, verdict, draft)
      setDraft('')
      toast.success(t(`org.repos.cr.reviewed.${verdict}`))
      onChanged()
    } catch (err) {
      toast.error(t(repoErrorKey(err)))
      onChanged()
    } finally {
      setBusy(null)
    }
  }

  return (
    <div data-ui="org.cr.conversation" className="grid gap-3">
      <article className="rounded-[10px] border border-[#efefef] px-4 py-3">
        {detail.body.trim() ? (
          <p className="m-0 text-[13px] leading-6 whitespace-pre-wrap text-[#171717]">{detail.body}</p>
        ) : (
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.repos.cr.noBody')}</p>
        )}
      </article>
      {entries.map((entry) =>
        entry.kind === 'comment' ? (
          <article key={entry.key} className="rounded-[10px] border border-[#efefef] px-4 py-3">
            <p className="m-0 mb-1 text-[12px] text-[#8a8a8a]">
              <span className="text-[#171717]">{memberLabel(members, entry.who)}</span> · {when(entry.at)}
            </p>
            <p className="m-0 text-[13px] leading-6 whitespace-pre-wrap text-[#171717]">{entry.body}</p>
          </article>
        ) : (
          <p key={entry.key} className="m-0 flex items-center gap-2 px-1 text-[12px] text-[#3f3f3f]">
            {entry.verdict === 'approved' ? (
              <Check className="size-3.5 text-[#1f7a3d]" strokeWidth={2} aria-hidden />
            ) : (
              <X className="size-3.5 text-[#c4321b]" strokeWidth={2} aria-hidden />
            )}
            {t(`org.repos.cr.reviewLine.${entry.verdict}`, {
              who: memberLabel(members, entry.who),
              oid: shortOid(entry.oid)
            })}
            <span className="text-[#8a8a8a]">· {when(entry.at)}</span>
          </p>
        )
      )}
      {repository.archived ? null : (
        <div className="grid gap-2 rounded-[10px] border border-[#efefef] p-3">
          <Textarea.Input
            aria-label={t('org.repos.cr.commentLabel')}
            placeholder={t('org.repos.cr.commentPlaceholder')}
            value={draft}
            rows={3}
            maxLength={8000}
            onChange={(event) => setDraft(event.target.value)}
          />
          <div className="flex flex-wrap justify-end gap-2">
            {canReview ? (
              <>
                <button
                  type="button"
                  data-ui="org.cr.request-changes"
                  disabled={busy !== null}
                  className={quietButtonClassName(30)}
                  onClick={() => void review('changes_requested')}>
                  {t('org.repos.cr.requestChanges')}
                </button>
                <button
                  type="button"
                  data-ui="org.cr.approve"
                  disabled={busy !== null}
                  className={quietButtonClassName(30)}
                  onClick={() => void review('approved')}>
                  <Check className="size-3.5" strokeWidth={1.8} aria-hidden />
                  {t('org.repos.cr.approve')}
                </button>
              </>
            ) : null}
            <button
              type="button"
              data-ui="org.cr.comment"
              disabled={busy !== null || !draft.trim()}
              className={filledButtonClassName(30)}
              onClick={() => void comment()}>
              {t('org.repos.cr.comment')}
            </button>
          </div>
          {isAuthor && repository.permissions.review && detail.state === 'open' ? (
            <p className="m-0 text-[12px] text-[#8a8a8a]">{t('org.repos.cr.authorCannotApprove')}</p>
          ) : null}
        </div>
      )}
    </div>
  )
}

function ChangesPanel({
  namespace,
  name,
  detail,
  commits
}: {
  namespace: string
  name: string
  detail: ChangeRequestDetail
  commits: boolean
}) {
  const { t } = useTranslation()
  const when = useRelativeTime()
  const merged = detail.state === 'merged' && detail.merge
  const compare = useSWR(
    merged ? null : `studio/org-repo-compare:${namespace}/${name}:${detail.target_oid}...${detail.source_oid}`,
    () => compareRefs(namespace, name, detail.target, detail.source)
  )
  const mergeCommit = useSWR(
    merged ? `studio/org-repo-commit:${namespace}/${name}:${detail.merge?.merged_oid}` : null,
    () => getCommit(namespace, name, detail.merge!.merged_oid)
  )
  const failed = compare.error ?? mergeCommit.error
  if (failed) {
    return (
      <p className="m-0 py-8 text-center text-[13px] text-[#8a8a8a]">
        {t(isNotFound(failed) ? 'org.repos.cr.sourceGone' : 'org.error.generic')}
      </p>
    )
  }
  if (merged) {
    if (!mergeCommit.data) return <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
    if (commits) {
      return (
        <p className="m-0 py-6 text-[13px] text-[#3f3f3f]">
          {t('org.repos.cr.mergedAs', { oid: shortOid(mergeCommit.data.commit.oid) })}
        </p>
      )
    }
    return <DiffView changes={mergeCommit.data.changes} />
  }
  if (!compare.data) return <div className="h-40 animate-pulse rounded-[10px] bg-[#fafafa]" />
  if (!commits) return <DiffView changes={compare.data.changes} />
  return (
    <div data-ui="org.cr.commits" className="overflow-hidden rounded-[10px] border border-[#efefef]">
      {compare.data.commits.length === 0 ? (
        <p className="m-0 px-3 py-6 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.cr.noCommits')}</p>
      ) : (
        compare.data.commits.map((commit) => (
          <div
            key={commit.oid}
            className="flex items-center gap-3 border-b border-[#f3f3f3] px-3 py-2.5 last:border-b-0">
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[13px] text-[#171717]">{commit.subject}</span>
              <span className="block truncate text-[12px] text-[#8a8a8a]">
                {commit.author.name} · {when(commit.author.at)}
              </span>
            </span>
            <span className="shrink-0 font-mono text-[12px] text-[#3f3f3f]">{shortOid(commit.oid)}</span>
          </div>
        ))
      )}
      {compare.data.commits_truncated ? (
        <p className="m-0 px-3 py-2 text-[12px] text-[#8a8a8a]">{t('org.repos.cr.commitsTruncated')}</p>
      ) : null}
    </div>
  )
}

function MergePanel({
  namespace,
  repository,
  detail,
  onChanged
}: {
  namespace: string
  repository: RepositoryDetail
  detail: ChangeRequestDetail
  onChanged: () => void
}) {
  const { t } = useTranslation()
  const { mutate } = useSWRConfig()
  const [busy, setBusy] = useState<ChangeRequestAction | null>(null)
  const gate = detail.gate
  const canWrite = repository.permissions.write && !repository.archived
  const open = detail.state === 'open'
  const message = refusalMessage(gate)
  const refusal = message === null ? null : 'raw' in message ? message.raw : t(message.key, message.values)

  async function act(action: ChangeRequestAction) {
    if (busy) return
    setBusy(action)
    try {
      await actOnChangeRequest(namespace, repository.name, detail.number, action)
      toast.success(t(`org.repos.cr.done.${action}`))
    } catch (err) {
      toast.error(t(repoErrorKey(err)))
    } finally {
      setBusy(null)
      onChanged()
      void mutate(
        (key) => typeof key === 'string' && key.startsWith(`studio/org-repo-crs:${namespace}/${repository.name}:`)
      )
      if (action === 'merge') {
        void mutate(
          (key) => typeof key === 'string' && key.startsWith(`studio/org-repo-branches:${namespace}/${repository.name}`)
        )
      }
    }
  }

  return (
    <aside data-ui="org.cr.gate" className="grid content-start gap-3">
      <OrgCard title={t('org.repos.cr.gate')}>
        {open ? (
          <ul className="m-0 grid list-none gap-2 p-0 text-[13px]">
            <GateLine
              ok={gate.approvals.length >= gate.required_approvals}
              label={t('org.repos.cr.approvals', { count: gate.approvals.length, required: gate.required_approvals })}
            />
            {gate.required_checks.length === 0 ? (
              <GateLine ok label={t('org.repos.cr.noRequiredChecks')} />
            ) : (
              gate.required_checks.map((check) => (
                <GateLine
                  key={check}
                  ok={gate.passed_checks.includes(check)}
                  label={t('org.repos.cr.checkLine', { check })}
                />
              ))
            )}
          </ul>
        ) : detail.state === 'merged' && detail.merge ? (
          <p className="m-0 text-[13px] text-[#3f3f3f]">
            {t('org.repos.cr.mergedAs', { oid: shortOid(detail.merge.merged_oid) })}
          </p>
        ) : (
          <p className="m-0 text-[13px] text-[#3f3f3f]">{t('org.repos.cr.closedNote')}</p>
        )}
        {open && refusal ? (
          <p data-ui="org.cr.refusal" className="m-0 mt-3 text-[12px] leading-5 text-[#b45309]">
            {refusal}
          </p>
        ) : null}
        {canWrite ? (
          <div className="mt-4 grid gap-2">
            {open ? (
              <>
                <button
                  type="button"
                  data-ui="org.cr.merge"
                  disabled={busy !== null || gate.refusal !== null}
                  className={cn(filledButtonClassName(36), 'w-full')}
                  onClick={() => void act('merge')}>
                  <GitMerge className="size-4 text-current" strokeWidth={1.7} aria-hidden />
                  {t('org.repos.cr.merge')}
                </button>
                <button
                  type="button"
                  data-ui="org.cr.close"
                  disabled={busy !== null}
                  className={cn(quietButtonClassName(36), 'w-full')}
                  onClick={() => void act('close')}>
                  {t('org.repos.cr.close')}
                </button>
              </>
            ) : detail.state === 'closed' ? (
              <button
                type="button"
                data-ui="org.cr.reopen"
                disabled={busy !== null}
                className={cn(quietButtonClassName(36), 'w-full')}
                onClick={() => void act('reopen')}>
                {t('org.repos.cr.reopen')}
              </button>
            ) : null}
          </div>
        ) : null}
      </OrgCard>
      {detail.checks.length > 0 ? (
        <OrgCard title={t('org.repos.cr.checks')}>
          <ul className="m-0 grid list-none gap-1.5 p-0 text-[12px]">
            {detail.checks.map((check) => (
              <li key={`${check.name}:${check.job_id}`} className="flex items-center justify-between gap-2">
                <span className="truncate font-mono text-[#171717]">{check.name}</span>
                <span className="shrink-0 text-[#8a8a8a]">
                  {t(`org.repos.cr.checkState.${check.state}`, check.state)}
                </span>
              </li>
            ))}
          </ul>
        </OrgCard>
      ) : null}
    </aside>
  )
}

function GateLine({ ok, label }: { ok: boolean; label: string }) {
  return (
    <li className="flex items-center gap-2">
      {ok ? (
        <Check className="size-4 shrink-0 text-[#1f7a3d]" strokeWidth={2} aria-hidden />
      ) : (
        <CircleDashed className="size-4 shrink-0 text-[#b45309]" strokeWidth={1.8} aria-hidden />
      )}
      <span className={cn(ok ? 'text-[#171717]' : 'text-[#3f3f3f]')}>{label}</span>
    </li>
  )
}
