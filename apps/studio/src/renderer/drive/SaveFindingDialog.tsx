import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'

import { driveApi, type DriveNode } from './driveTransport'

/**
 * Confirmed write of one markdown file into a folder that already exists.
 * The root has no folder yet, so confirm stays off until the member opens one.
 */
export function SaveFindingDialog({
  open,
  text,
  sessionLabel,
  onSave,
  onClose
}: {
  open: boolean
  text?: string
  sessionLabel?: string
  /** Writes the chosen files. Absent means one markdown copy of `text`. */
  onSave?: (parentId: string) => Promise<void>
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [parentId, setParentId] = useState<string | undefined>(undefined)
  const [trail, setTrail] = useState<DriveNode[]>([])
  const [nodes, setNodes] = useState<DriveNode[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const presence = useRestingDialog(open)

  useEffect(() => {
    if (presence.present) return
    setParentId(undefined)
    setTrail([])
    setNodes([])
    setError(null)
    setBusy(false)
  }, [presence.present])

  useEffect(() => {
    if (!open) return
    let active = true
    setError(null)
    void driveApi
      .listNodes(parentId)
      .then((rows) => {
        if (active) setNodes(rows)
      })
      .catch((reason: unknown) => {
        if (active) setError(reason instanceof Error ? reason.message : String(reason))
      })
    return () => {
      active = false
    }
  }, [open, parentId])

  if (!presence.present) return null

  const save = () => {
    if (!parentId || busy) return
    setBusy(true)
    const write = onSave
      ? onSave(parentId)
      : driveApi.upload(
          new File([text ?? ''], 'finding.md', { type: 'text/markdown' }),
          parentId,
          undefined,
          sessionLabel
        )
    void write
      .then(() => onClose())
      .catch((reason: unknown) => {
        setBusy(false)
        setError(reason instanceof Error ? reason.message : String(reason))
      })
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('messages.save_finding_title')}
      data-ui="messages.save-finding"
      data-state={presence.state}
      className={`a3s-dialog-scrim fixed inset-0 z-50 flex items-center justify-center bg-black/20 ${presence.state === 'closed' ? 'pointer-events-none' : ''}`}>
      <div
        data-state={presence.state}
        className="a3s-dialog-surface w-[360px] rounded-[10px] border border-[#ebebeb] bg-white p-4 shadow-[0_8px_22px_rgba(20,40,80,0.08)]">
        <h2 className="m-0 text-[15px] font-medium">{t('messages.save_finding_title')}</h2>
        <p className="m-0 mt-1 text-xs text-muted-foreground">{t('messages.save_finding_pick')}</p>
        {trail.length > 0 ? (
          <button
            type="button"
            className="mt-3 text-xs text-foreground underline"
            onClick={() => {
              const next = trail.slice(0, -1)
              setTrail(next)
              setParentId(next.at(-1)?.id)
            }}>
            {trail.map((node) => node.name).join(' / ')}
          </button>
        ) : null}
        <ul className="m-0 mt-3 max-h-48 list-none overflow-auto p-0">
          {nodes.length === 0 ? (
            <li className="text-sm text-muted-foreground">{t('messages.save_finding_empty')}</li>
          ) : (
            nodes.map((node) => (
              <li key={node.id}>
                <button
                  type="button"
                  data-ui="messages.save-finding.folder"
                  className="flex h-8 w-full items-center px-2 text-left text-sm hover:bg-[#f7f7f7]"
                  onClick={() => {
                    setTrail((current) => [...current, node])
                    setParentId(node.id)
                  }}>
                  {node.name}
                </button>
              </li>
            ))
          )}
        </ul>
        {error ? <p className="m-0 mt-2 text-xs text-destructive">{error}</p> : null}
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" data-ui="messages.save-finding.cancel" className="h-8 px-3 text-sm" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            type="button"
            data-ui="messages.save-finding.confirm"
            className="h-8 bg-[#171717] px-3 text-sm text-white disabled:opacity-40"
            disabled={!parentId || busy}
            onClick={save}>
            {t('messages.save_finding_confirm')}
          </button>
        </div>
      </div>
    </div>
  )
}
