import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'

import { loggerService } from '@logger'
import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings'
import type { DocumentResultRequest } from '@shared/types/documentResult'

import { cloudWorkspaceRoot } from './cloudWorkspace'
import { documentResultInvoke, resultBytes } from './documentResultHandoff'
import { SaveDocumentDialog } from './SaveDocumentDialog'

const logger = loggerService.withContext('DocumentResultSaveHost')

/** Main-window save surface uses the same account and root as Knowledge Documents. */
export function DocumentResultSaveHost() {
  const center = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const workspace = useDisplayedWorkspacePath()
  const root = center === PERSONAL_CENTER ? workspace : cloudWorkspaceRoot(center)
  const [request, setRequest] = useState<(DocumentResultRequest & { root: string }) | null>(null)
  const [completion, setCompletion] = useState<{ id: string; location: string | null } | null>(null)
  const saved = useRef<string | null>(null)
  const file = useMemo(
    () =>
      request
        ? new globalThis.File([resultBytes(request.file.bytesBase64).slice()], request.file.name, {
            type: request.file.mediaType
          })
        : null,
    [request]
  )

  useEffect(() => {
    let active = true
    let polling = false
    const poll = async () => {
      const invoke = documentResultInvoke()
      if (!invoke) return
      if (polling || (request && !completion)) return
      polling = true
      try {
        if (completion) {
          await invoke('finish_document_result_save', completion)
          if (active) {
            setRequest(null)
            setCompletion(null)
          }
          return
        }
        const next = (await invoke('take_document_result_save')) as DocumentResultRequest | null
        if (active && next) {
          saved.current = null
          setRequest({ ...next, root })
        }
      } catch (error) {
        logger.error('Document result handoff unavailable', error as Error)
        if (active && completion && error === 'Document result request refused.') {
          setRequest(null)
          setCompletion(null)
        }
      } finally {
        polling = false
      }
    }
    void poll()
    const timer = globalThis.setInterval(() => void poll(), 750)
    return () => {
      active = false
      globalThis.clearInterval(timer)
    }
  }, [request, completion, root])

  if (!request || !file || completion) return null
  return (
    <SaveDocumentDialog
      key={request.id}
      root={request.root}
      file={file}
      onSaved={(location) => {
        saved.current = location
      }}
      onClose={() => setCompletion({ id: request.id, location: saved.current })}
    />
  )
}
