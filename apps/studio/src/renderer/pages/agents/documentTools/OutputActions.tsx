import { Button } from '@a3s-lab/ui'
import { Download, FolderInput } from 'lucide-react'
import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { cloudWorkspaceRoot } from '@renderer/pages/documents/cloudWorkspace'
import { requestDocumentResultSave } from '@renderer/pages/documents/documentResultHandoff'
import { SaveDocumentDialog } from '@renderer/pages/documents/SaveDocumentDialog'
import { useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings'
import { readAppletRuntime } from '@shared/appletRuntime'

import type { ToolOutput } from '../pdfToolsRun'

/** Both commands use the accepted result bytes; saving never replaces the download. */
export function OutputActions({ file, url }: { file: ToolOutput; url: string }) {
  const { t } = useTranslation()
  const center = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const workspace = useDisplayedWorkspacePath()
  const root = center === PERSONAL_CENTER ? workspace : cloudWorkspaceRoot(center)
  const [opened, setOpened] = useState(false)
  const [saved, setSaved] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [failed, setFailed] = useState(false)
  const revision = useRef<Uint8Array | null>(file.bytes)
  revision.current = file.bytes
  useEffect(() => {
    revision.current = file.bytes
    return () => {
      revision.current = null
    }
  }, [file.bytes])
  useEffect(() => {
    setOpened(false)
    setSaved(null)
    setBusy(false)
    setFailed(false)
  }, [root, file.bytes])
  const save = async () => {
    if (readAppletRuntime()?.appId !== 'pdf') {
      setOpened(true)
      return
    }
    const bytes = file.bytes
    setBusy(true)
    setFailed(false)
    try {
      const location = await requestDocumentResultSave(file.name, file.mediaType, bytes)
      if (revision.current === bytes) setSaved(location)
    } catch {
      if (revision.current === bytes) setFailed(true)
    } finally {
      if (revision.current === bytes) setBusy(false)
    }
  }
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2" data-ui="document-tools.output-actions">
      <Button
        className="h-9 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white shadow-none hover:bg-[#171717]/90 focus-visible:ring-black/10 focus-visible:ring-inset"
        aria-label={t('documents.result.save_file', { name: file.name })}
        disabled={busy}
        onClick={() => void save()}>
        <FolderInput size={16} strokeWidth={1.7} aria-hidden />
        {t(busy ? 'documents.result.saving' : 'documents.result.save')}
      </Button>
      <Button
        asChild
        variant="outline"
        className="h-9 rounded-[8px] border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] shadow-none hover:bg-[#fafafa] focus-visible:ring-black/10 focus-visible:ring-inset">
        <a
          href={url}
          download={file.name}
          aria-label={t('workbench.feature.pdf.tools.workspace.download_file', { name: file.name })}>
          <Download size={16} strokeWidth={1.7} aria-hidden />
          {t('documents.result.download')}
        </a>
      </Button>
      {saved ? (
        <span role="status" className="text-[12px] text-[#8a8a8a]">
          {t('documents.result.saved', { location: saved })}
        </span>
      ) : null}
      {failed ? (
        <span role="alert" className="text-[12px] text-[#b42318]">
          {t('documents.result.save_failed')}
        </span>
      ) : null}
      {opened ? (
        <SaveDocumentDialog
          key={root}
          root={root}
          file={new globalThis.File([file.bytes.slice()], file.name, { type: file.mediaType })}
          onSaved={setSaved}
          onClose={() => setOpened(false)}
        />
      ) : null}
    </div>
  )
}
