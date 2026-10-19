import { Button } from '@a3s-lab/ui'
import { ArrowLeft } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import * as z from 'zod'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings'

import { WorkbenchWorkspaceFilePicker } from '../WorkbenchWorkspaceFiles'
import {
  connectionSchema,
  documentSchema,
  PaperloomError,
  type Paper,
  type PaperloomConnection,
  type PaperloomWorkspaceSource
} from './paperloomApi'
import { PaperloomClient, usePaperloomClient } from './PaperloomClient'
import { PaperloomReader } from './PaperloomReader'
import { readWorkspacePaper, verifyWorkspacePaper, type WorkspacePaperSnapshot } from './paperloomSource'

const FIELD =
  'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10'

export function PaperloomWorkspace({
  onClose,
  question = '',
  closeLabel,
  source,
  onSourceReady
}: {
  onClose: () => void
  question?: string
  closeLabel?: string
  source?: PaperloomWorkspaceSource
  onSourceReady?: () => void
}) {
  const member = useMemberProfile()
  const workspace = useDisplayedWorkspacePath()
  const [owner] = useState(member?.id ?? null)
  const currentMember = member?.id ?? null
  useEffect(() => {
    if (owner !== currentMember) onClose()
  }, [owner, currentMember, onClose])
  if (owner !== currentMember) return null
  const root = source?.root ?? workspace
  return (
    <PaperloomClient memberId={member?.id ?? null}>
      <MemberPaperloom
        key={`${member?.id ?? 'signed-out'}:${root}`}
        root={root}
        paths={source?.paths ?? []}
        onClose={onClose}
        question={question}
        closeLabel={closeLabel}
        onSourceReady={onSourceReady}
      />
    </PaperloomClient>
  )
}

function MemberPaperloom({
  onClose,
  question,
  closeLabel,
  root,
  paths,
  onSourceReady
}: {
  onClose: () => void
  question: string
  closeLabel?: string
  root: string
  paths: readonly string[]
  onSourceReady?: () => void
}) {
  const { t } = useTranslation()
  const { paperloom } = usePaperloomClient()
  const [connection, setConnection] = useState<PaperloomConnection | null>(null)
  const [error, setError] = useState('')
  const [tick, setTick] = useState(0)
  useEffect(() => {
    const abort = new AbortController()
    setError('')
    void paperloom('connection', connectionSchema, { signal: abort.signal })
      .then((value) => {
        if (!abort.signal.aborted) setConnection(value)
      })
      .catch((error) => {
        if (!abort.signal.aborted)
          setError(error instanceof PaperloomError && error.status === 404 ? 'not_installed' : 'connection_load_failed')
      })
    return () => abort.abort()
  }, [tick, paperloom])
  return (
    <section
      data-paperloom-workspace=""
      aria-label={t('paperloom.title')}
      className="flex h-full min-h-0 flex-col bg-white text-[13px] text-[#171717]">
      <header className="flex min-h-12 shrink-0 items-center gap-2 border-b border-[#efefef] px-3">
        <Button variant="ghost" size="icon-sm" aria-label={closeLabel ?? t('paperloom.back')} onClick={onClose}>
          <ArrowLeft size={16} strokeWidth={1.7} />
        </Button>
        <h1 className="min-w-0 flex-1 truncate text-[15px] font-medium">{t('paperloom.title')}</h1>
      </header>
      {error ? (
        <div role="alert" className="flex shrink-0 items-center gap-2 px-4 py-2 text-[#b42318]">
          <p className="flex-1">{t(`paperloom.${error}`)}</p>
          <Button variant="ghost" size="sm" onClick={() => setTick((value) => value + 1)}>
            {t('paperloom.retry')}
          </Button>
        </div>
      ) : !connection ? (
        <p role="status" className="shrink-0 px-4 py-2 text-[#8a8a8a]">
          {t('paperloom.loading')}
        </p>
      ) : !connection.apiKeyConfigured ? (
        <p className="shrink-0 px-4 py-2 text-[#8a8a8a]">{t('paperloom.processing_unavailable')}</p>
      ) : null}
      {connection && error !== 'not_installed' && (
        <WorkspacePaper
          onSourceReady={onSourceReady}
          root={root}
          paths={paths}
          question={question}
          connectionRevision={connection.apiKeyConfigured ? connection.revision : ''}
        />
      )}
    </section>
  )
}

