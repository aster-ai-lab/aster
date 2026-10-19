import { Tooltip } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import type { LocalEntry } from '@renderer/pages/documents/useLocalDocuments'
import { cn } from '@renderer/utils/style'

type CompileStatus = NonNullable<LocalEntry['compileStatus']>
type ParseStatus = NonNullable<LocalEntry['parseStatus']>

/**
 * One scan line for upload, parse, and compile.
 * A listed file is already uploaded. Parse and compile stay discrete stages:
 * there is no byte percentage, so a bar would invent progress.
 */
export function DocumentPipelineStatus({
  parseStatus = 'stored',
  compileStatus = 'skipped',
  parsing = false
}: {
  parseStatus?: ParseStatus
  compileStatus?: CompileStatus
  parsing?: boolean
}) {
  const { t } = useTranslation()
  const parsed = parseStatus === 'parsed'
  const refused = parseStatus === 'unparsed'
  const summaryKey = summaryFor(parsing || parseStatus === 'parsing', refused, parsed, compileStatus)
  const upload = t('documents.pipeline.upload.uploaded')
  const parse = t(
    parsing || parseStatus === 'parsing'
      ? 'documents.pipeline.summary.parsing'
      : refused
        ? 'documents.pipeline.parse.unparsed'
        : parsed
          ? 'documents.pipeline.parse.parsed'
          : 'documents.pipeline.parse.stored'
  )
  const compile = t(`documents.pipeline.compile.${compileStatus}`)
  const detail = `${upload} · ${parse} · ${compile}`
  const tone = chipTone(parsing || parseStatus === 'parsing', refused, parsed, compileStatus)

  return (
    <Tooltip content={detail}>
      <div
        role="group"
        aria-label={detail}
        data-upload-status="uploaded"
        data-parse-status={parsing || parseStatus === 'parsing' ? 'parsing' : parseStatus}
        data-compile-status={compileStatus}
        className="flex min-w-0 items-center">
        <span className={cn('truncate', CHIP[tone])}>{t(summaryKey)}</span>
      </div>
    </Tooltip>
  )
}

const CHIP = {
  current: 'inline-flex h-5 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#171717]',
  settled: 'text-[13px] text-[#8a8a8a]',
  failed: 'inline-flex h-5 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#b42318]'
} as const

function chipTone(
  parsing: boolean,
  refused: boolean,
  parsed: boolean,
  compileStatus: CompileStatus
): keyof typeof CHIP {
  if (refused) return 'failed'
  if (!parsing && parsed && compileStatus === 'failed') return 'failed'
  if (!parsing && parsed && compileStatus === 'done') return 'settled'
  return 'current'
}

function summaryFor(parsing: boolean, refused: boolean, parsed: boolean, compileStatus: CompileStatus): string {
  if (parsing) return 'documents.pipeline.summary.parsing'
  if (refused) return 'documents.pipeline.summary.unparsed'
  if (!parsed) return 'documents.pipeline.summary.stored'
  if (compileStatus === 'running') return 'documents.pipeline.summary.compiling'
  if (compileStatus === 'queued') return 'documents.pipeline.summary.queued'
  if (compileStatus === 'waiting') return 'documents.pipeline.summary.waiting'
  if (compileStatus === 'failed') return 'documents.pipeline.summary.failed'
  if (compileStatus === 'done') return 'documents.pipeline.summary.ready'
  return 'documents.pipeline.compile.skipped'
}
