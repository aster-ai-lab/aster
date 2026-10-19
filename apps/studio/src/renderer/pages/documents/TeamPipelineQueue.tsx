import { Popover, PopoverContent, PopoverTrigger } from '@a3s-lab/ui'
import { BookOpen, ListTodo, ScanText } from 'lucide-react'
import { Children, useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { driveApi, type PipelineQueue } from '@renderer/drive/driveTransport'

const EMPTY: PipelineQueue = { parse: [], compile: [] }

const COMPILE_LABEL: Record<PipelineQueue['compile'][number]['status'], string> = {
  queued: 'documents.pipeline.summary.queued',
  running: 'documents.pipeline.summary.compiling',
  done: 'documents.pipeline.compile.done',
  failed: 'documents.pipeline.summary.failed'
}

/** Top-right queue for files waiting to be parsed and encyclopedia compile jobs. */
export function TeamPipelineQueue({
  parsingId,
  onParse,
  onOpen
}: {
  parsingId: string | null
  onParse: (id: string, name: string) => void
  onOpen: (file: { id: string; name: string }) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [queue, setQueue] = useState<PipelineQueue>(EMPTY)
  const [failed, setFailed] = useState(false)
  const active =
    queue.parse.length + queue.compile.filter((task) => task.status === 'queued' || task.status === 'running').length

  useEffect(() => {
    let live = true
    const load = () => {
      driveApi.pipeline().then(
        (next) => {
          if (!live) return
          setQueue(next)
          setFailed(false)
        },
        () => {
          if (!live) return
          setFailed(true)
        }
      )
    }
    load()
    const timer = window.setInterval(load, open || active > 0 ? 2000 : 8000)
    return () => {
      live = false
      window.clearInterval(timer)
    }
  }, [open, active])

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t('documents.queue.open')}
          data-ui="documents.queue"
          className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
          <ListTodo className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
          {t('documents.queue.title')}
          {active > 0 ? (
            <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#171717]">
              {active}
            </span>
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="bottom"
        sideOffset={8}
        className="w-[320px] rounded-[12px] border-[1px] border-[#e5e5e5] bg-white p-3 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div role="dialog" aria-label={t('documents.queue.open')} data-ui="documents.queue.panel">
          <p className="m-0 px-1 pb-2 text-[13px] font-medium">{t('documents.queue.open')}</p>
          {failed ? (
            <p className="m-0 px-1 py-2 text-[13px] text-[#8a8a8a]" role="alert">
              {t('documents.queue.failed')}
            </p>
          ) : (
            <div className="flex max-h-[360px] flex-col gap-3 overflow-y-auto">
              <QueueSection title={t('documents.queue.parse')} empty={t('documents.queue.parse_empty')}>
                {queue.parse.map((task) => (
                  <li key={task.fileId} className="flex items-center gap-2 px-1 py-1">
                    <span className="min-w-0 flex-1 truncate text-[13px]">{task.fileName}</span>
                    <span
                      className={
                        task.status === 'unparsed' && parsingId !== task.fileId
                          ? 'inline-flex h-5 shrink-0 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#b42318]'
                          : 'inline-flex h-5 shrink-0 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#171717]'
                      }>
                      {t(parseQueueLabel(task.status, parsingId === task.fileId))}
                    </span>
                    <button
                      type="button"
                      aria-label={t('documents.encyclopedia.parse')}
                      title={t('documents.encyclopedia.parse')}
                      className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:opacity-40"
                      disabled={parsingId === task.fileId}
                      onClick={() => onParse(task.fileId, task.fileName)}>
                      <ScanText className="size-3.5" strokeWidth={1.7} aria-hidden />
                    </button>
                  </li>
                ))}
              </QueueSection>
              <QueueSection title={t('documents.queue.compile')} empty={t('documents.queue.compile_empty')}>
                {queue.compile.map((task) => (
                  <li key={task.id} className="flex items-center gap-2 px-1 py-1">
                    <span className="min-w-0 flex-1 truncate text-[13px]">
                      {task.fileName || t('documents.queue.missing')}
                    </span>
                    <span
                      className={
                        task.status === 'failed'
                          ? 'inline-flex h-5 shrink-0 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#b42318]'
                          : task.status === 'done'
                            ? 'shrink-0 text-[13px] text-[#8a8a8a]'
                            : 'inline-flex h-5 shrink-0 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#171717]'
                      }>
                      {t(COMPILE_LABEL[task.status])}
                    </span>
                    {task.fileId && task.status === 'done' ? (
                      <button
                        type="button"
                        aria-label={t('documents.encyclopedia.open')}
                        title={t('documents.encyclopedia.open')}
                        className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                        onClick={() => onOpen({ id: task.fileId!, name: task.fileName })}>
                        <BookOpen className="size-3.5" strokeWidth={1.7} aria-hidden />
                      </button>
                    ) : null}
                  </li>
                ))}
              </QueueSection>
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

function parseQueueLabel(status: PipelineQueue['parse'][number]['status'], parsingNow: boolean): string {
  if (parsingNow || status === 'parsing') return 'documents.pipeline.summary.parsing'
  if (status === 'unparsed') return 'documents.pipeline.summary.unparsed'
  return 'documents.pipeline.summary.stored'
}

function QueueSection({ title, empty, children }: { title: string; empty: string; children: ReactNode }) {
  const present = Children.count(children) > 0
  return (
    <section>
      <h3 className="m-0 px-1 pb-1 text-[12px] leading-4 font-normal text-[#8a8a8a]">{title}</h3>
      {present ? (
        <ul className="m-0 list-none p-0">{children}</ul>
      ) : (
        <p className="m-0 px-1 text-[12px] text-[#8a8a8a]">{empty}</p>
      )}
    </section>
  )
}
