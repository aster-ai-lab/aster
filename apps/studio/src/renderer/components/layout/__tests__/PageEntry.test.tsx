import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { act, fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { appShellPaintsEntry, clearPageEntrySession, PageEntry, workbenchEntryReady } from '../PageEntry'

const styles = readFileSync(resolve('src/renderer/assets/styles/tailwind.css'), 'utf8')

function Probe({ initial }: { initial: boolean }) {
  const [ready, setReady] = useState(initial)
  return (
    <div className="relative h-40">
      <button type="button" onClick={() => setReady(true)}>
        ready
      </button>
      <button type="button" onClick={() => setReady(false)}>
        pending
      </button>
      <PageEntry id="probe" ready={ready} />
    </div>
  )
}

describe('PageEntry', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
    clearPageEntrySession()
  })

  it('stays on screen while the view is not ready, then fades without replaying', () => {
    vi.useFakeTimers()
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    render(<Probe initial={false} />)

    now = 1000
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(document.querySelector('.page-loader--contain')).not.toBeNull()
    expect(document.querySelector('[data-ui="page.loader.wordmark"]')).not.toBeNull()
    expect(document.querySelectorAll('.towork-stroke')).toHaveLength(6)
    const handoff = document.querySelector('.page-loader-handoff')
    expect(handoff?.querySelector(':scope > [data-ui="page.loader.wordmark"]')).not.toBeNull()
    expect(handoff?.querySelector(':scope > .page-loader-ring [data-ui="page.loader.ring"]')).not.toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'ready' }))
    act(() => {
      vi.advanceTimersByTime(0)
    })
    expect(document.querySelector('.page-loader')).toHaveClass('is-leaving')
    act(() => {
      vi.advanceTimersByTime(180)
    })
    expect(document.querySelector('.page-loader')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'pending' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(document.querySelector('.page-loader')).toBeNull()
  })

  // Catch a wordmark that replays for the whole wait instead of handing over to the ring.
  it('writes the wordmark once, then hands the wait to the ring', () => {
    const block = (selector: string) => {
      const start = styles.indexOf(`${selector} {`)
      expect(start).toBeGreaterThanOrEqual(0)
      return styles.slice(start, styles.indexOf('}', start))
    }
    expect(block('.towork-wordmark-draw .towork-stroke')).toContain('animation-iteration-count: 1;')
    expect(block('.page-loader-handoff > .towork-wordmark-draw')).toContain('pageLoaderMarkOut 180ms ease 800ms both')
    expect(block('.page-loader-handoff > .page-loader-ring')).toContain('pageLoaderRingIn 180ms ease 800ms both')
  })

  it('holds an already-ready view for the entry beat before it fades', () => {
    vi.useFakeTimers()
    render(<Probe initial={true} />)

    act(() => {
      vi.advanceTimersByTime(499)
    })
    expect(document.querySelector('.page-loader')).not.toHaveClass('is-leaving')
    expect(document.querySelector('[data-ui="page.loader.wordmark"]')).not.toBeNull()

    act(() => {
      vi.advanceTimersByTime(1 + 180)
    })
    expect(document.querySelector('.page-loader')).toBeNull()
  })

  it('plays each view once and still plays a view that has not opened', () => {
    vi.useFakeTimers()
    const first = render(<PageEntry id="plugins" />)
    act(() => {
      vi.advanceTimersByTime(0)
    })
    expect(document.querySelector('.page-loader')).not.toBeNull()
    first.unmount()

    render(<PageEntry id="plugins" />)
    expect(document.querySelector('.page-loader')).toBeNull()

    render(<PageEntry id="files" />)
    expect(document.querySelectorAll('.page-loader')).toHaveLength(1)
  })

  it('keeps the work page loader up until the task list has settled', () => {
    const pending = {
      agentsLoading: false,
      sessionLoading: false,
      sessionsLoading: true,
      pinsLoading: false
    }
    expect(workbenchEntryReady(pending)).toBe(false)
    expect(workbenchEntryReady({ ...pending, sessionsLoading: false, pinsLoading: true })).toBe(false)
    expect(workbenchEntryReady({ ...pending, sessionsLoading: false })).toBe(true)
  })

  it('paints a shell entry for activity pages and leaves the homepage loaders to those views', () => {
    expect(appShellPaintsEntry('/app/agents')).toBe(false)
    expect(appShellPaintsEntry('/app/capabilities')).toBe(false)
    expect(appShellPaintsEntry('/app/org')).toBe(false)
    expect(appShellPaintsEntry('/app/plugin')).toBe(false)
    expect(appShellPaintsEntry('/app/knowledge')).toBe(true)
    expect(appShellPaintsEntry('/app/plugins')).toBe(true)
    expect(appShellPaintsEntry('/app/files')).toBe(true)
    expect(appShellPaintsEntry('/app/code')).toBe(true)
    expect(appShellPaintsEntry('/app/notes')).toBe(true)
    expect(appShellPaintsEntry('/app/mini-app')).toBe(true)
  })
})
