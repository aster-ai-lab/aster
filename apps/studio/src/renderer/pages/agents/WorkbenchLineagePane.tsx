import { ChevronLeft, ChevronRight, Copy } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePartsMap } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import { loadWorkbenchArtifact, loadWorkbenchProofs } from '@renderer/data/studioAgentPlane'
import { proofFromParts, type WorkbenchProof, type WorkbenchReviewFinding } from '@shared/types/workbenchProof'

import { TaskAutoReview } from './taskConversation/TaskAutoReview'
import { useOptionalWorkbenchWorkspaceTabs, type WorkbenchWorkspaceTab } from './workbenchWorkspaceTabs'

const TABS = ['artifact', 'evidence', 'conversation', 'execution', 'code', 'environment', 'review'] as const
const SERVICE_TABS = ['artifact', 'evidence', 'source', 'execution', 'environment'] as const
const TAB_LABELS = {
  source: 'paperloom.provenance_source',
  artifact: 'workbench.proof.tab_artifact',
  evidence: 'workbench.proof.tab_evidence',
  conversation: 'workbench.proof.tab_conversation',
  execution: 'workbench.proof.tab_execution',
  code: 'workbench.proof.tab_code',
  environment: 'workbench.proof.tab_environment',
  review: 'workbench.proof.tab_review'
} as const
type Tab = (typeof TABS)[number] | 'source'

export interface WorkbenchLineageTransport {
  loadProofs: (sessionId: string) => Promise<WorkbenchProof[]>
  loadArtifact: (sessionId: string, artifactId: string, signal: AbortSignal) => ReturnType<typeof loadWorkbenchArtifact>
}
const AGENT_TRANSPORT: WorkbenchLineageTransport = {
  loadProofs: loadWorkbenchProofs,
  loadArtifact: loadWorkbenchArtifact
}

