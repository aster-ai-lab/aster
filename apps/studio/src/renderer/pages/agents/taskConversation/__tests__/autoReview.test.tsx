import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { actOnWorkbenchProof } from '@renderer/data/studioAgentPlane'
import { makeWorkbenchProof } from '@shared/types/__tests__/workbenchProof.fixture'
import { proofFromParts, workbenchProofSchema } from '@shared/types/workbenchProof'

import { quotedRange, TaskAutoReview } from '../TaskAutoReview'

vi.mock('@renderer/data/studioAgentPlane', () => ({ actOnWorkbenchProof: vi.fn(), loadWorkbenchProofs: vi.fn() }))

const finding = {
  id: 'finding-1',
  severity: 'warning' as const,
  category: 'change' as const,
  claim: 'Check the data',
  evidenceQuote: 'quoted data',
  guidance: 'Verify the original source',
  status: 'pending' as const
}

beforeEach(() => {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
  )
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('host-owned review and provenance', () => {
  it('keeps the answer scan visible for the whole live review and stops on completion', () => {
    vi.useFakeTimers()
    const proof = makeWorkbenchProof({ review: { status: 'running', reviewer: 'independent-model', findings: [] } })
    const view = (value: typeof proof) => (
      <div data-task-conversation-turn="">
        <p data-task-prose="">The answer under review.</p>
        <TaskAutoReview proof={value} onChange={vi.fn()} />
      </div>
    )
    const { container, rerender } = render(view(proof))
    expect(container.querySelector('[data-review-scanning]')).not.toBeNull()
    act(() => vi.advanceTimersByTime(3000))
    expect(container.querySelector('[data-review-scanning]')).not.toBeNull()
    rerender(view({ ...proof, review: { status: 'completed', reviewer: 'independent-model', findings: [] } }))
    expect(container.querySelector('[data-review-scanning]')).toBeNull()
  })

  it('ignores nested prose containers when locating the same exact quote', () => {
    const proof = makeWorkbenchProof({
      review: { status: 'open_queue', reviewer: 'independent-model', findings: [finding] }
    })
    const scroll = vi.fn()
    render(
      <div data-task-conversation-turn="">
        <div data-task-prose="outer">
          <p
            data-task-prose="inner"
            ref={(node) => {
              if (node) node.scrollIntoView = scroll
            }}>
            quoted data
          </p>
        </div>
        <TaskAutoReview proof={proof} onChange={vi.fn()} />
      </div>
    )
    // jsdom has no geometry; the browser suite verifies overlay visibility.
    fireEvent.click(document.querySelector<HTMLButtonElement>('.task-review-quote')!)
    expect(scroll).toHaveBeenCalledOnce()
    expect(screen.queryByRole('alert')).toBeNull()
  })
  it('accepts only a strict host data part bound to its own message', () => {
    const proof = makeWorkbenchProof()
    expect(proofFromParts([{ type: 'text', data: proof }])).toBeNull()
    expect(proofFromParts([{ type: 'data-workbench-proof', data: proof }], 'other')).toBeNull()
    expect(proofFromParts([{ type: 'data-workbench-proof', data: proof }], 'reply')).toEqual(proof)
    expect(
      workbenchProofSchema.safeParse({ ...proof, review: { ...proof.review, status: 'looks-clean' } }).success
    ).toBe(false)
    expect(
      workbenchProofSchema.safeParse({ ...proof, artifacts: [{ ...proof.artifacts[0], receiptDigest: 'path-only' }] })
        .success
    ).toBe(false)
  })

  it('is silent for a completed clean review and never adds a review-only artifact', () => {
    const { container } = render(
      <TaskAutoReview
        proof={makeWorkbenchProof({ policy: { reviewMode: true, artifactTrace: false } })}
        onChange={vi.fn()}
      />
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('offers ignore only when pending is idle, and acceptance only after verified repair', () => {
    const proof = makeWorkbenchProof({
      review: {
        status: 'remediating',
        reviewer: 'independent-model',
        findings: [finding, { ...finding, id: 'addressed', status: 'addressed' }]
      }
    })
    const { rerender } = render(<TaskAutoReview proof={proof} onChange={vi.fn()} />)
    expect(document.querySelector('[data-finding-status=pending]')).not.toHaveTextContent(/忽略|Ignore/)
    expect(document.querySelector('[data-finding-status=addressed]')).toHaveTextContent(/验收|Accept/)
    expect(screen.queryByText(/手动修复|Address/)).toBeNull()
    rerender(
      <TaskAutoReview proof={{ ...proof, review: { ...proof.review, status: 'open_queue' } }} onChange={vi.fn()} />
    )
    expect(document.querySelector('[data-finding-status=pending]')).toHaveTextContent(/忽略|Ignore/)
  })

  it('persists acceptance using the original session/message/finding identities', async () => {
    const proof = makeWorkbenchProof({
      review: { status: 'open_queue', reviewer: 'independent-model', findings: [{ ...finding, status: 'addressed' }] }
    })
    vi.mocked(actOnWorkbenchProof).mockResolvedValue(proof)
    const changed = vi.fn()
    render(<TaskAutoReview proof={proof} onChange={changed} />)
    fireEvent.click(screen.getByRole('button', { name: /验收|Accept/ }))
    await waitFor(() =>
      expect(actOnWorkbenchProof).toHaveBeenCalledWith(proof.sessionId, proof.messageId, {
        action: 'accept',
        findingId: finding.id
      })
    )
    expect(changed).toHaveBeenCalledWith(proof)
  })

  it('shows explicit failure and keeps findings available when collapsed', () => {
    const proof = makeWorkbenchProof({
      review: { status: 'failed', reviewer: 'independent-model', findings: [finding], error: 'Gateway unavailable' }
    })
    render(<TaskAutoReview proof={proof} onChange={vi.fn()} />)
    expect(screen.getByRole('alert')).toHaveTextContent('Gateway unavailable')
    expect(screen.getByRole('button', { name: /重试审查|Retry review/ })).toBeEnabled()
    fireEvent.click(document.querySelector('button[aria-expanded]')!)
    expect(document.querySelector('[data-task-review-finding]')).toBeNull()
    expect(proof.review.findings).toHaveLength(1)
  })

  it('anchors literal quotes across formatting but refuses ambiguous matches', () => {
    const root = document.createElement('div')
    root.innerHTML = '<p>before <b>quoted</b> data after</p>'
    expect(quotedRange(root, 'quoted data')?.toString()).toBe('quoted data')
    expect(quotedRange(root, 'QUOTED DATA')).toBeNull()
    root.append(' quoted data')
    expect(quotedRange(root, 'quoted data')).toBeNull()
  })

  it('mounts annotations outside the message list without adding space to the turn', () => {
    const proof = makeWorkbenchProof({
      review: { status: 'open_queue', reviewer: 'independent-model', findings: [finding] }
    })
    const { container, unmount } = render(
      <div data-task-conversation-turn="">
        <p data-task-prose="">quoted data</p>
        <TaskAutoReview proof={proof} onChange={vi.fn()} />
      </div>
    )
    const turn = container.querySelector('[data-task-conversation-turn]')!
    expect(turn.querySelector('[data-task-auto-review]')).toBeNull()
    expect((turn as HTMLElement).style.paddingBottom).toBe('')
    const overlay = document.querySelector('[data-task-review-floating]')!
    expect(overlay.parentElement).toBe(document.body)
    expect(overlay).toHaveStyle({ position: 'fixed' })
    unmount()
    expect(document.querySelector('[data-task-review-floating]')).toBeNull()
  })

  it('keeps severity annotations when collapsed and updates their disposition without scanning again', () => {
    const highlights = new Map<string, { ranges: Range[]; priority: number }>()
    class TestHighlight {
      priority = 0
      constructor(...publicRanges: Range[]) {
        this.ranges = publicRanges
      }
      ranges: Range[]
    }
    vi.stubGlobal('CSS', { highlights })
    vi.stubGlobal('Highlight', TestHighlight)
    const proof = makeWorkbenchProof({
      review: { status: 'open_queue', reviewer: 'independent-model', findings: [{ ...finding, severity: 'high' }] }
    })
    const view = (value: typeof proof) => (
      <div data-task-conversation-turn="">
        <p data-task-prose="">quoted data</p>
        <TaskAutoReview proof={value} onChange={vi.fn()} />
      </div>
    )
    const { container, rerender } = render(view(proof))
    const ranges = (kind: string) =>
      Array.from(highlights).find(([name]) => name.endsWith(`-${kind}`))?.[1].ranges.length
    expect(ranges('high')).toBe(1)
    fireEvent.click(document.querySelector('button[aria-expanded]')!)
    expect(ranges('high')).toBe(1)
    rerender(
      view({ ...proof, review: { ...proof.review, findings: [{ ...finding, severity: 'high', status: 'accepted' }] } })
    )
    expect(ranges('high')).toBe(0)
    expect(ranges('accepted')).toBe(1)
    expect(container.querySelector('[data-review-scanning]')).toBeNull()
  })
})
