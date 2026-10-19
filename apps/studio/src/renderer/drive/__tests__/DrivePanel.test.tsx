import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'

import { DrivePanel } from '../DrivePanel'

const FOLDER = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'

function installFetch(nodes: Array<Record<string, unknown>>, onPut?: (body: string) => void) {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
  })
  localStorage.setItem('a3s_org_member_token', 'member-session')
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET'
      if (method === 'PUT') onPut?.(String(init?.body ?? ''))
      const data = String(url).includes('/files') ? [] : nodes
      return {
        ok: true,
        status: 200,
        json: async () => ({ success: true, data: method === 'PUT' ? { ok: true } : data })
      }
    })
  )
}

beforeEach(() => {
  localStorage.clear()
  writeCenterId('org-1')
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  localStorage.clear()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('drive folder encyclopedia mark', () => {
  it('shows the mark only for an organization administrator and sends the opposite value', async () => {
    const bodies: string[] = []
    installFetch(
      [
        {
          id: FOLDER,
          parent_id: null,
          dept_id: null,
          name: '秘密',
          created_at: '2026-10-07',
          excluded: false,
          can_exclude: true
        }
      ],
      (body) => bodies.push(body)
    )
    render(<DrivePanel />)
    const toggle = await screen.findByRole('button', { name: '不编入百科' })
    expect(toggle).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(toggle)
    await waitFor(() => expect(bodies).toEqual(['{"excluded":true}']))
  })

  it('hides the mark from a member', async () => {
    installFetch([
      {
        id: FOLDER,
        parent_id: null,
        dept_id: null,
        name: '公开',
        created_at: '2026-10-07',
        excluded: true,
        can_exclude: false
      }
    ])
    render(<DrivePanel />)
    await screen.findByRole('button', { name: /公开/ })
    expect(screen.queryByRole('button', { name: '不编入百科' })).toBeNull()
  })
})
