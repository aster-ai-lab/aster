import { act, render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AixAppletWindow } from '../AixAppletWindow'

const readMemberAccount = vi.hoisted(() => vi.fn())
const publishMemberProfile = vi.hoisted(() => vi.fn())

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('@renderer/data/memberOrgs', () => ({ readMemberAccount }))

vi.mock('@renderer/components/account/memberProfile', () => ({ publishMemberProfile }))

vi.mock('@renderer/components/plugins/AixWorkbenchRuntime', () => ({
  AixWorkbenchRuntime: ({ packageSlug, children }: { packageSlug: string; children: ReactNode }) => (
    <div data-testid="aix-runtime" data-slug={packageSlug}>
      {children}
    </div>
  )
}))

vi.mock('@renderer/components/plugins/PluginPage', () => ({
  PluginPage: () => <div data-testid="aix-page" />
}))

vi.mock('@renderer/pages/agents/AppletWindowBar', () => ({
  AppletWindowBar: ({ title }: { title: string }) => <div>{title}</div>
}))

async function flush(ms = 0) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })
}

afterEach(() => {
  vi.useRealTimers()
  readMemberAccount.mockReset()
  publishMemberProfile.mockReset()
})

describe('AixAppletWindow', () => {
  // Catch a window that settles on an id-less session account while the OS is restarting.
  it('retries the member account until it carries an id, then mounts only this package', async () => {
    vi.useFakeTimers()
    readMemberAccount
      .mockResolvedValueOnce({ displayName: 'Roy', phone: '' })
      .mockResolvedValueOnce({ displayName: 'Roy', phone: '' })
      .mockResolvedValue({ displayName: 'Roy', phone: '', id: 'member-1' })
    render(<AixAppletWindow slug="a3s-g1-arm" view="workbench" title="G1" />)
    await flush()
    expect(screen.queryByTestId('aix-runtime')).not.toBeInTheDocument()
    await flush(1000)
    expect(screen.queryByTestId('aix-runtime')).not.toBeInTheDocument()
    await flush(2000)
    expect(screen.getByTestId('aix-runtime')).toHaveAttribute('data-slug', 'a3s-g1-arm')
    expect(readMemberAccount).toHaveBeenCalledTimes(3)
    expect(publishMemberProfile).toHaveBeenCalledWith(expect.objectContaining({ id: 'member-1' }))
  })

  // Catch a denied window that never recovers once the OS is reachable again.
  it('reports a missing mini app after the attempts run out and retries on focus', async () => {
    vi.useFakeTimers()
    readMemberAccount.mockResolvedValue({ displayName: 'Roy', phone: '' })
    render(<AixAppletWindow slug="a3s-g1-arm" view="workbench" title="G1" />)
    await flush(60_000)
    expect(screen.getByRole('status')).toHaveTextContent('sidebar.resource_missing')
    expect(readMemberAccount).toHaveBeenCalledTimes(8)
    readMemberAccount.mockResolvedValue({ displayName: 'Roy', phone: '', id: 'member-1' })
    await act(async () => {
      globalThis.window.dispatchEvent(new globalThis.Event('focus'))
    })
    await flush()
    expect(screen.getByTestId('aix-runtime')).toBeInTheDocument()
  })
})
