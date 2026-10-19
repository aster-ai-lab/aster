import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'

import { ExecutionTimeline } from '../ExecutionTimeline'
import { SkillPublishButton } from '../SkillPublishButton'

// ── SkillPublishButton ────────────────────────────────────────────────────

describe('SkillPublishButton', () => {
  it('disables when markdown is empty', () => {
    render(
      <SkillPublishButton assetId="asset-1" markdown="" />
    )
    const button = screen.getByRole('button')
    expect(button).toHaveProperty('disabled', true)
  })

  it('enables when markdown is present', () => {
    render(
      <SkillPublishButton assetId="asset-1" markdown="# My Skill" />
    )
    const button = screen.getByRole('button')
    expect(button).toHaveProperty('disabled', false)
  })

  it('calls onPublished with the commit SHA on success', async () => {
    const onPublished = vi.fn()
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ data: { commitSha: 'abc123def456' } })
    })
    render(
      <SkillPublishButton
        assetId="asset-1"
        markdown="# Skill"
        onPublished={onPublished}
      />
    )
    screen.getByRole('button').click()
    await waitFor(() => {
      expect(onPublished).toHaveBeenCalledWith('abc123def456')
    })
  })

  it('shows the error message on failure', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 403,
      json: () => Promise.resolve({ message: 'Forbidden' })
    })
    render(
      <SkillPublishButton assetId="asset-1" markdown="# Skill" />
    )
    screen.getByRole('button').click()
    await waitFor(() => {
      expect(screen.getByText('Forbidden')).to.exist
    })
  })
})

// ── ExecutionTimeline ─────────────────────────────────────────────────────

describe('ExecutionTimeline', () => {
  it('renders execution events as timeline steps', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          data: [
            { sequence: 1, kind: 'requested', content: { text: 'Fix the bug' } },
            { sequence: 2, kind: 'tool_call', content: { toolName: 'search' } },
            { sequence: 3, kind: 'text_delta', content: { text: 'Found it' } },
            { sequence: 4, kind: 'completed', content: { text: 'Done' } }
          ]
        })
    })
    render(<ExecutionTimeline executionId="exec-1" apiOrigin="http://api.test" />)
    await waitFor(() => {
      expect(screen.getByText(/4 events/)).to.exist
    })
    expect(screen.getByText(/Tool: search/)).to.exist
    expect(screen.getByText(/completed/)).to.exist
  })

  it('renders the empty state without crashing when no events exist', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ data: [] })
    })
    const { container } = render(
      <ExecutionTimeline executionId="exec-2" apiOrigin="http://api.test" />
    )
    // The component mounts and renders some content without crashing.
    await waitFor(
      () => {
        expect(container.querySelector('p')).toBeTruthy()
      },
      { timeout: 3000 }
    )
  })

  it('renders without crashing on fetch failure', async () => {
    global.fetch = vi.fn().mockRejectedValue(new Error('network'))
    const { container } = render(
      <ExecutionTimeline executionId="exec-3" apiOrigin="http://api.test" />
    )
    await waitFor(
      () => {
        expect(container.querySelector('p')).toBeTruthy()
      },
      { timeout: 3000 }
    )
  })
})
