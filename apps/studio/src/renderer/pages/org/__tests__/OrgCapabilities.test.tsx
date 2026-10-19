import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MemberOrgError } from '@renderer/data/memberOrgs'

import type * as AdminApi from '../orgAdminApi'
import { samePolicy, type TeamCapabilityPolicy } from '../orgAdminApi'
import { OrgCapabilitiesPanel } from '../OrgCapabilitiesPanel'

const admin = vi.hoisted(() => ({ getCapabilityPolicy: vi.fn(), saveCapabilityPolicy: vi.fn() }))
const catalog = vi.hoisted(() => ({ listCapabilities: vi.fn() }))
const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) => (vars ? `${key} ${JSON.stringify(vars)}` : key),
    i18n: { language: 'zh-CN' }
  })
}))
vi.mock('@renderer/services/toast', () => ({ toast }))
vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('../orgAdminApi', async (importOriginal) => ({
  ...(await importOriginal<typeof AdminApi>()),
  ...admin
}))
vi.mock('@renderer/pages/capabilities/capabilitiesData', () => catalog)

const OPEN: TeamCapabilityPolicy = {
  restricted: false,
  allowed: [],
  revision: 0,
  updated_by: null,
  updated_at: null
}

function entry(kind: 'connector' | 'skill' | 'plugin', slug: string, name: string) {
  return { kind, slug, name, category: 'office', description: '', color: '#000' }
}

function renderPanel() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <OrgCapabilitiesPanel orgId="org-1" />
    </SWRConfig>
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  admin.getCapabilityPolicy.mockReset()
  admin.saveCapabilityPolicy.mockReset()
  catalog.listCapabilities.mockImplementation(async ({ kind }: { kind: string }) => ({
    items:
      kind === 'connector'
        ? [entry('connector', 'web-search', '网页搜索')]
        : kind === 'skill'
          ? [entry('skill', 'pdf-reader', 'PDF 阅读')]
          : [],
    total: 1
  }))
})

describe('samePolicy', () => {
  it('ignores order and compares mode and entries', () => {
    const a = { kind: 'skill' as const, slug: 'a' }
    const b = { kind: 'connector' as const, slug: 'b' }
    expect(samePolicy({ restricted: true, allowed: [a, b] }, { restricted: true, allowed: [b, a] })).toBe(true)
    expect(samePolicy({ restricted: true, allowed: [a] }, { restricted: false, allowed: [a] })).toBe(false)
    expect(samePolicy({ restricted: true, allowed: [a] }, { restricted: true, allowed: [b] })).toBe(false)
  })
})

describe('OrgCapabilitiesPanel', () => {
  it('restricts installs to a picked list and saves against the read revision', async () => {
    admin.getCapabilityPolicy.mockResolvedValue(OPEN)
    admin.saveCapabilityPolicy.mockImplementation(async (_org, next) => ({
      ...OPEN,
      restricted: next.restricted,
      allowed: next.allowed,
      revision: 1
    }))
    renderPanel()

    fireEvent.click(await screen.findByRole('radio', { name: /org.capabilities.mode.restricted/ }))
    expect(screen.getByText('org.capabilities.emptyRestricted')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'org.capabilities.add' }))
    const picker = await screen.findByRole('dialog')
    fireEvent.click(await within(picker).findByRole('button', { name: /org.capabilities.picker.pick .*网页搜索/ }))
    fireEvent.click(within(picker).getByRole('tab', { name: 'org.capabilities.kind.skill' }))
    fireEvent.click(await within(picker).findByRole('button', { name: /org.capabilities.picker.pick .*PDF 阅读/ }))
    fireEvent.click(within(picker).getByRole('button', { name: 'org.capabilities.picker.done' }))

    const list = await screen.findByRole('list', { name: 'org.capabilities.listLabel' })
    expect(within(list).getAllByRole('listitem')).toHaveLength(2)
    fireEvent.click(screen.getByRole('button', { name: 'org.capabilities.save' }))
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('org.capabilities.saved'))
    expect(admin.saveCapabilityPolicy).toHaveBeenCalledWith('org-1', {
      restricted: true,
      allowed: [
        { kind: 'connector', slug: 'web-search' },
        { kind: 'skill', slug: 'pdf-reader' }
      ],
      expected_revision: 0
    })
    await waitFor(() => expect(screen.queryByRole('button', { name: 'org.capabilities.save' })).not.toBeInTheDocument())
  })

  it('discards a draft back to the saved policy', async () => {
    admin.getCapabilityPolicy.mockResolvedValue({
      ...OPEN,
      restricted: true,
      allowed: [{ kind: 'skill', slug: 'pdf-reader' }],
      revision: 3
    } satisfies TeamCapabilityPolicy)
    renderPanel()
    const remove = await screen.findByRole('button', { name: /org.capabilities.remove .*PDF 阅读/ })
    fireEvent.click(remove)
    expect(screen.getByText('org.capabilities.empty')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'org.capabilities.discard' }))
    expect(await screen.findByRole('button', { name: /org.capabilities.remove/ })).toBeInTheDocument()
    expect(admin.saveCapabilityPolicy).not.toHaveBeenCalled()
  })

  it('reloads the latest policy when someone else saved first', async () => {
    let stored: TeamCapabilityPolicy = OPEN
    admin.getCapabilityPolicy.mockImplementation(async () => stored)
    admin.saveCapabilityPolicy.mockImplementation(async () => {
      stored = { ...OPEN, restricted: true, revision: 1 }
      throw new MemberOrgError('capability policy changed; reload and try again', 409)
    })
    renderPanel()
    fireEvent.click(await screen.findByRole('radio', { name: /org.capabilities.mode.restricted/ }))
    fireEvent.click(screen.getByRole('button', { name: 'org.capabilities.save' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.capabilities.stale'))
    await waitFor(() =>
      expect(screen.getByRole('radio', { name: /org.capabilities.mode.restricted/ })).toHaveAttribute(
        'aria-checked',
        'true'
      )
    )
    expect(screen.queryByRole('button', { name: 'org.capabilities.save' })).not.toBeInTheDocument()
  })

  it('keeps the draft when a save fails for another reason', async () => {
    admin.getCapabilityPolicy.mockResolvedValue(OPEN)
    admin.saveCapabilityPolicy.mockRejectedValue(new MemberOrgError('boom', 500))
    renderPanel()
    fireEvent.click(await screen.findByRole('radio', { name: /org.capabilities.mode.restricted/ }))
    fireEvent.click(screen.getByRole('button', { name: 'org.capabilities.save' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.error.generic'))
    expect(await screen.findByRole('button', { name: 'org.capabilities.save' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /org.capabilities.mode.restricted/ })).toHaveAttribute(
      'aria-checked',
      'true'
    )
  })

  it('explains a denied page without a retry', async () => {
    admin.getCapabilityPolicy.mockRejectedValue(new MemberOrgError('not found', 404))
    renderPanel()
    expect(await screen.findByText('org.capabilities.denied')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.retry' })).not.toBeInTheDocument()
  })
})
