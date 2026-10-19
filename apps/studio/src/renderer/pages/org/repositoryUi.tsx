import { ChevronRight, FolderGit2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import type { OrgMemberRow } from './orgApi'
import { quietButtonClassName } from './orgUi'
import type { Changes, ChangeRequestState, FileDiff } from './repositoryApi'

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60]
]

/** 「10 分钟前」「昨天」；一分钟内写「刚刚」。 */
export function relativeTime(iso: string, locale: string, now = Date.now()): string {
  const at = Date.parse(iso)
  if (!Number.isFinite(at)) return iso
  const seconds = Math.round((at - now) / 1000)
  const format = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) return format.format(Math.round(seconds / size), unit)
  }
  return format.format(0, 'minute')
}

export function useRelativeTime() {
  const { i18n } = useTranslation()
  return (iso: string) => relativeTime(iso, i18n.language || 'zh-CN')
}

/** 成员编号换成团队里的名字；已离开团队的人只剩编号前 8 位。 */
export function memberLabel(members: Pick<OrgMemberRow, 'member_id' | 'name'>[], memberId: string) {
  return members.find((row) => row.member_id === memberId)?.name || memberId.slice(0, 8)
}

/** 拒绝和不存在同一答复。 */
export function RepoMissing({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation()
  return (
    <div
      data-ui="org.repos.missing"
      className="flex flex-1 flex-col items-center justify-center gap-3 py-16 text-center">
      <FolderGit2 className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
      <p className="m-0 text-[13px] text-[#3f3f3f]">{t('org.repos.notFound')}</p>
      <button type="button" className={quietButtonClassName(30)} onClick={onBack}>
        {t('org.repos.backToList')}
      </button>
    </div>
  )
}

export function LoadFailed({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation()
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 py-12">
      <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
      <button type="button" onClick={onRetry} className={quietButtonClassName(30)}>
        {t('common.retry')}
      </button>
    </div>
  )
}

const STATE_TONE: Record<ChangeRequestState, string> = {
  open: 'bg-[#e8f5ec] text-[#1f7a3d]',
  merged: 'bg-[#efe9fb] text-[#5b3cc4]',
  closed: 'bg-[#f2f2f2] text-[#6b6b6b]'
}

export function StateBadge({ state }: { state: ChangeRequestState }) {
  const { t } = useTranslation()
  return (
    <span
      className={cn(
        'inline-flex h-[22px] shrink-0 items-center rounded-full px-2 text-[12px] leading-none',
        STATE_TONE[state]
      )}>
      {t(`org.repos.cr.state.${state}`)}
    </span>
  )
}

function FileDiffView({ file }: { file: FileDiff }) {
  const { t } = useTranslation()
  return (
    <section data-ui="org.repos.diff-file" className="overflow-hidden rounded-[10px] border border-[#efefef]">
      <header className="flex items-center justify-between gap-3 border-b border-[#efefef] bg-[#fafafa] px-3 py-2">
        <span className="truncate font-mono text-[12px] text-[#171717]">{file.path}</span>
        <span className="shrink-0 text-[12px] text-[#8a8a8a]">
          {t(`org.repos.diff.status.${file.status}`, file.status)}
        </span>
      </header>
      {file.diff !== 'text' ? (
        <p className="m-0 px-3 py-3 text-[12px] text-[#8a8a8a]">{t(`org.repos.diff.kind.${file.diff}`, file.diff)}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse font-mono text-[12px] leading-5">
            <tbody>
              {file.hunks.map((hunk, index) => (
                <HunkRows key={index} hunk={hunk} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}

function HunkRows({ hunk }: { hunk: FileDiff['hunks'][number] }) {
  return (
    <>
      <tr className="bg-[#f4f7fb] text-[#6b7a90]">
        <td colSpan={3} className="px-3 py-0.5">
          @@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
        </td>
      </tr>
      {hunk.lines.map((line, index) => (
        <tr
          key={index}
          className={cn(line.kind === 'added' && 'bg-[#eefbf1]', line.kind === 'removed' && 'bg-[#fdf0f0]')}>
          <td className="w-10 px-2 text-right text-[#a3a3a3] select-none">{line.old_line ?? ''}</td>
          <td className="w-10 px-2 text-right text-[#a3a3a3] select-none">{line.new_line ?? ''}</td>
          <td className="px-2 whitespace-pre text-[#171717]">
            {line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}
            {line.text}
          </td>
        </tr>
      ))}
    </>
  )
}

export function DiffView({ changes }: { changes: Changes }) {
  const { t } = useTranslation()
  if (changes.files.length === 0) {
    return <p className="m-0 py-6 text-center text-[13px] text-[#8a8a8a]">{t('org.repos.diff.empty')}</p>
  }
  return (
    <div data-ui="org.repos.diff" className="grid gap-3">
      {changes.files.map((file) => (
        <FileDiffView key={file.path} file={file} />
      ))}
      {changes.truncated ? <p className="m-0 text-[12px] text-[#8a8a8a]">{t('org.repos.diff.truncated')}</p> : null}
    </div>
  )
}

/** 仓库和合并请求详情的外框：面包屑、标题行、可滚动正文。 */
export function RepoShell({
  ui,
  crumbs,
  title,
  actions,
  children
}: {
  ui: string
  crumbs: { label: string; onClick?: () => void }[]
  title?: ReactNode
  actions?: ReactNode
  children: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <div data-ui={ui} className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-col gap-2 px-6 pt-4 pb-3">
        <nav aria-label={t('org.repos.breadcrumb')} className="flex min-w-0 items-center gap-1 text-[13px]">
          {crumbs.map((crumb, index) => (
            <span key={index} className="inline-flex min-w-0 items-center gap-1">
              {index > 0 ? <ChevronRight className="size-3.5 shrink-0 text-[#c4c4c4]" aria-hidden /> : null}
              {crumb.onClick ? (
                <button
                  type="button"
                  className="truncate border-0 bg-transparent p-0 text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:underline"
                  onClick={crumb.onClick}>
                  {crumb.label}
                </button>
              ) : (
                <span className="truncate text-[#171717]">{crumb.label}</span>
              )}
            </span>
          ))}
        </nav>
        {title ? (
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="m-0 min-w-0 text-[24px] leading-tight font-bold tracking-[-0.01em] text-foreground">
              {title}
            </h2>
            {actions ? <div className="flex min-w-0 flex-wrap items-center gap-2">{actions}</div> : null}
          </div>
        ) : null}
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-auto px-6 pb-6">{children}</div>
    </div>
  )
}

export const repoTabClassName = (active: boolean) =>
  cn(
    'relative h-9 shrink-0 border-0 bg-transparent px-1 text-[13px] outline-none focus-visible:underline',
    active
      ? 'font-medium text-[#171717] after:absolute after:inset-x-0 after:bottom-0 after:h-[2px] after:bg-[#171717]'
      : 'text-[#8a8a8a] hover:text-[#3f3f3f]'
  )

export function formatBytes(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
}