function WorkspacePaper({
  root,
  paths,
  question,
  connectionRevision,
  onSourceReady
}: {
  root: string
  paths: readonly string[]
  question: string
  connectionRevision: string
  onSourceReady?: () => void
}) {
  const { t } = useTranslation()
  const { paperloom } = usePaperloomClient()
  const selectedPath = paths.length === 1 ? paths[0] : ''
  const [path, setPath] = useState(selectedPath)
  const [openedPath, setOpenedPath] = useState(selectedPath)
  const [source, setSource] = useState<WorkspacePaperSnapshot | null>(null)
  const [displayedSource, setDisplayedSource] = useState<WorkspacePaperSnapshot | null>(null)
  const [registered, setRegistered] = useState<Paper | null>(null)
  const [error, setError] = useState('')
  const [serviceError, setServiceError] = useState('')
  const [loading, setLoading] = useState(false)
  const [tick, setTick] = useState(0)
  const sourceAcknowledged = useRef(false)
  const handleSourceReady = useCallback(() => {
    if (!source) return
    setDisplayedSource(source)
    if (sourceAcknowledged.current) return
    sourceAcknowledged.current = true
    onSourceReady?.()
  }, [source, onSourceReady])
  useEffect(() => {
    setPath(selectedPath)
    setOpenedPath(selectedPath)
  }, [selectedPath])
  useEffect(() => {
    const abort = new AbortController()
    setSource(null)
    setError('')
    setLoading(Boolean(openedPath && root))
    if (!openedPath || !root) return () => abort.abort()
    setLoading(true)
    void readWorkspacePaper(root, openedPath, abort.signal)
      .then((value) => {
        if (!abort.signal.aborted) setSource(value)
      })
      .catch((reason) => {
        if (!abort.signal.aborted) setError(reason instanceof PaperloomError ? reason.code : 'workspace_source_failed')
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false)
      })
    return () => abort.abort()
  }, [root, openedPath, tick])
  useEffect(() => {
    const abort = new AbortController()
    setRegistered(null)
    setServiceError('')
    if (!source || source !== displayedSource || !connectionRevision) return () => abort.abort()
    // The service indexes this immutable snapshot; file ownership stays in the workspace.
    void paperloom(`uploads?name=${encodeURIComponent(source.name)}`, z.object({ document_id: z.string().min(1) }), {
      method: 'POST',
      headers: { 'content-type': 'application/pdf' },
      body: source.bytes.slice(),
      signal: abort.signal
    })
      .then(async (uploaded) => {
        if (abort.signal.aborted) return
        if (uploaded.document_id !== source.digest) throw new PaperloomError(409, 'source_changed')
        const document = await paperloom(`documents/${uploaded.document_id}`, documentSchema, { signal: abort.signal })
        if (document.document_id !== source.digest) throw new PaperloomError(409, 'source_changed')
        if (!abort.signal.aborted) setRegistered(document)
      })
      .catch((reason) => {
        if (!abort.signal.aborted)
          setServiceError(
            reason instanceof PaperloomError && reason.code === 'source_changed'
              ? 'source_changed'
              : 'source_processing_failed'
          )
      })
    return () => abort.abort()
  }, [source, displayedSource, connectionRevision, paperloom])
  const paper = useMemo<Paper | null>(
    () =>
      source
        ? {
            document_id: source.digest,
            bytes: source.bytes.length,
            page_count: 0,
            reading_status: 'unread',
            added_at: '',
            updated_at: '',
            tags: [],
            ...registered,
            // A reused internal content index may have another filename.
            title: source.name,
            source_filename: source.name
          }
        : null,
    [source, registered]
  )
  const verifySource = useCallback(async () => {
    if (!source) throw new PaperloomError(400, 'workspace_source_required')
    await verifyWorkspacePaper(source)
  }, [source])
  if (source && paper)
    return (
      <>
        <div className="flex shrink-0 items-center gap-2 border-b border-[#efefef] px-3 py-2 text-[12px] text-[#8a8a8a]">
          <p className="min-w-0 flex-1 truncate" title={source.path}>
            {source.path}
          </p>
          {serviceError && <span role="alert">{t(`paperloom.${serviceError}`)}</span>}
          {serviceError && (
            <Button variant="ghost" size="sm" onClick={() => setTick((value) => value + 1)}>
              {t('paperloom.retry')}
            </Button>
          )}
        </div>
        <PaperloomReader
          key={`${source.path}:${source.digest}`}
          paper={paper}
          question={question}
          source={source}
          onSourceReady={handleSourceReady}
          serviceReady={registered !== null}
          verifySource={verifySource}
          onBack={() => {
            setSource(null)
            setOpenedPath('')
          }}
        />
      </>
    )
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4 overflow-auto p-6">
      <h2 className="text-[15px] font-medium">{t('paperloom.choose_workspace_pdf')}</h2>
      <p className="max-w-md text-center text-[#8a8a8a]">{t('paperloom.workspace_source_hint')}</p>
      <p className="max-w-full text-[12px] break-all text-[#8a8a8a]">
        {root || t('paperloom.workspace_source_required')}
      </p>
      {root && (
        <WorkbenchWorkspaceFilePicker
          root={root}
          selectedPaths={path ? [path] : []}
          onCommit={(next) => {
            if (next.length !== 1 || next[0].isDirectory) {
              setError('choose_one_pdf')
              return
            }
            setPath(next[0].path)
            setOpenedPath(next[0].path)
          }}
        />
      )}
      <form
        className="grid w-full max-w-md gap-3"
        onSubmit={(event) => {
          event.preventDefault()
          setOpenedPath(path)
          setTick((value) => value + 1)
        }}>
        <label className="grid gap-2">
          {t('paperloom.workspace_pdf_path')}
          <input
            className={FIELD}
            value={path}
            onChange={(event) => setPath(event.target.value)}
            disabled={loading || !root}
          />
        </label>
        <Button disabled={loading || !root || !path.trim()} type="submit">
          {t(loading ? 'paperloom.loading' : 'paperloom.open_pdf')}
        </Button>
      </form>
      {paths.length > 1 && <p role="status">{t('paperloom.choose_one_pdf')}</p>}
      {error && (
        <p role="alert" className="text-[#b42318]">
          {t(`paperloom.${error}`)}
        </p>
      )}
    </div>
  )
}
