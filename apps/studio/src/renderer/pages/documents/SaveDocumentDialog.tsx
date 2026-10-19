import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@a3s-lab/ui'
import { ArrowLeft, Folder, LoaderCircle } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { importLocalFile, listLocalEntries, localEntryStatus, type LocalEntry } from './useLocalDocuments'

const QUIET =
  'h-9 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] shadow-none hover:bg-[#fafafa] focus-visible:ring-black/10 focus-visible:ring-inset'

/** Uses the Knowledge Documents tree and its collision-safe file import. */
export function SaveDocumentDialog({
  root,
  file,
  beforeSave,
  destination,
  onSaved,
  onClose
}: {
  root: string
  file: InstanceType<typeof globalThis.File>
  beforeSave?: () => Promise<void>
  destination?: { title: string; description: string; rootLabel: string }
  onSaved: (directory: string, path: string) => void
  onClose: () => void
}) {
  const { t } = useTranslation()
  const live = useRef(true)
  useEffect(() => {
    live.current = true
    return () => {
      live.current = false
    }
  }, [])
  const [trail, setTrail] = useState<LocalEntry[]>([])
  const [folders, setFolders] = useState<LocalEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [retry, setRetry] = useState(0)
  const directory = trail.at(-1)?.path ?? root
  const location =
    trail.map((entry) => entry.name).join(' / ') || destination?.rootLabel || t('knowledge.hub.documents')

  useEffect(() => {
    const abort = new globalThis.AbortController()
    setFolders([])
    setLoading(true)
    setError(null)
    if (!root) {
      setError('documents.result.unavailable')
      setLoading(false)
      return () => abort.abort()
    }
    void listLocalEntries(root, directory, abort.signal)
      .then((listing) => {
        if (abort.signal.aborted) return
        setFolders(listing.entries.filter((entry) => entry.kind === 'directory'))
        setLoading(false)
      })
      .catch(() => {
        if (abort.signal.aborted) return
        setError('documents.result.load_failed')
        setLoading(false)
      })
    return () => abort.abort()
  }, [root, directory, retry])

  const save = async () => {
    if (loading || busy || !root) return
    setBusy(true)
    setError(null)
    try {
      await beforeSave?.()
      if (!live.current) return
      await importLocalFile(root, directory, file, globalThis.AbortSignal.timeout(60_000), false)
      if (!live.current) return
      onSaved(location, `${directory.replace(/[/\\]+$/u, '')}/${file.name}`)
      onClose()
    } catch (reason) {
      if (!live.current) return
      setError(localEntryStatus(reason) === 409 ? 'documents.result.exists' : 'documents.result.save_failed')
      setBusy(false)
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose()
      }}>
      <DialogContent
        size="sm"
        showCloseButton={!busy}
        closeOnOverlayClick={!busy}
        closeLabel={t('common.close')}
        data-ui="document-tools.save-dialog"
        onEscapeKeyDown={(event) => {
          if (busy) event.preventDefault()
        }}
        onInteractOutside={(event) => {
          if (busy) event.preventDefault()
        }}>
        <DialogHeader>
          <DialogTitle>{destination?.title ?? t('documents.result.save')}</DialogTitle>
          <DialogDescription>
            {destination?.description ?? t('documents.result.destination', { name: file.name })}
          </DialogDescription>
        </DialogHeader>
        <div className="flex min-w-0 items-center gap-2 text-[13px] text-[#3f3f3f]">
          {trail.length > 0 ? (
            <Button
              variant="ghost"
              className="size-6 shrink-0 rounded-[6px] p-0 focus-visible:ring-black/10 focus-visible:ring-inset"
              aria-label={t('documents.result.up')}
              disabled={busy}
              onClick={() => setTrail((value) => value.slice(0, -1))}>
              <ArrowLeft size={14} strokeWidth={1.7} aria-hidden />
            </Button>
          ) : null}
          <span className="min-w-0 break-words" data-ui="document-tools.save-location">
            {location}
          </span>
        </div>
        <div className="max-h-64 overflow-y-auto" aria-busy={loading || busy}>
          {loading ? (
            <p role="status" className="py-6 text-[13px] text-[#8a8a8a]">
              {t('file_preview.loading')}
            </p>
          ) : folders.length ? (
            <ul className="flex flex-col gap-0.5">
              {folders.map((folder) => (
                <li key={folder.path}>
                  <Button
                    variant="ghost"
                    disabled={busy}
                    className="h-9 w-full justify-start gap-2.5 rounded-[8px] px-2.5 text-[13px] font-normal text-[#3f3f3f] hover:bg-[#f7f7f7] focus-visible:ring-black/10 focus-visible:ring-inset"
                    onClick={() => setTrail((value) => [...value, folder])}>
                    <Folder size={16} strokeWidth={1.7} aria-hidden />
                    <span className="truncate">{folder.name}</span>
                  </Button>
                </li>
              ))}
            </ul>
          ) : !error ? (
            <p className="py-6 text-[13px] text-[#8a8a8a]">{t('documents.result.no_folders')}</p>
          ) : null}
        </div>
        {error ? (
          <div role="alert" className="text-[13px] text-[#b42318]">
            {t(error)}
            {error === 'documents.result.load_failed' ? (
              <Button variant="ghost" onClick={() => setRetry((value) => value + 1)}>
                {t('documents.result.retry')}
              </Button>
            ) : null}
          </div>
        ) : null}
        <DialogFooter>
          <Button variant="outline" className={QUIET} disabled={busy} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button
            className="h-9 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white shadow-none hover:bg-[#171717]/90 focus-visible:ring-black/10 focus-visible:ring-inset"
            disabled={
              !root ||
              loading ||
              busy ||
              error === 'documents.result.load_failed' ||
              error === 'documents.result.unavailable'
            }
            onClick={() => void save()}>
            {busy ? <LoaderCircle size={16} strokeWidth={1.7} className="animate-spin" aria-hidden /> : null}
            {t(busy ? 'documents.result.saving' : 'documents.result.save_here')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
