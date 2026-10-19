import { PopConfirm } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { readCodeSession, writeCodePullRequest } from './codeMode'
import { DiffStatLabel } from './DiffStatLabel'
import { createCodePullRequest } from './prepareCodeWorkspace'
import type { TaskCodeDiff, TaskDiffOperation } from './taskConversation/taskDiff'
import { TaskDiffView } from './taskConversation/TaskDiffView'

const STATUS_MARK: Record<TaskDiffOperation, string> = {
  write: 'A',
  edit: 'M',
  patch: 'M',
  delete: 'D'
}

function failureText(error: unknown, fallback: string): string {
  if (typeof error === 'string' && error.trim()) return error.trim()
  if (error instanceof Error && error.message.trim()) return error.message.trim()
  return fallback
}

function fileParts(path: string | undefined): { name: string; directory: string } {
  const normalized = path?.trim().replace(/\\/g, '/') ?? ''
  const slash = normalized.lastIndexOf('/')
  if (slash < 0) return { name: normalized, directory: '' }
  return { name: normalized.slice(slash + 1), directory: normalized.slice(0, slash) }
}

/** Files changed by tools in the latest turn, plus a GitHub pull request for that work. */
export function CodeChangesPanel({
  sessionId,
  diffs,
  onOpenFile
}: {
  sessionId?: string
  diffs: readonly TaskCodeDiff[]
  onOpenFile?: (path: string) => void
}) {
  const { t } = useTranslation()
  const record = sessionId ? readCodeSession(sessionId) : null
  const [url, setUrl] = useState(record?.pullRequestUrl ?? '')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [pickedPath, setPickedPath] = useState<string | null>(null)
  const listed = diffs.map((diff) => diff.path ?? '').join('\0')
  useEffect(() => {
    const paths = listed ? listed.split('\0') : []
    setPickedPath((current) => {
      const next = paths.length === 0 ? null : current && paths.includes(current) ? current : (paths[0] ?? null)
      return next === current ? current : next
    })
  }, [listed])
  const selected = diffs.find((diff) => (diff.path ?? '') === pickedPath) ?? diffs[0]
  const additions = diffs.reduce((sum, diff) => sum + diff.additions, 0)
  const deletions = diffs.reduce((sum, diff) => sum + diff.deletions, 0)

  useEffect(() => {
    setUrl(sessionId ? (readCodeSession(sessionId)?.pullRequestUrl ?? '') : '')
    setError('')
  }, [sessionId])

  async function openPullRequest() {
    if (!sessionId || !record?.cwd || busy) return
    setBusy(true)
    setError('')
    const failed = t('workbench.code.pull_request_failed', '无法创建拉取请求')
    try {
      const next = await createCodePullRequest({
        cwd: record.cwd,
        branch: record.branch,
        title: record.topic.trim() || record.branch || t('workbench.code.changes', '变更'),
        body: diffs
          .map((diff) => diff.path)
          .filter((path): path is string => Boolean(path))
          .join('\n')
      })
      writeCodePullRequest(sessionId, next)
      setUrl(next)
    } catch (caught) {
      const message = failureText(caught, failed)
      setError(message === 'gh-missing' ? failed : message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div data-ui="workbench.code.changes" className="flex h-full min-h-0 flex-col bg-white">
      <header className="flex h-9 shrink-0 items-center gap-2 border-b border-[#efefef] px-3">
        <span className="text-[13px] font-medium text-[#171717]">{t('workbench.code.changes', '变更')}</span>
        <span className="text-[12px] text-[#8a8a8a] tabular-nums">{diffs.length}</span>
        <DiffStatLabel additions={additions} deletions={deletions} />
        {record?.branch ? (
          <span className="max-w-[40%] min-w-0 truncate font-mono text-[12px] text-[#8a8a8a]">{record.branch}</span>
        ) : null}
        {record?.cwd ? (
          <span
            title={record.cwd}
            aria-label={record.cwd}
            className="min-w-0 flex-1 truncate font-mono text-[12px] text-[#8a8a8a]">
            {record.cwd}
          </span>
        ) : null}
        <span className="ml-auto shrink-0">
          {url ? (
            <a href={url} target="_blank" rel="noreferrer" className="text-[13px] text-[#171717] underline">
              {t('workbench.code.pull_request_ready', '拉取请求')}
            </a>
          ) : (
            <PopConfirm
              tone="default"
              title={t('workbench.code.pull_request_confirm', '创建拉取请求')}
              description={t(
                'workbench.code.pull_request_confirm_detail',
                '将当前分支推送到 origin，并在 GitHub 上创建拉取请求'
              )}
              confirmLabel={t('common.confirm', '确认')}
              cancelLabel={t('common.cancel', '取消')}
              disabled={!record?.cwd || busy}
              onConfirm={openPullRequest}>
              <button
                type="button"
                disabled={!record?.cwd || busy}
                aria-busy={busy}
                className="inline-flex h-[30px] items-center rounded-[8px] border border-[#e5e5e5] bg-white px-2.5 text-[13px] text-[#171717] hover:bg-[#fafafa] disabled:opacity-40">
                {t('workbench.code.pull_request', '创建拉取请求')}
              </button>
            </PopConfirm>
          )}
        </span>
      </header>
      {error ? (
        <p role="alert" className="px-3 py-2 text-[12px] text-[#b42318]">
          {error}
        </p>
      ) : null}
      {diffs.length === 0 ? (
        <p className="grid flex-1 place-items-center px-3 py-16 text-[13px] text-[#8a8a8a]">
          {t('workbench.code.empty_turn', '这一回合还没有改文件')}
        </p>
      ) : (
        <div className="flex min-h-0 flex-1">
          <ul className="w-[240px] shrink-0 overflow-y-auto border-r border-[#efefef]">
            {diffs.map((diff, index) => {
              const { name, directory } = fileParts(diff.path)
              const title = name || t('workbench.diff.untitled', '未命名文件')
              const active = diff === selected
              return (
                <li key={`${diff.path ?? diff.operation}:${index}`}>
                  <button
                    type="button"
                    aria-current={active ? 'true' : undefined}
                    className={`flex h-9 w-full items-center gap-2.5 border-b border-[#efefef] px-4 text-left text-[13px] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset ${
                      active ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'
                    }`}
                    onClick={() => setPickedPath(diff.path ?? '')}>
                    <span
                      className={`inline-flex h-5 shrink-0 items-center rounded-[4px] px-1.5 text-[11px] font-medium text-[#171717] ${
                        active ? 'bg-white' : 'bg-[#f2f3f5]'
                      }`}>
                      {STATUS_MARK[diff.operation]}
                    </span>
                    <span className="min-w-0 flex-1 truncate">
                      {title}
                      {directory ? (
                        <span className="ml-2 text-[12px] font-normal text-[#8a8a8a]">{directory}</span>
                      ) : null}
                    </span>
                    <DiffStatLabel additions={diff.additions} deletions={diff.deletions} />
                  </button>
                </li>
              )
            })}
          </ul>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {selected ? <TaskDiffView diff={selected} onOpenFile={onOpenFile} /> : null}
          </div>
        </div>
      )}
    </div>
  )
}
