import { createHash, webcrypto } from 'node:crypto'

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useEffect, useRef } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { PartsProvider } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import { actOnWorkbenchProof, loadWorkbenchArtifact, loadWorkbenchProofs } from '@renderer/data/studioAgentPlane'
import { makeWorkbenchProof } from '@shared/types/__tests__/workbenchProof.fixture'

import { WorkbenchLineagePane } from '../WorkbenchLineagePane'
import {
  WorkbenchWorkspaceTabsProvider,
  useWorkbenchWorkspaceTabs,
  type WorkbenchWorkspaceTabsApi
} from '../workbenchWorkspaceTabs'

vi.mock('@renderer/data/studioAgentPlane', () => ({
  loadWorkbenchArtifact: vi.fn(),
  loadWorkbenchProofs: vi.fn(),
  actOnWorkbenchProof: vi.fn()
}))
vi.mock('@renderer/components/chat/panes/Shell', () => ({
  useRightPanelState: () => ({ presentationOpen: false }),
  useOptionalRightPanelActions: () => undefined
}))

let api: WorkbenchWorkspaceTabsApi
const proof = makeWorkbenchProof()
const next = makeWorkbenchProof({
  messageId: 'repair',
  runId: 'run-2',
  sourceText: 'hello2',
  artifacts: [
    {
      ...proof.artifacts[0],
      id: 'artifact-2',
      ordinal: 2,
      bytes: 6,
      executionId: 'run-2',
      receiptDigest: `sha256:${createHash('sha256').update('fixture receipt 2').digest('hex')}`,
      contentDigest: `sha256:${createHash('sha256').update('hello2').digest('hex')}`
    }
  ]
})

function Workspace({ open = true }: { open?: boolean }) {
  api = useWorkbenchWorkspaceTabs()
  const once = useRef(false)
  useEffect(() => {
    if (open && !once.current) {
      once.current = true
      api.openLineage(proof, proof.artifacts[0].id)
    }
  }, [open])
  const selected = api.tabs.find((tab) => tab.key === api.activeTabKey)
  return (
    <>
      <p data-testid="active-tab">{selected?.kind}</p>
      {selected?.lineage ? <WorkbenchLineagePane lineage={selected.lineage} /> : null}
    </>
  )
}

