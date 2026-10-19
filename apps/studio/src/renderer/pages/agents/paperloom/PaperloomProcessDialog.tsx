import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@a3s-lab/ui'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import * as z from 'zod'

import { jsonBody, PaperloomError, type Paper } from './paperloomApi'
import { usePaperloomClient } from './PaperloomClient'
import { paperloomDialogStyle } from './paperloomUi'

export function paperloomPageRange(first: number, last: number, count: number): number[] | null {
  if (
    !Number.isInteger(first) ||
    !Number.isInteger(last) ||
    first < 1 ||
    last < first ||
    last > count ||
    last - first > 5000
  )
    return null
  return Array.from({ length: last - first + 1 }, (_, index) => first + index)
}

export function PaperloomProcessDialog({
  open,
  onOpenChange,
  paper,
  verifySource,
  onSubmitted
}: {
  open: boolean
  onOpenChange: (value: boolean) => void
  paper: Paper
  verifySource: () => Promise<void>
  onSubmitted: (jobId: string) => void
}) {
  const { t } = useTranslation()
  const { paperloom } = usePaperloomClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const submit = async () => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await verifySource()
      const submitted = await paperloom(
        `documents/${paper.document_id}/ocr`,
        z.object({ job_id: z.string(), status: z.string() }),
        jsonBody({ workflow: 'ocr', source: {}, ocr: { provider: 'a3s_gateway' } })
      )
      onOpenChange(false)
      onSubmitted(submitted.job_id)
    } catch (reason) {
      setError(
        reason instanceof PaperloomError && reason.code === 'source_changed' ? 'source_changed' : 'process_failed'
      )
    } finally {
      setBusy(false)
    }
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (!busy) onOpenChange(value)
      }}>
      <DialogContent
        overlayClassName="z-[220]"
        size="default"
        style={paperloomDialogStyle(512)}
        showCloseButton={false}
        className="z-[221] flex max-h-[calc(100dvh-3rem)] flex-col">
        <DialogHeader className="shrink-0 text-left">
          <DialogTitle className="text-[15px] font-medium">{t('paperloom.process_paper')}</DialogTitle>
          <DialogDescription className="truncate">{paper.title}</DialogDescription>
        </DialogHeader>
        <div data-paperloom-process-fields="" className="min-h-0 flex-1 overflow-y-auto">
          <div className="grid gap-3 text-[13px]">
            <p className="leading-5 text-[#8a8a8a]">{t('paperloom.ocr_required_hint')}</p>
            <p>
              <span className="text-[#8a8a8a]">{t('paperloom.ocr_provider')}</span>
              <span className="ms-2" data-paperloom-ocr-source="text-layer">
                {t('paperloom.os_ocr')}
              </span>
            </p>
            <p className="text-[12px] leading-5 text-[#8a8a8a]">{t('paperloom.os_ocr_credentials')}</p>
          </div>
        </div>
        {error && (
          <p role="alert" className="shrink-0 text-[#b42318]">
            {t(`paperloom.${error}`)}
          </p>
        )}
        <DialogFooter className="shrink-0 flex-row justify-end">
          <Button
            variant="outline"
            size="md"
            className="rounded-[8px]"
            disabled={busy}
            onClick={() => onOpenChange(false)}>
            {t('paperloom.cancel')}
          </Button>
          <Button size="md" className="rounded-[8px]" disabled={busy} onClick={() => void submit()}>
            {t(busy ? 'paperloom.submitting' : 'paperloom.start_processing')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