export function WorkbenchLineagePane({
  lineage,
  transport = AGENT_TRANSPORT
}: {
  lineage: NonNullable<WorkbenchWorkspaceTab['lineage']>
  transport?: WorkbenchLineageTransport
}) {
  const { t } = useTranslation()
  const workspace = useOptionalWorkbenchWorkspaceTabs()
  const id = useId()
  const tablist = useRef<HTMLDivElement>(null)
  const copyRequest = useRef(0)
  const parts = usePartsMap()
  const [history, setHistory] = useState<WorkbenchProof[]>([])
  const [historyError, setHistoryError] = useState('')
  const [selectedId, setSelectedId] = useState(lineage.artifactId)
  const serviceOrigin = lineage.proof.artifacts.find((item) => item.id === lineage.artifactId)?.origin
  const tabs: readonly Tab[] = serviceOrigin ? SERVICE_TABS : TABS
  const [tab, setTab] = useState<Tab>(lineage.review ? 'review' : 'artifact')
  const [snapshot, setSnapshot] = useState<{ url: string; text?: string } | null>(null)
  const [error, setError] = useState('')
  const [copyStatus, setCopyStatus] = useState('')
  const [confirmed, setConfirmed] = useState(new Map<string, Map<string, WorkbenchReviewFinding['status']>>())
  useEffect(() => {
    setSelectedId(lineage.artifactId)
    setTab(lineage.review ? 'review' : 'artifact')
  }, [lineage.artifactId, lineage.review, lineage.requestId])
  useEffect(() => {
    let disposed = false
    transport.loadProofs(lineage.proof.sessionId).then(
      (value) => {
        if (!disposed) {
          setHistory(value)
          setHistoryError('')
        }
      },
      () => {
        if (!disposed) setHistoryError(t('workbench.proof.history_failed'))
      }
    )
    return () => {
      disposed = true
    }
  }, [lineage.proof, transport, t])
  const proofs = useMemo(() => {
    const map = new Map(history.map((proof) => [proof.messageId, proof]))
    for (const [messageId, values] of Object.entries(parts ?? {})) {
      const proof = proofFromParts(values, messageId)
      if (proof?.sessionId === lineage.proof.sessionId) map.set(messageId, proof)
    }
    map.set(lineage.proof.messageId, lineage.proof)
    return Array.from(map.values()).map((proof) => {
      const dispositions = confirmed.get(proof.messageId)
      if (!dispositions) return proof
      const findings = proof.review.findings.map((finding) => {
        const status = dispositions.get(finding.id)
        return status === 'accepted' || status === 'waived' ? { ...finding, status } : finding
      })
      const status =
        proof.review.status === 'open_queue' &&
        findings.every((finding) => finding.status === 'accepted' || finding.status === 'waived')
          ? ('completed' as const)
          : proof.review.status
      return { ...proof, review: { ...proof.review, findings, status } }
    })
  }, [confirmed, history, lineage.proof, parts])
  const path = lineage.proof.artifacts.find((version) => version.id === lineage.artifactId)?.path
  const versions = proofs
    .flatMap((proof) =>
      proof.artifacts.filter((version) => version.path === path).map((version) => ({ version, proof }))
    )
    .sort((a, b) => a.version.ordinal - b.version.ordinal)
  const index = versions.findIndex((item) => item.version.id === selectedId)
  const focused = versions[index]
  const version = focused?.version
  const proof = focused?.proof
  const { id: versionId, contentDigest, bytes: versionBytes, mediaType } = version ?? {}
  useEffect(() => {
    copyRequest.current += 1
    setSnapshot(null)
    setError('')
    setCopyStatus('')
    if (!versionId) return
    const controller = new AbortController()
    let url: string | undefined
    transport
      .loadArtifact(lineage.proof.sessionId, versionId, controller.signal)
      .then(async (result) => {
        const bytes = Uint8Array.from(atob(result.data), (char) => char.charCodeAt(0))
        const digest = `sha256:${Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), (value) => value.toString(16).padStart(2, '0')).join('')}`
        if (result.contentDigest !== contentDigest || digest !== contentDigest || bytes.length !== versionBytes)
          throw new Error(t('workbench.proof.digest_mismatch'))
        if (controller.signal.aborted) return
        url = URL.createObjectURL(new Blob([bytes], { type: mediaType }))
        let text: string | undefined
        try {
          if (mediaType?.startsWith('text/') || mediaType === 'application/json')
            text = new TextDecoder('utf-8', { fatal: true }).decode(bytes)
        } catch {
          /* Binary snapshots remain downloadable. */
        }
        setSnapshot({ url, text: result.text ?? text })
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : t('workbench.proof.snapshot_failed'))
      })
    return () => {
      copyRequest.current += 1
      controller.abort()
      if (url) URL.revokeObjectURL(url)
    }
  }, [lineage.proof.sessionId, versionId, contentDigest, versionBytes, mediaType, transport, t])
  if (!version || !proof) return <p role="alert">{t('workbench.proof.snapshot_failed')}</p>
  const copy = async (value: string) => {
    const request = ++copyRequest.current
    setCopyStatus('')
    try {
      await navigator.clipboard.writeText(value)
      if (request === copyRequest.current) setCopyStatus(t('workbench.proof.copied'))
    } catch {
      if (request === copyRequest.current) setCopyStatus(t('workbench.proof.copy_failed'))
    }
  }
  const tool = proof.tools.find((item) => item.id === version.toolCallId)
  const reviewProofs = proofs
    .filter((item) => item.policy.reviewMode)
    .map((item) => ({
      ...item,
      review: {
        ...item.review,
        findings: item.review.findings.filter((finding) => finding.receiptDigest === version.receiptDigest)
      }
    }))
    .filter((item) => item.review.findings.length)
  return (
    <section
      className="studio-lineage"
      data-task-lineage=""
      data-task-lineage-status={error ? 'failed' : version.status}
      aria-label={t('workbench.lineage.title')}>
      <header>
        <strong>{version.name}</strong>
        <span>
          {
            {
              traced: t('workbench.proof.trace_traced'),
              incomplete: t('workbench.proof.trace_incomplete'),
              failed: t('workbench.proof.trace_failed')
            }[error ? 'failed' : version.status]
          }
        </span>
      </header>
      {version.origin ? (
        <p role="status" className="shrink-0 px-3 pb-3 text-[12px] leading-5 text-[#8a8a8a]">
          {t('paperloom.provenance_authority')}
        </p>
      ) : null}
      {version.status === 'incomplete' ? <p role="status">{t('workbench.proof.incomplete_notice')}</p> : null}
      <div className="studio-lineage-version">
        <button
          type="button"
          aria-label={t('workbench.proof.previous_version')}
          disabled={index <= 0}
          onClick={() => setSelectedId(versions[index - 1].version.id)}>
          <ChevronLeft size={16} strokeWidth={1.7} />
        </button>
        <span>
          {t('workbench.proof.version')} v{version.ordinal}
        </span>
        <button
          type="button"
          aria-label={t('workbench.proof.next_version')}
          disabled={index === versions.length - 1}
          onClick={() => setSelectedId(versions[index + 1].version.id)}>
          <ChevronRight size={16} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          aria-label={t('workbench.proof.copy_digest')}
          title={version.contentDigest}
          onClick={() => void copy(version.contentDigest)}>
          <span>SHA-256</span>
          <code>{version.contentDigest.slice(7, 19)}</code>
          <Copy size={16} strokeWidth={1.7} />
        </button>
        <span role="status">{copyStatus}</span>
      </div>
      {historyError ? <p role="alert">{historyError}</p> : null}
      <div
        role="tablist"
        ref={tablist}
        aria-label={t('workbench.lineage.title')}
        onKeyDown={(event) => {
          let next = tabs.indexOf(tab)
          if (event.key === 'ArrowRight') next = (next + 1) % tabs.length
          else if (event.key === 'ArrowLeft') next = (next + tabs.length - 1) % tabs.length
          else if (event.key === 'Home') next = 0
          else if (event.key === 'End') next = tabs.length - 1
          else return
          event.preventDefault()
          setTab(tabs[next])
          tablist.current?.querySelectorAll<HTMLButtonElement>('[role=tab]')[next]?.focus()
        }}>
        {tabs.map((item) => (
          <button
            key={item}
            type="button"
            role="tab"
            id={`${id}-${item}`}
            aria-controls={`${id}-panel`}
            aria-selected={tab === item}
            tabIndex={tab === item ? 0 : -1}
            onClick={() => setTab(item)}>
            {t(TAB_LABELS[item])}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={`${id}-panel`} aria-labelledby={`${id}-${tab}`} tabIndex={0}>
        {tab === 'artifact' ? (
          <>
            <p>{version.path}</p>
            <p>
              {version.bytes.toLocaleString()} B · {version.mediaType}
            </p>
            {error ? (
              <p role="alert">{error}</p>
            ) : !snapshot ? (
              <p role="status">{t('workbench.proof.loading')}</p>
            ) : (
              <>
                <a href={snapshot.url} download={version.name}>
                  {t('workbench.proof.download_version')}
                </a>
                {['image/png', 'image/jpeg', 'image/svg+xml'].includes(version.mediaType) ? (
                  <img alt={version.name} src={snapshot.url} />
                ) : snapshot.text !== undefined ? (
                  <pre>{snapshot.text}</pre>
                ) : (
                  <p>{t('workbench.proof.binary_preview')}</p>
                )}
              </>
            )}
          </>
        ) : null}
        {tab === 'evidence' ? (
          <dl>
            {[
              ['content', version.contentDigest],
              ['receipt', version.receiptDigest],
              ['workflow', version.workflowDigest],
              ['code', version.codeDigest],
              ['environment', version.environmentDigest],
              ...version.inputDigests.map((value, i) => [`input ${i + 1}`, value])
            ].map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>
                  <code>{value}</code>
                  <button
                    type="button"
                    aria-label={`${t('workbench.proof.copy_digest')} ${label}`}
                    onClick={() => void copy(value)}>
                    <Copy size={16} strokeWidth={1.7} />
                  </button>
                </dd>
              </div>
            ))}
          </dl>
        ) : null}
        {tab === 'source' && version.origin ? (
          <dl>
            {[
              [t('paperloom.workspace_pdf_path'), version.origin.sourcePath],
              [t('paperloom.provenance_workspace'), version.origin.workspaceRoot],
              [t('paperloom.provenance_source_revision'), version.origin.sourceRevision],
              ['SHA-256', version.origin.sourceDigest],
              [t('paperloom.provenance_job'), version.origin.jobId],
              [
                t('paperloom.provenance_job_status'),
                {
                  queued: t('paperloom.job_queued'),
                  running: t('paperloom.job_running'),
                  succeeded: t('paperloom.job_succeeded'),
                  failed: t('paperloom.job_failed'),
                  canceled: t('paperloom.job_canceled')
                }[version.origin.jobStatus]
              ],
              [t('paperloom.attempt', { count: version.origin.attempt }), version.origin.artifactKey],
              [t('paperloom.provenance_manifest'), version.origin.manifestDigest],
              [t('paperloom.provenance_package'), version.origin.packageDigest]
            ].map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>
                  <code>{value}</code>
                </dd>
              </div>
            ))}
          </dl>
        ) : null}
        {tab === 'conversation' ? (
          <>
            <h3>{t('workbench.proof.request')}</h3>
            <pre>{proof.userText}</pre>
            <h3>{t('workbench.proof.original_reply')}</h3>
            <pre>{proof.sourceText}</pre>
          </>
        ) : null}
        {tab === 'execution' ? (
          <>
            <p>{t(version.origin ? 'paperloom.provenance_authority' : 'workbench.proof.execution_authority')}</p>
            <dl>
              {[
                ['run', proof.runId],
                ['execution', version.executionId],
                ['tool', version.toolName],
                ['tool call', version.toolCallId],
                ...(!version.origin ? [['model', proof.model]] : []),
                ['created', version.createdAt]
              ].map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
            {tool ? (
              <>
                <p>{tool.state}</p>
                <pre>{tool.input}</pre>
                <pre>{tool.output}</pre>
              </>
            ) : (
              <p>{t('workbench.proof.no_evidence')}</p>
            )}
          </>
        ) : null}
        {tab === 'code' ? (
          <>
            <p>{t(version.origin ? 'paperloom.provenance_authority' : 'workbench.proof.execution_authority')}</p>
            {tool?.input ? <pre>{tool.input}</pre> : <p>{t('workbench.proof.no_evidence')}</p>}
          </>
        ) : null}
        {tab === 'environment' ? (
          <>
            <pre>{version.environment}</pre>
            <code>{version.environmentDigest}</code>
          </>
        ) : null}
        {tab === 'review' ? (
          reviewProofs.length ? (
            reviewProofs.map((item) => (
              <TaskAutoReview
                key={item.messageId}
                proof={item}
                onChange={(updated) => {
                  setConfirmed((current) => {
                    const next = new Map(current)
                    next.set(
                      updated.messageId,
                      new Map(
                        updated.review.findings
                          .filter((finding) => finding.status === 'accepted' || finding.status === 'waived')
                          .map((finding) => [finding.id, finding.status])
                      )
                    )
                    return next
                  })
                  setHistory((current) => [...current.filter((item) => item.messageId !== updated.messageId), updated])
                  workspace?.syncProof(updated)
                }}
              />
            ))
          ) : (
            <p>{t('workbench.proof.no_version_findings')}</p>
          )
        ) : null}
      </div>
    </section>
  )
}