describe('version-bound lineage workspace', () => {
  beforeEach(() => {
    vi.stubGlobal('crypto', webcrypto)
    URL.createObjectURL = vi.fn(() => 'blob:test')
    URL.revokeObjectURL = vi.fn()
    vi.mocked(loadWorkbenchProofs).mockResolvedValue([proof, next])
    vi.mocked(loadWorkbenchArtifact).mockImplementation(async (_session, id) => {
      const version = id === 'artifact-2' ? next.artifacts[0] : proof.artifacts[0]
      return {
        id,
        name: version.name,
        mediaType: version.mediaType,
        contentDigest: version.contentDigest,
        data: btoa(id === 'artifact-2' ? 'hello2' : 'hello'),
        text: undefined
      }
    })
  })

  it('keeps a single workspace tab, supports seven keyboard tabs and immutable version navigation', async () => {
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace />
      </WorkbenchWorkspaceTabsProvider>
    )
    expect(screen.getAllByRole('tab')).toHaveLength(7)
    await screen.findByText('hello')
    fireEvent.click(screen.getByRole('button', { name: /下一版本|Next version/ }))
    await screen.findByText('hello2')
    expect(loadWorkbenchArtifact).toHaveBeenLastCalledWith(proof.sessionId, 'artifact-2', expect.any(AbortSignal))
    fireEvent.keyDown(screen.getByRole('tab', { name: /^(产物|Artifact)$/ }), { key: 'ArrowRight' })
    expect(screen.getByRole('tab', { name: /^(证据|Evidence)$/ })).toHaveFocus()
    expect(screen.getByRole('tabpanel')).toHaveTextContent(next.artifacts[0].contentDigest)
    act(() => api.openLineage(proof, proof.artifacts[0].id))
    expect(api.tabs.filter((tab) => tab.kind === 'lineage')).toHaveLength(1)
    act(() => api.openLineage(proof, proof.artifacts[0].id, true))
    expect(screen.getByRole('tab', { name: /^(审查|Review)$/ })).toHaveAttribute('aria-selected', 'true')
    fireEvent.click(screen.getByRole('tab', { name: /^(代码|Code)$/ }))
    act(() => api.syncProof({ ...proof, sourceText: 'updated proof' }))
    expect(screen.getByRole('tab', { name: /^(代码|Code)$/ })).toHaveAttribute('aria-selected', 'true')
    act(() => api.openLineage(proof, proof.artifacts[0].id, true))
    expect(screen.getByRole('tab', { name: /^(审查|Review)$/ })).toHaveAttribute('aria-selected', 'true')
  })

  it('does not reopen a closed pane or steal focus during proof synchronization', async () => {
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace />
      </WorkbenchWorkspaceTabsProvider>
    )
    await screen.findByText('hello')
    act(() => api.openSurface('files'))
    const key = api.activeTabKey
    act(() => api.syncProof({ ...proof, sourceText: 'live status' }))
    expect(api.activeTabKey).toBe(key)
    act(() => api.closeTab(api.tabs.find((tab) => tab.kind === 'lineage')!.key))
    act(() => api.syncProof({ ...proof, sourceText: 'later' }))
    expect(api.tabs.some((tab) => tab.kind === 'lineage')).toBe(false)
  })

  it('fails closed on a tampered byte snapshot', async () => {
    vi.mocked(loadWorkbenchArtifact).mockResolvedValue({
      id: 'artifact-1',
      name: 'report.md',
      mediaType: 'text/markdown',
      contentDigest: proof.artifacts[0].contentDigest,
      data: btoa('wrong'),
      text: undefined
    })
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace />
      </WorkbenchWorkspaceTabsProvider>
    )
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(/摘要不一致|do not match/))
    expect(screen.queryByRole('link', { name: /下载|Download/ })).toBeNull()
  })

  it('refuses another session and requires trace to be explicitly enabled', () => {
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace open={false} />
      </WorkbenchWorkspaceTabsProvider>
    )
    act(() => api.openLineage({ ...proof, sessionId: 'foreign' }, proof.artifacts[0].id))
    act(() => api.openLineage({ ...proof, policy: { reviewMode: true, artifactTrace: false } }, proof.artifacts[0].id))
    expect(api.tabs).toHaveLength(0)
  })

  it('copies the full focused digest and reports clipboard failure', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace />
      </WorkbenchWorkspaceTabsProvider>
    )
    await screen.findByText('hello')
    fireEvent.click(screen.getByRole('button', { name: /^(复制摘要|Copy digest)$/ }))
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(proof.artifacts[0].contentDigest))
    fireEvent.click(screen.getByRole('button', { name: /下一版本|Next version/ }))
    await screen.findByText('hello2')
    writeText.mockRejectedValueOnce(new Error('clipboard denied'))
    fireEvent.click(screen.getByRole('button', { name: /^(复制摘要|Copy digest)$/ }))
    await screen.findByText(/复制失败|Copy failed/)
    expect(writeText).toHaveBeenLastCalledWith(next.artifacts[0].contentDigest)
  })

  it.each(['resolved', 'rejected'] as const)(
    'does not show a late %s clipboard result on another version',
    async (outcome) => {
      let settle: () => void = () => {
        throw new Error('Clipboard request not started')
      }
      const writeText = vi.fn(
        () =>
          new Promise<void>((resolve, reject) => {
            settle = outcome === 'resolved' ? resolve : () => reject(new Error('clipboard denied'))
          })
      )
      Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
      render(
        <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
          <Workspace />
        </WorkbenchWorkspaceTabsProvider>
      )
      await screen.findByText('hello')
      fireEvent.click(screen.getByRole('button', { name: /^(复制摘要|Copy digest)$/ }))
      expect(writeText).toHaveBeenCalledWith(proof.artifacts[0].contentDigest)
      fireEvent.click(screen.getByRole('button', { name: /下一版本|Next version/ }))
      await screen.findByText('hello2')
      await act(async () => settle())
      expect(screen.getByRole('status')).toBeEmptyDOMElement()
    }
  )

  it('keeps incomplete bytes inspectable without presenting a completed trace', async () => {
    const partial = makeWorkbenchProof({
      producerStatus: 'failed',
      artifacts: [{ ...proof.artifacts[0], status: 'incomplete', error: 'producer execution did not complete' }],
      review: {
        status: 'failed',
        reviewer: 'independent-model',
        findings: [],
        error: 'producer execution did not complete'
      }
    })
    vi.mocked(loadWorkbenchProofs).mockResolvedValue([partial])
    render(
      <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
        <Workspace open={false} />
      </WorkbenchWorkspaceTabsProvider>
    )
    act(() => api.openLineage(partial, partial.artifacts[0].id))
    await screen.findByText('hello')
    expect(document.querySelector('[data-task-lineage]')).toHaveAttribute('data-task-lineage-status', 'incomplete')
    expect(screen.getByRole('link', { name: /下载此版本|Download this version/ })).toBeVisible()
    expect(screen.queryByText(/^(已追溯|Traced)$/)).toBeNull()
    expect(
      screen.getAllByRole('status').some((node) => /未完成|incomplete|did not complete/i.test(node.textContent ?? ''))
    ).toBe(true)
  })

  it('keeps acceptance visible when a repair version and its finding belong to different messages', async () => {
    const parent = makeWorkbenchProof({
      review: {
        status: 'open_queue',
        reviewer: 'independent-model',
        findings: [
          {
            id: 'fixed',
            severity: 'warning',
            category: 'change',
            claim: 'Fixed draft',
            evidenceQuote: 'hello',
            guidance: 'Check the repair',
            status: 'addressed',
            receiptDigest: next.artifacts[0].receiptDigest
          }
        ]
      }
    })
    const repair = {
      ...next,
      policy: { reviewMode: false, artifactTrace: true },
      review: { status: 'off' as const, reviewer: 'independent-model' as const, findings: [] }
    }
    vi.mocked(loadWorkbenchProofs).mockResolvedValue([parent, repair])
    vi.mocked(actOnWorkbenchProof).mockResolvedValue({
      ...parent,
      review: {
        ...parent.review,
        status: 'completed',
        findings: [{ ...parent.review.findings[0], status: 'accepted' }]
      }
    })
    render(
      <PartsProvider value={{ [parent.messageId]: [{ type: 'data-workbench-proof', data: parent }] }}>
        <WorkbenchWorkspaceTabsProvider sessionId={proof.sessionId}>
          <Workspace open={false} />
        </WorkbenchWorkspaceTabsProvider>
      </PartsProvider>
    )
    act(() => api.openLineage(repair, repair.artifacts[0].id, true))
    fireEvent.click(await screen.findByRole('button', { name: /^(验收|Accept)$/ }))
    await screen.findByText(/^(已验收|Accepted)$/)
    expect(screen.queryByRole('button', { name: /^(验收|Accept)$/ })).toBeNull()
    expect(actOnWorkbenchProof).toHaveBeenCalledWith(parent.sessionId, parent.messageId, {
      action: 'accept',
      findingId: 'fixed'
    })
  })
  it('localizes service-reported status without claiming an agent conversation or processor code', async () => {
    const external = makeWorkbenchProof({
      policy: { reviewMode: false, artifactTrace: true },
      review: { status: 'off', findings: [], reviewer: 'independent-model' },
      artifacts: [
        {
          ...proof.artifacts[0],
          origin: {
            kind: 'paperloom-service',
            memberId: 'member',
            workspaceRoot: '/workspace',
            sourcePath: '/workspace/original.pdf',
            sourceRevision: '12'.repeat(32),
            sourceDigest: `sha256:${'a'.repeat(64)}`,
            savedRevision: '34'.repeat(32),
            jobId: 'paper-job',
            jobStatus: 'succeeded',
            attempt: 1,
            artifactKey: 'translated-pdf',
            manifestDigest: `sha256:${'b'.repeat(64)}`,
            connectionRevision: '56'.repeat(32),
            packageDigest: `sha256:${'c'.repeat(64)}`
          }
        }
      ]
    })
    render(
      <WorkbenchLineagePane
        lineage={{ proof: external, artifactId: external.artifacts[0].id, review: false, requestId: 'service-copy' }}
        transport={{
          loadProofs: async () => [external],
          loadArtifact: async () => ({
            id: external.artifacts[0].id,
            name: 'report.md',
            mediaType: 'text/plain',
            contentDigest: external.artifacts[0].contentDigest,
            data: btoa('hello'),
            text: undefined
          })
        }}
      />
    )
    expect(screen.getAllByRole('tab')).toHaveLength(5)
    expect(screen.queryByRole('tab', { name: /^(代码|Code)$/ })).toBeNull()
    fireEvent.click(screen.getByRole('tab', { name: /^(来源|Source)$/ }))
    expect(screen.getByRole('tabpanel')).toHaveTextContent('已完成')
    expect(screen.getByRole('tabpanel')).not.toHaveTextContent('paperloom.succeeded')
  })
})
