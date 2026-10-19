import { Button, SearchInput, SearchSelect, SegmentedControl } from '@a3s-lab/ui'
import {
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  Download,
  FolderInput,
  MessageSquare,
  Play,
  RefreshCw,
  Square,
  X
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import * as z from 'zod'

import { SaveDocumentDialog } from '@renderer/pages/documents/SaveDocumentDialog'
import type { WorkbenchProof } from '@shared/types/workbenchProof'

import { WorkbenchLineagePane } from '../WorkbenchLineagePane'
import { documentSchema, jobsSchema, jsonBody, PaperloomError, type Paper, type PaperJob } from './paperloomApi'
import { usePaperloomClient } from './PaperloomClient'
import { PaperloomPdf } from './PaperloomPdf'
import { PaperloomProcessDialog } from './PaperloomProcessDialog'
import type { WorkspacePaperSnapshot } from './paperloomSource'

const artifactSchema = z.object({
  job_id: z.string(),
  items: z.array(
    z.object({
      artifact_key: z.string(),
      ready: z.boolean(),
      file_name: z.string().nullish(),
      artifact_kind: z.string(),
      checksum: z.string().nullish(),
      attempt: z.number().int()
    })
  )
})
const citationSchema = z.object({
  ref: z.number().int(),
  document_id: z.string(),
  job_id: z.string(),
  page_idx: z.number().int().nonnegative(),
  block_id: z.string(),
  snippet: z.string()
})
const answerSchema = z.object({
  answer: z.string().min(1),
  citations: z.array(citationSchema),
  persisted: z.boolean(),
  conversation_id: z.string().optional()
})
const searchSchema = z.object({
  hits: z.array(
    z.object({
      document_id: z.string(),
      job_id: z.string(),
      page_idx: z.number().int().nonnegative(),
      block_id: z.string(),
      source_snippet: z.string(),
      translated_snippet: z.string()
    })
  )
})
type Answer = z.infer<typeof answerSchema>

export function PaperloomReader({
  paper: initial,
  onBack,
  question: initialQuestion,
  source,
  onSourceReady,
  serviceReady,
  verifySource
}: {
  paper: Paper
  onBack: () => void
  question: string
  source: WorkspacePaperSnapshot
  onSourceReady?: () => void
  serviceReady: boolean
  verifySource: () => Promise<void>
}) {
  const { t } = useTranslation()
  const {
    paperloom,
    paperloomResponse,
    paperloomArtifactBytes,
    registerSavedArtifact,
    provenanceTransport,
    lookupSourceProvenance
  } = usePaperloomClient()
  const [paper, setPaper] = useState(initial)
  const [jobs, setJobs] = useState<PaperJob[]>([])
  const [jobId, setJobId] = useState(initial.active_job_id ?? '')
  const [page, setPage] = useState(1)
  const [pages, setPages] = useState(initial.page_count)
  const [zoom, setZoom] = useState('1')
  const [view, setView] = useState('original')
  const [tab, setTab] = useState<'document' | 'tasks' | 'artifacts'>('document')
  const [process, setProcess] = useState(false)
  const [saveFile, setSaveFile] = useState<{
    file: File
    jobId: string
    key: string
    checksum: string
    attempt: number
  } | null>(null)
  const [savedArtifact, setSavedArtifact] = useState<{ path: string; artifact: NonNullable<typeof saveFile> } | null>(
    null
  )
  const [proof, setProof] = useState<WorkbenchProof | null>(null)
  const [storedProofs, setStoredProofs] = useState<WorkbenchProof[]>([])
  const [storedProofsError, setStoredProofsError] = useState(false)
  useEffect(() => {
    let live = true
    setStoredProofs([])
    setStoredProofsError(false)
    void lookupSourceProvenance(source).then(
      (value) => {
        if (live) setStoredProofs(value)
      },
      () => {
        if (live) setStoredProofsError(true)
      }
    )
    return () => {
      live = false
    }
  }, [lookupSourceProvenance, source])
  const savedVersions = useMemo(() => {
    const paths = new Map<string, WorkbenchProof>()
    for (const proof of storedProofs) {
      const version = proof.artifacts[0]
      const current = paths.get(version.path)
      if (!current || current.artifacts[0].ordinal < version.ordinal) paths.set(version.path, proof)
    }
    return [...paths.values()]
  }, [storedProofs])
  const [showProof, setShowProof] = useState(false)
  const [tracing, setTracing] = useState(false)
  const [traceError, setTraceError] = useState(false)
  const lineageTransport = useMemo(() => (proof ? provenanceTransport(proof) : null), [proof, provenanceTransport])
  const deliveryRequest = useRef(0)
  const deliveryAbort = useRef<AbortController | null>(null)
  const traceRequest = useRef(0)
  useEffect(() => {
    deliveryRequest.current += 1
    deliveryAbort.current?.abort()
    traceRequest.current += 1
    setSaveFile(null)
    setSavedArtifact(null)
    setProof(null)
    setShowProof(false)
    setTracing(false)
    setTraceError(false)
    setBusy('')
    setSaved('')
    return () => {
      deliveryRequest.current += 1
      deliveryAbort.current?.abort()
      traceRequest.current += 1
    }
  }, [source.root, source.path, source.revision, source.digest, jobId])
  const [saved, setSaved] = useState('')
  const [tick, setTick] = useState(0)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState('')
  const [artifacts, setArtifacts] = useState<z.infer<typeof artifactSchema>['items']>([])
  const [question, setQuestion] = useState(initialQuestion)
  const [answers, setAnswers] = useState<{ id: string; question: string; answer: Answer }[]>([])
  const [asking, setAsking] = useState(false)
  const [chatOpen, setChatOpen] = useState(!!initialQuestion)
  const [search, setSearch] = useState('')
  const [hits, setHits] = useState<z.infer<typeof searchSchema>['hits']>([])
  const [markdown, setMarkdown] = useState<string | null>(null)
  const askAbort = useRef<AbortController | null>(null)
  const searchAbort = useRef<AbortController | null>(null)
  const askRequest = useRef<{ question: string; user: string; assistant: string } | null>(null)
  const latestConversation = answers.at(-1)?.answer.conversation_id
  useEffect(() => {
    setPaper(initial)
    setJobId((current) => current || initial.active_job_id || '')
  }, [initial])
  useEffect(() => {
    setQuestion(initialQuestion)
    if (initialQuestion) setChatOpen(true)
  }, [initialQuestion])
  const selectedJob = jobs.find((job) => job.job_id === jobId)
  const hasTranslation = selectedJob?.output_pdf_ready === true
  useEffect(() => {
    const abort = new AbortController()
    let timeout: ReturnType<typeof setTimeout> | undefined
    if (!serviceReady) return () => abort.abort()
    const refresh = async () => {
      try {
        const data = await paperloom(`documents/${paper.document_id}/jobs?limit=100`, jobsSchema, {
          signal: abort.signal
        })
        if (abort.signal.aborted) return
        setJobs(data.items)
        if (data.items.some((job) => job.status === 'queued' || job.status === 'running'))
          timeout = setTimeout(() => void refresh(), 2000)
      } catch {
        if (!abort.signal.aborted) setError('tasks_failed')
      }
    }
    void refresh()
    return () => {
      abort.abort()
      clearTimeout(timeout)
    }
  }, [paper.document_id, serviceReady, tick, paperloom])
  useEffect(() => {
    if (!jobId) {
      setArtifacts([])
      return
    }
    const abort = new AbortController()
    setArtifacts([])
    void paperloom(`jobs/${jobId}/artifacts-manifest`, artifactSchema, { signal: abort.signal })
      .then((data) => {
        if (!abort.signal.aborted && data.job_id === jobId)
          setArtifacts(data.items.filter((item) => item.artifact_kind === 'file'))
      })
      .catch(() => {
        if (!abort.signal.aborted) setError('artifacts_failed')
      })
    return () => abort.abort()
  }, [jobId, selectedJob?.status, selectedJob?.attempt, tick, paperloom])
  useEffect(() => {
    if (view !== 'markdown' || !jobId || !selectedJob?.markdown_ready) {
      setMarkdown(null)
      return
    }
    const abort = new AbortController()
    setMarkdown(null)
    void paperloomResponse(`jobs/${jobId}/markdown?raw=true`, { signal: abort.signal })
      .then((response) => response.text())
      .then((text) => {
        if (!abort.signal.aborted) {
          setMarkdown(text)
          setError((current) => (current === 'markdown_failed' ? '' : current))
        }
      })
      .catch(() => {
        if (!abort.signal.aborted) setError('markdown_failed')
      })
    return () => abort.abort()
  }, [view, jobId, selectedJob?.attempt, selectedJob?.status, selectedJob?.markdown_ready, tick, paperloomResponse])
  useEffect(
    () => () => {
      askAbort.current?.abort()
      searchAbort.current?.abort()
    },
    []
  )
  const setReadingStatus = async (value: string) => {
    if (busy) return
    setBusy('status')
    setError('')
    try {
      setPaper(
        await paperloom(`documents/${paper.document_id}`, documentSchema, jsonBody({ reading_status: value }, 'PATCH'))
      )
    } catch {
      setError('metadata_failed')
    } finally {
      setBusy('')
    }
  }
  const jobAction = async (job: PaperJob, action: 'cancel' | 'resume') => {
    if (busy || !serviceReady) return
    setBusy(job.job_id)
    setError('')
    try {
      if (action === 'resume') await verifySource()
      await paperloom(`jobs/${job.job_id}/${action}`, z.unknown(), jsonBody({}))
      setTick((value) => value + 1)
    } catch (reason) {
      setError(
        reason instanceof PaperloomError && reason.code === 'source_changed' ? 'source_changed' : 'task_action_failed'
      )
    } finally {
      setBusy('')
    }
  }
  const ask = async () => {
    if (asking || !serviceReady || !question.trim()) return
    const input = question.trim()
    if (askRequest.current?.question !== input)
      askRequest.current = {
        question: input,
        user: crypto.randomUUID(),
        assistant: crypto.randomUUID()
      }
    const request = askRequest.current
    const abort = new AbortController()
    askAbort.current = abort
    setAsking(true)
    setError('')
    try {
      await verifySource()
      if (abort.signal.aborted) return
      const answer = await paperloom('ai/ask', answerSchema, {
        ...jsonBody({
          question: input,
          document_id: paper.document_id,
          ...(jobId ? { job_id: jobId } : {}),
          ...(latestConversation ? { conversation_id: latestConversation } : {}),
          user_message_id: request.user,
          assistant_message_id: request.assistant,
          assistant_mode: 'reading',
          stream: false
        }),
        signal: abort.signal
      })
      if (abort.signal.aborted) return
      setAnswers((current) => [...current, { id: request.assistant, question: input, answer }])
      setQuestion('')
      askRequest.current = null
    } catch (reason) {
      if (!abort.signal.aborted)
        setError(reason instanceof PaperloomError && reason.code === 'source_changed' ? 'source_changed' : 'ask_failed')
    } finally {
      if (askAbort.current === abort) {
        setAsking(false)
        askAbort.current = null
      }
    }
  }
  const find = async () => {
    searchAbort.current?.abort()
    if (!serviceReady || !search.trim()) {
      setHits([])
      return
    }
    const abort = new AbortController()
    searchAbort.current = abort
    setError('')
    try {
      const data = await paperloom(
        `search?${new URLSearchParams({ q: search, document_id: paper.document_id, limit: '50' })}`,
        searchSchema,
        { signal: abort.signal }
      )
      if (!abort.signal.aborted) setHits(data.hits)
    } catch {
      if (!abort.signal.aborted) setError('search_failed')
    }
  }
  const followCitation = (citation: z.infer<typeof citationSchema>) => {
    if (citation.document_id !== paper.document_id || citation.page_idx >= paper.page_count || !citation.block_id) {
      setError('citation_unavailable')
      return
    }
    setPage(citation.page_idx + 1)
    setView('original')
    setTab('document')
  }
  const deliverArtifact = async (
    key: string,
    name: string,
    checksum: string | null | undefined,
    destination: 'workspace' | 'copy',
    attempt: number
  ) => {
    if (busy || !serviceReady) return
    const request = ++deliveryRequest.current
    deliveryAbort.current?.abort()
    const abort = new AbortController()
    deliveryAbort.current = abort
    setBusy(key)
    setError('')
    setSaved('')
    try {
      await verifySource()
      if (request !== deliveryRequest.current) return
      const path = `jobs/${jobId}/artifacts/${encodeURIComponent(key)}`
      if (destination === 'workspace') {
        if (!checksum) throw new PaperloomError(409, 'artifact_unverified')
        const bytes = await paperloomArtifactBytes(path, name, checksum, abort.signal)
        await verifySource()
        if (request === deliveryRequest.current)
          setSaveFile({
            file: new File([bytes.slice()], name, { type: 'application/octet-stream' }),
            jobId,
            key,
            checksum,
            attempt
          })
      } else {
        const bytes = await paperloomArtifactBytes(path, name, checksum, abort.signal)
        await verifySource()
        if (request === deliveryRequest.current) await window.api.file.save(name, bytes)
      }
    } catch (reason) {
      if (request !== deliveryRequest.current) return
      setError(
        reason instanceof PaperloomError && ['source_changed', 'artifact_unverified'].includes(reason.code)
          ? reason.code
          : 'download_failed'
      )
    } finally {
      if (request === deliveryRequest.current) setBusy('')
    }
  }
  const registerSaved = async (value: NonNullable<typeof savedArtifact>) => {
    const request = ++traceRequest.current
    setSavedArtifact(value)
    setTracing(true)
    setTraceError(false)
    try {
      const result = await registerSavedArtifact(source, value.path, value.artifact)
      if (request === traceRequest.current) {
        setProof(result)
        setStoredProofs((current) => [...current.filter((item) => item.messageId !== result.messageId), result])
      }
    } catch {
      if (request === traceRequest.current) setTraceError(true)
    } finally {
      if (request === traceRequest.current) setTracing(false)
    }
  }
  if (showProof && proof && lineageTransport)
    return (
      <div className="flex min-h-0 flex-1 flex-col overflow-auto" data-paperloom-lineage="">
        <div className="shrink-0 border-b border-[#efefef] p-2">
          <Button variant="ghost" size="sm" onClick={() => setShowProof(false)}>
            <ArrowLeft size={16} strokeWidth={1.7} />
            {t('paperloom.back_to_reader')}
          </Button>
        </div>
        <WorkbenchLineagePane
          lineage={{ proof, artifactId: proof.artifacts[0].id, review: false, requestId: proof.messageId }}
          transport={lineageTransport}
        />
      </div>
    )
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-paperloom-reader={paper.document_id}>
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[#efefef] px-3 py-2">
        <Button variant="ghost" size="icon-sm" aria-label={t('paperloom.change_source')} onClick={onBack}>
          <ArrowLeft size={16} strokeWidth={1.7} />
        </Button>
        <h2 className="min-w-24 flex-1 truncate text-[15px] font-medium text-[#171717]" title={paper.title}>
          {paper.title}
        </h2>
        <SearchSelect
          aria-label={t('paperloom.reading_status')}
          size="sm"
          value={paper.reading_status}
          options={['unread', 'reading', 'done'].map((value) => ({
            value,
            label: t(`paperloom.${value}`)
          }))}
          onChange={(value) => void setReadingStatus(value)}
          searchable={false}
          allowClear={false}
          disabled={!!busy || !serviceReady}
        />
        <Button size="md" disabled={!serviceReady || paper.page_count < 1} onClick={() => setProcess(true)}>
          <Play size={16} strokeWidth={1.7} />
          {t('paperloom.process_paper')}
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-pressed={chatOpen}
          aria-label={t('paperloom.paper_qa')}
          onClick={() => setChatOpen((value) => !value)}>
          <MessageSquare size={16} strokeWidth={1.7} />
        </Button>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[#efefef] px-3 py-1.5">
        <SegmentedControl
          aria-label={t('paperloom.paper_tools')}
          size="sm"
          value={tab}
          options={(['document', 'tasks', 'artifacts'] as const).map((value) => ({
            value,
            label: t(`paperloom.tab_${value}`)
          }))}
          onValueChange={setTab}
        />
        {jobs.length > 0 && (
          <SearchSelect
            aria-label={t('paperloom.processing_version')}
            size="sm"
            className="max-w-56 min-w-0"
            value={jobId}
            options={[
              { value: '', label: t('paperloom.original_source') },
              ...jobs.map((job) => ({
                value: job.job_id,
                label: `${job.job_id} · ${t(`paperloom.job_${job.status}`)}`
              }))
            ]}
            onChange={(value) => {
              setJobId(value)
              setView('original')
            }}
            allowClear={false}
          />
        )}
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t('paperloom.refresh')}
          onClick={() => {
            setError('')
            setTick((value) => value + 1)
          }}>
          <RefreshCw size={16} strokeWidth={1.7} />
        </Button>
      </div>
      {error && (
        <p role="alert" className="shrink-0 px-4 py-2 text-[#b42318]">
          {t(`paperloom.${error}`)}
        </p>
      )}
      <div className="@container/reader flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex min-h-0 flex-1 flex-col @min-[760px]/reader:flex-row">
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            {tab === 'document' ? (
              <>
                <div className="flex shrink-0 flex-wrap items-center gap-1 border-b border-[#efefef] p-2">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t('paperloom.previous_page')}
                    disabled={page <= 1}
                    onClick={() => setPage((value) => value - 1)}>
                    <ChevronLeft size={16} strokeWidth={1.7} />
                  </Button>
                  <input
                    type="number"
                    min={1}
                    max={pages}
                    aria-label={t('paperloom.page_number')}
                    value={page}
                    className="h-[30px] w-14 rounded-[8px] bg-[#f5f5f5] px-2 text-center outline-none focus-visible:ring-2 focus-visible:ring-black/10"
                    onChange={(event) => {
                      const value = Number(event.target.value)
                      if (Number.isInteger(value) && value >= 1 && value <= pages) setPage(value)
                    }}
                  />
                  <span className="px-1 text-[12px] text-[#8a8a8a]">/ {pages}</span>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t('paperloom.next_page')}
                    disabled={page >= pages}
                    onClick={() => setPage((value) => value + 1)}>
                    <ChevronRight size={16} strokeWidth={1.7} />
                  </Button>
                  <SearchSelect
                    aria-label={t('paperloom.zoom')}
                    size="sm"
                    value={zoom}
                    options={[
                      { value: '1', label: t('paperloom.fit_width') },
                      { value: '1.25', label: '125%' },
                      { value: '1.5', label: '150%' },
                      { value: '2', label: '200%' }
                    ]}
                    onChange={setZoom}
                    searchable={false}
                    allowClear={false}
                  />
                  <SearchSelect
                    aria-label={t('paperloom.reading_view')}
                    size="sm"
                    value={view}
                    options={[
                      { value: 'original', label: t('paperloom.original') },
                      ...(hasTranslation
                        ? [
                            { value: 'translated', label: t('paperloom.translated') },
                            { value: 'comparison', label: t('paperloom.comparison') }
                          ]
                        : []),
                      ...(selectedJob?.markdown_ready ? [{ value: 'markdown', label: 'Markdown' }] : [])
                    ]}
                    onChange={setView}
                    searchable={false}
                    allowClear={false}
                  />
                  <div className="min-w-36 flex-1">
                    <SearchInput
                      aria-label={t('paperloom.search_content')}
                      placeholder={t('paperloom.search_content')}
                      value={search}
                      onChange={(event) => setSearch(event.target.value)}
                      onClear={() => {
                        searchAbort.current?.abort()
                        setSearch('')
                        setHits([])
                      }}
                      clearLabel={t('paperloom.clear')}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' && !event.nativeEvent.isComposing) void find()
                      }}
                    />
                  </div>
                </div>
                {hits.length > 0 && (
                  <div
                    className="flex max-h-32 shrink-0 flex-col gap-0.5 overflow-y-auto border-b border-[#efefef] px-2 py-1"
                    aria-label={t('paperloom.search_results')}>
                    {hits.map((hit, index) => (
                      <button
                        key={`${hit.block_id}:${index}`}
                        type="button"
                        className="block h-9 w-full truncate rounded-[8px] px-2.5 text-left text-[13px] text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]"
                        onClick={() => followCitation({ ...hit, ref: index + 1, snippet: hit.source_snippet })}>
                        {t('paperloom.citation_page', { page: hit.page_idx + 1 })}{' '}
                        {hit.source_snippet || hit.translated_snippet}
                      </button>
                    ))}
                  </div>
                )}
                <div className="flex min-h-0 flex-1">
                  {view === 'markdown' ? (
                    <pre className="min-h-0 min-w-0 flex-1 overflow-auto p-4 font-sans text-[13px] leading-6 whitespace-pre-wrap text-[#171717]">
                      {markdown ?? (error === 'markdown_failed' ? null : t('paperloom.loading'))}
                    </pre>
                  ) : (
                    <div
                      className={`flex min-h-0 min-w-0 flex-1 ${view === 'comparison' ? 'flex-col @min-[600px]/reader:flex-row' : ''}`}>
                      {(view === 'original' || view === 'comparison') && (
                        <PaperloomPdf
                          path={`documents/${paper.document_id}/source.pdf`}
                          page={page}
                          zoom={Number(zoom)}
                          label={t('paperloom.original')}
                          caption={view === 'comparison'}
                          source={source.bytes}
                          onPages={setPages}
                          onReady={onSourceReady}
                        />
                      )}
                      {(view === 'translated' || view === 'comparison') && hasTranslation && (
                        <PaperloomPdf
                          key={`${jobId}:${selectedJob?.attempt}`}
                          path={`jobs/${jobId}/pdf`}
                          page={page}
                          zoom={Number(zoom)}
                          label={t('paperloom.translated')}
                          caption={view === 'comparison'}
                        />
                      )}
                    </div>
                  )}
                </div>
              </>
            ) : tab === 'tasks' ? (
              <div className="min-h-0 flex-1 overflow-y-auto px-4">
                {jobs.length === 0 && <p className="py-6 text-[13px] text-[#8a8a8a]">{t('paperloom.no_tasks')}</p>}
                {jobs.map((job) => (
                  <section key={job.job_id} className="border-b border-[#efefef] py-3" data-paperloom-job={job.job_id}>
                    <div className="mb-2 flex flex-wrap items-center gap-2">
                      <h3 className="min-w-0 flex-1 truncate text-[13px] font-medium text-[#171717]">
                        {t(`paperloom.workflow_${job.workflow}`)}
                      </h3>
                      <span className="text-[12px] text-[#8a8a8a]">{t(`paperloom.job_${job.status}`)}</span>
                      {['queued', 'running'].includes(job.status) ? (
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={!!busy}
                          onClick={() => void jobAction(job, 'cancel')}>
                          <Square size={16} strokeWidth={1.7} />
                          {t('paperloom.stop')}
                        </Button>
                      ) : ['failed', 'canceled'].includes(job.status) ? (
                        <Button
                          variant="secondary"
                          size="sm"
                          disabled={!!busy}
                          onClick={() => void jobAction(job, 'resume')}>
                          {t('paperloom.resume')}
                        </Button>
                      ) : null}
                    </div>
                    <p className="mb-2 text-[12px] text-[#8a8a8a]">
                      {job.job_id} {job.attempt ? t('paperloom.attempt', { count: job.attempt }) : ''}
                    </p>
                    <div className="grid gap-3">
                      {(['ocr', 'translation', 'render'] as const).map((stage) => (
                        <div key={stage} className="grid grid-cols-[70px_1fr_auto] items-center gap-3 text-[12px]">
                          <span>{t(`paperloom.stage_${stage}`)}</span>
                          {job.stages[stage].progress.percent != null ? (
                            <progress
                              className="h-1 w-full accent-[#171717]"
                              max={100}
                              value={Math.max(0, Math.min(100, job.stages[stage].progress.percent))}
                              aria-label={t(`paperloom.stage_${stage}`)}
                            />
                          ) : (
                            <span className="text-[#8a8a8a]">
                              {t(`paperloom.stage_state_${job.stages[stage].state}`)}
                            </span>
                          )}
                          <span className="text-[#8a8a8a]">
                            {job.stages[stage].progress.current != null && job.stages[stage].progress.total != null
                              ? `${job.stages[stage].progress.current}/${job.stages[stage].progress.total}`
                              : ''}
                          </span>
                        </div>
                      ))}
                    </div>
                  </section>
                ))}
              </div>
            ) : (
              <div className="min-h-0 flex-1 overflow-y-auto">
                {storedProofsError && (
                  <p role="alert" className="px-4 py-6 text-[13px] text-[#b42318]">
                    {t('paperloom.provenance_load_failed')}
                  </p>
                )}
                {savedVersions.map((saved) => {
                  const version = saved.artifacts[0]
                  const path = version.path.startsWith(`${source.root}/`)
                    ? version.path.slice(source.root.length + 1)
                    : version.path
                  return (
                    <div
                      key={version.id}
                      className="flex min-h-12 items-center gap-2 border-b border-[#efefef] px-4 hover:bg-[#f7f7f7]">
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-[13px] text-[#1a1a1a]" title={version.path}>
                          {path}
                        </p>
                        <p className="text-[12px] text-[#8a8a8a]">
                          {t('paperloom.provenance_saved_copy')} · v{version.ordinal}
                        </p>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={t('paperloom.open_saved_provenance', { name: path })}
                        onClick={() => {
                          setProof(saved)
                          setShowProof(true)
                        }}>
                        {t('paperloom.view_provenance')}
                      </Button>
                    </div>
                  )
                })}
                {!jobId
                  ? savedVersions.length === 0 && (
                      <p className="px-4 py-6 text-[13px] text-[#8a8a8a]">{t('paperloom.choose_version')}</p>
                    )
                  : artifacts.length === 0
                    ? savedVersions.length === 0 && (
                        <p className="px-4 py-6 text-[13px] text-[#8a8a8a]">{t('paperloom.no_artifacts')}</p>
                      )
                    : artifacts.map((item) => (
                        <div
                          key={item.artifact_key}
                          role="group"
                          aria-label={item.file_name || item.artifact_key}
                          className="flex min-h-12 items-center gap-2 border-b border-[#efefef] px-4 hover:bg-[#f7f7f7]">
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-[13px] text-[#1a1a1a]">
                              {item.file_name || item.artifact_kind}
                            </p>
                            <p className="truncate text-[12px] text-[#8a8a8a]" title={item.checksum ?? undefined}>
                              {t('paperloom.attempt', { count: item.attempt })}
                              {item.checksum ? ` · ${item.checksum}` : ''}
                              {item.ready && !item.checksum ? ` · ${t('paperloom.artifact_unverified')}` : ''}
                            </p>
                          </div>
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={!item.ready || !item.checksum || !!busy}
                            onClick={() =>
                              void deliverArtifact(
                                item.artifact_key,
                                item.file_name || item.artifact_key,
                                item.checksum,
                                'workspace',
                                item.attempt
                              )
                            }>
                            <FolderInput size={16} strokeWidth={1.7} />
                            {t('paperloom.save_workspace')}
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={!item.ready || !item.checksum || !!busy}
                            onClick={() =>
                              void deliverArtifact(
                                item.artifact_key,
                                item.file_name || item.artifact_key,
                                item.checksum,
                                'copy',
                                item.attempt
                              )
                            }>
                            <Download size={16} strokeWidth={1.7} />
                            {t(item.ready ? 'paperloom.download' : 'paperloom.artifact_pending')}
                          </Button>
                        </div>
                      ))}
              </div>
            )}
          </div>
          {chatOpen && (
            <aside
              aria-label={t('paperloom.paper_qa')}
              className="flex max-h-[50%] min-h-48 min-w-0 shrink-0 flex-col border-t border-[#efefef] bg-white @min-[760px]/reader:max-h-none @min-[760px]/reader:w-80 @min-[760px]/reader:border-t-0 @min-[760px]/reader:border-l">
              <header className="flex h-9 shrink-0 items-center px-3">
                <h3 className="flex-1 text-[13px] font-medium text-[#171717]">{t('paperloom.paper_qa')}</h3>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t('paperloom.close_qa')}
                  onClick={() => setChatOpen(false)}>
                  <X size={16} strokeWidth={1.7} />
                </Button>
              </header>
              <div className="min-h-0 flex-1 overflow-y-auto px-3" aria-live="polite">
                {answers.length === 0 && (
                  <p className="py-3 text-[12px] leading-5 text-[#8a8a8a]">{t('paperloom.qa_hint')}</p>
                )}
                {answers.map((item) => (
                  <article key={item.id} className="mb-5">
                    <p className="mb-3 rounded-[8px] bg-[#f5f5f5] px-3 py-2 whitespace-pre-wrap">{item.question}</p>
                    <p className="text-[13px] leading-6 whitespace-pre-wrap text-[#171717]">{item.answer.answer}</p>
                    {!item.answer.persisted && (
                      <p role="alert" className="mt-2 text-[12px] text-[#b42318]">
                        {t('paperloom.answer_not_saved')}
                      </p>
                    )}
                    <div className="mt-2 flex flex-wrap gap-1">
                      {item.answer.citations.map((citation) => (
                        <Button
                          key={citation.ref}
                          variant="secondary"
                          size="sm"
                          title={citation.snippet}
                          onClick={() => followCitation(citation)}>
                          [{citation.ref}] {t('paperloom.citation_page', { page: citation.page_idx + 1 })}
                        </Button>
                      ))}
                    </div>
                  </article>
                ))}
                {asking && (
                  <p role="status" className="py-3 text-[#8a8a8a]">
                    {t('paperloom.asking')}
                  </p>
                )}
              </div>
              <form
                className="m-3 grid shrink-0 gap-2 rounded-[14px] border border-[#ececec] bg-white p-2 shadow-[0_8px_22px_rgba(20,40,80,0.04)]"
                onSubmit={(event) => {
                  event.preventDefault()
                  void ask()
                }}>
                <textarea
                  aria-label={t('paperloom.question')}
                  className="min-h-16 resize-y border-0 p-1 text-[13px] outline-none"
                  placeholder={t('paperloom.question_hint')}
                  maxLength={16000}
                  value={question}
                  disabled={asking}
                  onChange={(event) => setQuestion(event.target.value)}
                />
                <div className="flex justify-end">
                  {asking ? (
                    <Button type="button" variant="secondary" size="sm" onClick={() => askAbort.current?.abort()}>
                      {t('paperloom.stop')}
                    </Button>
                  ) : (
                    <Button type="submit" size="sm" disabled={!serviceReady || !question.trim()}>
                      {t('paperloom.ask')}
                    </Button>
                  )}
                </div>
              </form>
            </aside>
          )}
        </div>
      </div>
      {saved && (
        <p role="status" className="shrink-0 px-4 py-2 text-[12px] text-[#8a8a8a]">
          {t('paperloom.saved_workspace', { location: saved })}
        </p>
      )}
      {savedArtifact && (
        <div className="flex shrink-0 flex-wrap items-center gap-2 px-4 py-2 text-[12px]">
          {tracing ? (
            <p role="status">{t('paperloom.provenance_verifying')}</p>
          ) : proof ? (
            <Button variant="ghost" size="sm" onClick={() => setShowProof(true)}>
              {t('paperloom.view_provenance')}
            </Button>
          ) : traceError ? (
            <>
              <p role="alert" className="text-[#b42318]">
                {t('paperloom.provenance_failed')}
              </p>
              <Button variant="ghost" size="sm" onClick={() => void registerSaved(savedArtifact)}>
                {t('paperloom.retry')}
              </Button>
            </>
          ) : null}
        </div>
      )}
      {saveFile && (
        <SaveDocumentDialog
          root={source.root}
          file={saveFile.file}
          beforeSave={async () => {
            try {
              await verifySource()
            } catch (reason) {
              setSaveFile(null)
              setError(
                reason instanceof PaperloomError && reason.code === 'source_changed'
                  ? 'source_changed'
                  : 'workspace_source_failed'
              )
              throw reason
            }
          }}
          destination={{
            title: t('paperloom.save_workspace'),
            description: t('paperloom.save_workspace_hint', { name: saveFile.file.name }),
            rootLabel: source.root
          }}
          onSaved={(location, path) => {
            setSaved(location)
            void registerSaved({ path, artifact: saveFile })
          }}
          onClose={() => setSaveFile(null)}
        />
      )}
      <PaperloomProcessDialog
        key={paper.page_count}
        open={process}
        verifySource={verifySource}
        onOpenChange={setProcess}
        paper={paper}
        onSubmitted={(jobId) => {
          setJobId(jobId)
          setTab('tasks')
          setTick((value) => value + 1)
        }}
      />
    </div>
  )
}
