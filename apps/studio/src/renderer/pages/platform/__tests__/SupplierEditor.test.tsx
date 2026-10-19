import { Button } from '@a3s-lab/ui'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import useSWR, { SWRConfig, useSWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { bindCenterMember, releaseCenterMember } from '@renderer/data/memberOrgs'
import { readPlatformCatalogPage } from '@renderer/data/platformCatalog'
import type { SupplierCreateResolution, SupplierRecord } from '@renderer/data/platformSuppliers'

import { writePlatformMenu } from '../platformNav'
import PlatformPage from '../PlatformPage'
import { SupplierEditorProvider, useSupplierActions } from '../SupplierEditor'

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))

const id = '11111111-1111-4111-8111-111111111111'
const scope = 'member-a/official-team'
const intent = '22222222-2222-4222-8222-222222222222'
const markerKey = `studio/platform/supplier-create-intent/${scope}`
const original: SupplierRecord = {
  id,
  name: 'Acme',
  kind: 'vendor',
  protocol: 'openai',
  baseUrl: 'https://upstream.example/v1',
  hasKey: true,
  keyHint: '…hint',
  dataRegion: 'region-a',
  retainsData: false,
  originVendors: ['exact vendor'],
  models: ['exact/model'],
  enabled: true,
  routable: true,
  createdAt: 100,
  updatedAt: 101
}
let record: SupplierRecord | null
let access: { read: boolean; create: boolean; update: boolean; delete: boolean }
let accessStatus: number
let requests: { method: string; path: string; body: Record<string, unknown> }[]
let pageQueries: string[]
let uncertainCreate: boolean
let createBarrier: Promise<void> | undefined
let updateBarrier: Promise<void> | undefined
let deleteBlocked: boolean
let dependencyTotal: number
let relatedRevision: number | undefined
let relatedFailure: boolean
let pageFailure: boolean
let recoveryResult: SupplierCreateResolution
let recoveryStatus: number
let recoveryBarrier: Promise<void> | undefined
let revalidate: () => Promise<unknown>
const say = (key: string, values?: Record<string, unknown>) => String(i18n.t(`platform.supplierForm.${key}`, values))
async function chooseRowAction(
  user: ReturnType<typeof userEvent.setup>,
  action: 'editAction' | 'deleteAction',
  name: string
) {
  await user.click(await screen.findByRole('button', { name: say('moreAction', { name }) }))
  await user.click(await screen.findByRole('menuitem', { name: say(action, { name }) }))
}

function Controls() {
  const actions = useSupplierActions()!
  const { mutate } = useSWRConfig()
  revalidate = () => mutate(`studio/platform/${scope}/supplier-access`)
  return (
    <>
      {actions.access?.create ? <Button onClick={() => actions.open('create')}>{say('createTitle')}</Button> : null}
      {actions.access?.update ? <Button onClick={() => actions.open('edit', id)}>{say('edit')}</Button> : null}
      {actions.access?.delete ? <Button onClick={() => actions.open('delete', id)}>{say('delete')}</Button> : null}
      {actions.access?.read ? <Button onClick={() => actions.open('read', id)}>{say('read')}</Button> : null}
    </>
  )
}
function Preview() {
  const { data, error } = useSWR(JSON.stringify([`studio/platform/${scope}/suppliers/page`, 1, 50]), () =>
    readPlatformCatalogPage('suppliers', {
      page: 1,
      limit: 50,
      q: '',
      filter: '',
      brand: '',
      sort: '',
      direction: 'asc'
    })
  )
  return (
    <p role="status" aria-label="Collection preview">
      {error ? 'unavailable' : data ? `${data.total}:${data.rows.map((item) => item.name).join(',')}` : 'loading'}
    </p>
  )
}
function Harness({ identity = scope, preview = false }: { identity?: string; preview?: boolean }) {
  return (
    <SupplierEditorProvider key={identity} scope={identity}>
      <Controls />
      {preview ? <Preview /> : null}
    </SupplierEditorProvider>
  )
}
function mount() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
      <Harness />
    </SWRConfig>
  )
}
function mountPage() {
  bindCenterMember('33333333-3333-4333-8333-333333333333')
  localStorage.setItem('a3s.org.center', 'official-team')
  writePlatformMenu('suppliers')
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
      <PlatformPage />
    </SWRConfig>
  )
}
function json(data: unknown, status = 200) {
  return new Response(status === 204 ? null : JSON.stringify({ code: status < 400 ? 0 : status, data }), { status })
}
beforeEach(async () => {
  if (!HTMLElement.prototype.hasPointerCapture) HTMLElement.prototype.hasPointerCapture = () => false
  if (!HTMLElement.prototype.setPointerCapture) HTMLElement.prototype.setPointerCapture = () => {}
  if (!HTMLElement.prototype.releasePointerCapture) HTMLElement.prototype.releasePointerCapture = () => {}
  await i18n.changeLanguage('en-US')
  localStorage.clear()
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) },
    configurable: true
  })
  record = structuredClone(original)
  access = { read: true, create: true, update: true, delete: true }
  accessStatus = 200
  requests = []
  pageQueries = []
  uncertainCreate = false
  createBarrier = undefined
  updateBarrier = undefined
  deleteBlocked = false
  dependencyTotal = 0
  relatedRevision = undefined
  relatedFailure = false
  pageFailure = false
  recoveryResult = { state: 'canceled' }
  recoveryStatus = 200
  recoveryBarrier = undefined
  vi.stubGlobal(
    'fetch',
    vi.fn<typeof fetch>(async (url, init) => {
      const path = new URL(String(url)).pathname
      const method = init?.method ?? 'GET'
      if (path === '/api/member-center') return json({ ok: true })
      if (path === '/api/member-orgs')
        return json([
          {
            id: 'official-team',
            name: 'Platform team',
            logo: '',
            creator: true,
            membership: 'admin',
            is_official: true
          }
        ])
      if (path.endsWith('/access')) return json(access, accessStatus)
      if (path.endsWith('/models/page') || path.endsWith('/dependencies/page')) {
        if (relatedFailure) return json(null, 503)
        const params = new URL(String(url)).searchParams
        const kind = path.endsWith('/models/page') ? 'models' : 'dependencies'
        const limit = Number(params.get('limit') ?? 20)
        const source =
          kind === 'models'
            ? (record?.models ?? []).map((model) => ({
                id: model,
                name: model,
                state: null,
                upstreamModels: [],
                dependencyCount: dependencyTotal
              }))
            : Array.from({ length: dependencyTotal }, (_, n) => ({
                id: `dependent-${n}`,
                name: `Dependent ${n}`,
                state: 'draft',
                upstreamModels: ['undeclared-model'],
                dependencyCount: 0
              }))
        const page = Math.min(Number(params.get('page') ?? 1), Math.max(1, Math.ceil(source.length / limit)))
        return json({
          supplierId: id,
          kind,
          items: source.slice((page - 1) * limit, page * limit),
          total: source.length,
          page,
          limit,
          revision: relatedRevision ?? record?.updatedAt ?? 101,
          dependencyTotal
        })
      }
      if (path.endsWith('/page')) {
        pageQueries.push(new URL(String(url)).searchParams.get('search') ?? '')
        return pageFailure
          ? json(null, 503)
          : json({
              items: record
                ? [{ id: record.id, name: record.name, fields: [{ key: 'kind', value: record.kind }] }]
                : [],
              total: record ? 1 : 0,
              page: 1,
              limit: Number(new URL(String(url)).searchParams.get('limit')),
              facets: { kind: record ? [{ value: record.kind, count: 1 }] : [] }
            })
      }
      const body = init?.body ? JSON.parse(String(init.body)) : {}
      if (method !== 'GET') requests.push({ method, path, body })
      if (path.endsWith('/resolve')) {
        if (recoveryBarrier) await recoveryBarrier
        return json(recoveryResult, recoveryStatus)
      }
      if (method === 'POST') {
        const write = body.supplier
        record = { ...original, ...write, hasKey: write.credential.action === 'replace', updatedAt: 101 }
        if (createBarrier) await createBarrier
        if (uncertainCreate) {
          uncertainCreate = false
          throw new TypeError('network result lost')
        }
        return json({ id, revision: 101 }, 201)
      }
      if (!path.endsWith(`/${id}`)) return json([])
      if (!record) return json(null, 404)
      if (method === 'PUT') {
        if (body.revision !== record.updatedAt) return json(null, 409)
        if (updateBarrier) await updateBarrier
        const write = body.supplier
        record = {
          ...record,
          ...write,
          hasKey: write.credential.action === 'keep' ? record.hasKey : write.credential.action === 'replace',
          updatedAt: record.updatedAt + 1
        }
        return json(record)
      }
      if (method === 'DELETE') {
        if (deleteBlocked || body.revision !== record.updatedAt) return json(null, 409)
        record = null
        return json(null, 204)
      }
      return json(record)
    })
  )
})
afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  releaseCenterMember()
  vi.unstubAllGlobals()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
})

async function start(mode: 'createTitle' | 'edit' | 'delete' | 'read') {
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: say(mode) }))
  return user
}
async function fillNew(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByRole('textbox', { name: say('name') }), 'New supplier')
  await user.type(screen.getByRole('textbox', { name: say('baseUrl') }), 'https://new.example/v1')
  await user.click(screen.getByRole('combobox', { name: say('credential') }))
  await user.click(await screen.findByRole('option', { name: say('option.replace') }))
  const secret = screen.getByLabelText(say('secret'), { exact: false })
  expect(secret).toHaveAccessibleName(say('secret'))
  await user.type(secret, 'private-credential')
}

describe('supplier editor workflows with real UI primitives', () => {
  it('pages every blocking dependency including undeclared upstream models and disables deletion until a fresh empty review', async () => {
    dependencyTotal = 31
    mount()
    const user = await start('delete')
    const dialog = await screen.findByRole('dialog')
    await within(dialog).findByText('Dependent 0')
    expect(within(dialog).getByRole('button', { name: say('confirmDelete') })).toBeDisabled()
    expect(within(dialog).getAllByText('undeclared-model')).toHaveLength(20)
    await user.click(within(dialog).getByRole('button', { name: String(i18n.t('common.next')) }))
    expect(await within(dialog).findByText('Dependent 30')).toBeVisible()
    expect(within(dialog).queryByText('Dependent 0')).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(0)
    dependencyTotal = 0
    await user.click(within(dialog).getByRole('button', { name: String(i18n.t('common.refresh')) }))
    await waitFor(() => expect(within(dialog).getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(within(dialog).getByRole('button', { name: say('confirmDelete') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(record).toBeNull()
  })
  it('fails closed on an unavailable dependency check and allows a read retry before deleting', async () => {
    relatedFailure = true
    mount()
    const user = await start('delete')
    const dialog = await screen.findByRole('dialog')
    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      String(i18n.t('platform.supplierRelations.failed'))
    )
    expect(within(dialog).getByRole('button', { name: say('confirmDelete') })).toBeDisabled()
    relatedFailure = false
    await user.click(within(dialog).getByRole('button', { name: String(i18n.t('common.retry')) }))
    await waitFor(() => expect(within(dialog).getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    expect(record?.id).toBe(id)
    expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(0)
  })
  it('requires explicit current-record review when dependency metadata names a newer supplier revision', async () => {
    relatedRevision = 110
    mount()
    const user = await start('delete')
    expect(await screen.findByText(String(i18n.t('platform.supplierRelations.stale')))).toBeVisible()
    expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeDisabled()
    record = { ...original, updatedAt: 110 }
    await user.click(await screen.findByRole('button', { name: say('readLatest') }))
    await user.click(await screen.findByRole('button', { name: say('acceptRevision') }))
    await waitFor(() => expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: say('confirmDelete') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(requests.find((request) => request.method === 'DELETE')?.body).toEqual({ revision: 110 })
  })
  it('links validation errors, creates once while pending and shows the authoritative record without its credential', async () => {
    let finish!: () => void
    createBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    mount()
    const user = await start('createTitle')
    await user.click(screen.getByRole('button', { name: say('save') }))
    expect(screen.getByRole('alert')).toHaveFocus()
    await user.click(screen.getByRole('link', { name: say('nameRequired') }))
    expect(screen.getByRole('textbox', { name: say('name') })).toHaveFocus()
    expect(requests).toEqual([])
    await fillNew(user)
    await user.click(screen.getByRole('button', { name: say('save') }))
    const submitting = await screen.findByRole('button', { name: say('submitting') })
    expect(submitting).toBeDisabled()
    await user.keyboard('{Escape}')
    expect(screen.getByRole('dialog')).toBeVisible()
    await user.click(submitting)
    expect(requests.filter((item) => item.method === 'POST')).toHaveLength(1)
    await act(async () => finish())
    expect(await screen.findByRole('dialog', { name: say('saved') })).toHaveTextContent('New supplier')
    expect(screen.queryByLabelText(say('secret'), { exact: false })).not.toBeInTheDocument()
    expect(localStorage.getItem(`studio/platform/supplier-create-intent/${scope}`)).toBeNull()
    expect(record?.name).toBe('New supplier')
  })
  it('protects a dirty draft on Escape and keeps it when the user returns to editing', async () => {
    mount()
    const user = await start('edit')
    const name = await screen.findByRole('textbox', { name: say('name') })
    await user.clear(name)
    await user.type(name, 'My draft')
    await user.keyboard('{Escape}')
    expect(await screen.findByRole('button', { name: say('keepEditing') })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: say('keepEditing') }))
    expect(screen.getByRole('textbox', { name: say('name') })).toHaveValue('My draft')
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.cancel')) }))
    await user.click(screen.getByRole('button', { name: say('discard') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: say('edit') })).toHaveFocus()
    expect(requests).toEqual([])
  })
  it('preserves the draft on a stale revision and requires explicit review before submitting a new revision', async () => {
    mount()
    const user = await start('edit')
    const name = await screen.findByRole('textbox', { name: say('name') })
    expect(screen.queryByLabelText(say('secret'), { exact: false })).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: say('models') })).toHaveValue('exact/model')
    await user.clear(name)
    await user.type(name, 'My reviewed draft')
    record = { ...original, name: 'Changed elsewhere', updatedAt: 105 }
    await user.click(screen.getByRole('button', { name: say('save') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('conflict'))
    expect(screen.getByRole('textbox', { name: say('name') })).toHaveValue('My reviewed draft')
    expect(screen.getByRole('button', { name: say('save') })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: say('readLatest') }))
    expect(await screen.findByText('Changed elsewhere')).toBeVisible()
    expect(requests).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: say('acceptRevision') }))
    expect(screen.getByRole('textbox', { name: say('name') })).toHaveValue('My reviewed draft')
    await user.click(screen.getByRole('button', { name: say('save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await waitFor(() => expect(record).toMatchObject({ name: 'My reviewed draft', hasKey: true, updatedAt: 106 }))
    expect(requests[1].body).toMatchObject({ revision: 105, supplier: { credential: { action: 'keep' } } })
  })
  it('clears a credential only through the explicit clear choice', async () => {
    mount()
    const user = await start('edit')
    await screen.findByRole('textbox', { name: say('name') })
    await user.click(screen.getByRole('combobox', { name: say('credential') }))
    await user.click(await screen.findByRole('option', { name: say('option.clear') }))
    expect(screen.getByRole('note')).toHaveTextContent(say('clearWarning'))
    await user.click(screen.getByRole('button', { name: say('save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await waitFor(() => expect(record?.hasKey).toBe(false))
    expect(requests[0].body).toMatchObject({ revision: 101, supplier: { credential: { action: 'clear' } } })
  })
  it('keeps a refused deletion open and deletes only the explicitly reviewed current revision', async () => {
    deleteBlocked = true
    mount()
    const user = await start('delete')
    await within(await screen.findByRole('dialog')).findByText('https://upstream.example/v1')
    expect(screen.getByRole('button', { name: String(i18n.t('common.cancel')) })).toHaveFocus()
    await waitFor(() => expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: say('confirmDelete') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('conflict'))
    expect(record).not.toBeNull()
    deleteBlocked = false
    record = { ...original, updatedAt: 110 }
    await user.click(screen.getByRole('button', { name: say('readLatest') }))
    await user.click(await screen.findByRole('button', { name: say('acceptRevision') }))
    await waitFor(() => expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: say('confirmDelete') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(record).toBeNull()
    expect(requests[1].body).toEqual({ revision: 110 })
  })
  it('retries an uncertain creation with the identical intent and payload, without persisting its secret', async () => {
    uncertainCreate = true
    mount()
    const user = await start('createTitle')
    await fillNew(user)
    await user.click(screen.getByRole('button', { name: say('save') }))
    await screen.findByRole('button', { name: say('retryIntent') })
    const marker = localStorage.getItem(`studio/platform/supplier-create-intent/${scope}`)
    expect(marker).toMatch(/^[\da-f-]{36}$/)
    expect(JSON.stringify(localStorage)).not.toContain('private-credential')
    await user.click(screen.getByRole('button', { name: say('leavePending') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await user.click(screen.getByRole('button', { name: say('resume') }))
    expect(screen.getByLabelText(say('secret'), { exact: false })).toHaveValue('private-credential')
    await user.click(screen.getByRole('button', { name: say('retryIntent') }))
    await screen.findByRole('dialog', { name: say('saved') })
    expect(requests).toHaveLength(2)
    expect(requests[1].body).toEqual(requests[0].body)
    expect(localStorage.getItem(`studio/platform/supplier-create-intent/${scope}`)).toBeNull()
  })
  it('blocks a second create after a remount with an unresolved persisted intent', async () => {
    localStorage.setItem(
      'studio/platform/supplier-create-intent/33333333-3333-4333-8333-333333333333/official-team',
      '22222222-2222-4222-8222-222222222222'
    )
    mountPage()
    const user = userEvent.setup()
    const create = await screen.findByRole('button', { name: say('resume') })
    expect(create).toBeDisabled()
    await user.click(create)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.getByText(say('recoveryRequired'))).toBeVisible()
    expect(requests).toEqual([])
    await chooseRowAction(user, 'editAction', 'Acme')
    expect(await screen.findByRole('textbox', { name: say('name') })).toHaveValue('Acme')
  })
  it('recovers an uncreated request after restart, fences duplicate confirmation and permits a new intent', async () => {
    const pageMarker = 'studio/platform/supplier-create-intent/33333333-3333-4333-8333-333333333333/official-team'
    localStorage.setItem(pageMarker, intent)
    let finish!: () => void
    recoveryBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    mountPage()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    const dialog = await screen.findByRole('dialog', { name: say('recoveryTitle') })
    expect(within(dialog).getByRole('button', { name: String(i18n.t('common.cancel')) })).toHaveFocus()
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(localStorage.getItem(pageMarker)).toBe(intent)
    expect(requests).toEqual([])
    await user.click(screen.getByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    const submitting = await screen.findByRole('button', { name: say('submitting') })
    expect(submitting).toBeDisabled()
    await user.click(submitting)
    await user.keyboard('{Escape}')
    expect(screen.getByRole('dialog', { name: say('recoveryTitle') })).toBeVisible()
    expect(requests).toEqual([
      { method: 'POST', path: `/api/member/platform/suppliers/intents/${intent}/resolve`, body: {} }
    ])
    await act(async () => finish())
    expect(await screen.findByText(say('recoveryCanceled'))).toBeVisible()
    expect(localStorage.getItem(pageMarker)).toBeNull()
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await user.click(await screen.findByRole('button', { name: say('createTitle') }))
    await fillNew(user)
    await user.click(screen.getByRole('button', { name: say('save') }))
    await screen.findByRole('dialog', { name: say('saved') })
    expect(requests).toHaveLength(2)
    expect(requests[1].path).toBe('/api/member/platform/suppliers')
    expect(requests[1].body.intentId).not.toBe(intent)
    expect(JSON.stringify(localStorage)).not.toContain('private-credential')
  })
  it('recovers the original create receipt in Chinese and opens the current changed record without creating again', async () => {
    await i18n.changeLanguage('zh-CN')
    localStorage.setItem(markerKey, intent)
    recoveryResult = { state: 'created', id, revision: 101 }
    record = { ...original, name: 'Current supplier', updatedAt: 110 }
    mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    expect(await screen.findByText(say('recoveryCreated', { id, revision: 101 }))).toBeVisible()
    expect(localStorage.getItem(markerKey)).toBeNull()
    await user.click(screen.getByRole('button', { name: say('recoveryView') }))
    expect(
      await screen.findByRole('dialog', { name: say('readTitle', { name: 'Current supplier' }) })
    ).toHaveTextContent('Current supplier')
    expect(screen.getByRole('dialog')).toHaveTextContent('110')
    expect(requests).toHaveLength(1)
    expect(requests[0].body).toEqual({})
  })
  it('keeps the same request unresolved after recovery failure and retries only the resolution command', async () => {
    localStorage.setItem(markerKey, intent)
    recoveryStatus = 503
    mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('recoveryFailed'))
    expect(localStorage.getItem(markerKey)).toBe(intent)
    recoveryStatus = 200
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.retry')) }))
    expect(await screen.findByText(say('recoveryCanceled'))).toBeVisible()
    expect(localStorage.getItem(markerKey)).toBeNull()
    expect(requests).toHaveLength(2)
    expect(requests[1]).toEqual(requests[0])
  })
  it('separates a confirmed recovery from refresh failure and retries only the collection read', async () => {
    localStorage.setItem(markerKey, intent)
    recoveryResult = { state: 'created', id, revision: 101 }
    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness preview />
      </SWRConfig>
    )
    await screen.findByText('1:Acme')
    pageFailure = true
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    await screen.findByText(say('recoveryCreated', { id, revision: 101 }))
    await user.keyboard('{Escape}')
    expect(await screen.findByText(say('recoveryRefreshFailed'))).toBeVisible()
    expect(screen.getByRole('status', { name: 'Collection preview' })).toHaveTextContent('unavailable')
    pageFailure = false
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.retry')) }))
    expect(await screen.findByText('1:Acme')).toBeVisible()
    expect(localStorage.getItem(markerKey)).toBeNull()
    expect(requests).toHaveLength(1)
    expect(requests[0].path).toMatch(/\/resolve$/)
  })
  it('retains the pending marker if local cleanup fails after owner confirmation and completes recovery on retry', async () => {
    localStorage.setItem(markerKey, intent)
    const clear = vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(() => {
      throw new Error('storage unavailable')
    })
    mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('recoveryStorageFailed'))
    expect(localStorage.getItem(markerKey)).toBe(intent)
    clear.mockRestore()
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: String(i18n.t('common.retry')) }))
    await waitFor(() => expect(localStorage.getItem(markerKey)).toBeNull())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(requests).toHaveLength(2)
    expect(requests.every((request) => request.path.endsWith('/resolve'))).toBe(true)
  })
  it('ignores an old recovery completion after account replacement and preserves both accounts pending markers', async () => {
    localStorage.setItem(markerKey, intent)
    const other = 'member-b/official-team'
    const otherMarker = `studio/platform/supplier-create-intent/${other}`
    const otherIntent = '44444444-4444-4444-8444-444444444444'
    localStorage.setItem(otherMarker, otherIntent)
    let finish!: () => void
    recoveryBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    const view = render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness />
      </SWRConfig>
    )
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    view.rerender(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness identity={other} />
      </SWRConfig>
    )
    await act(async () => finish())
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(localStorage.getItem(markerKey)).toBe(intent)
    expect(localStorage.getItem(otherMarker)).toBe(otherIntent)
    expect(await screen.findByRole('button', { name: say('recoveryTitle') })).toBeEnabled()
  })
  it('withdraws pending recovery when the create grant is revoked while keeping independent read access', async () => {
    localStorage.setItem(markerKey, intent)
    let finish!: () => void
    recoveryBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: say('recoveryTitle') }))
    await user.click(screen.getByRole('button', { name: say('recoveryConfirm') }))
    access = { ...access, create: false }
    await act(async () => {
      await revalidate()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await act(async () => finish())
    expect(localStorage.getItem(markerKey)).toBe(intent)
    expect(screen.queryByRole('button', { name: say('recoveryTitle') })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: say('read') })).toBeEnabled()
  })
  it('withdraws an open secret-bearing editor when authority can no longer be verified', async () => {
    mount()
    const user = await start('edit')
    await screen.findByRole('textbox', { name: say('name') })
    await user.click(screen.getByRole('combobox', { name: say('credential') }))
    await user.click(await screen.findByRole('option', { name: say('option.replace') }))
    await user.type(screen.getByLabelText(say('secret'), { exact: false }), 'withdraw-me')
    accessStatus = 404
    await act(async () => {
      await revalidate().catch(() => undefined)
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByLabelText(say('secret'), { exact: false })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: say('edit') })).not.toBeInTheDocument()
    expect(requests).toEqual([])
  })
  it('does not let an old write completion replace an editor opened in another identity', async () => {
    let finish!: () => void
    updateBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    const cache = new Map()
    const tree = (identity: string) => (
      <SWRConfig value={{ provider: () => cache, dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness identity={identity} />
      </SWRConfig>
    )
    const view = render(tree(scope))
    const user = await start('edit')
    const name = await screen.findByRole('textbox', { name: say('name') })
    await user.type(name, ' changed')
    await user.click(screen.getByRole('button', { name: say('save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    view.rerender(tree('member-b/official-team'))
    await user.click(await screen.findByRole('button', { name: say('createTitle') }))
    await user.type(screen.getByRole('textbox', { name: say('name') }), 'Other identity draft')
    await act(async () => {
      finish()
    })
    expect(screen.getByRole('dialog', { name: say('createTitle') })).toBeVisible()
    expect(screen.getByRole('textbox', { name: say('name') })).toHaveValue('Other identity draft')
    expect(requests.filter((item) => item.method === 'POST')).toHaveLength(0)
  })
  it('closes an editing draft when update is revoked while read access remains valid', async () => {
    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness preview />
      </SWRConfig>
    )
    const user = await start('edit')
    await user.type(await screen.findByRole('textbox', { name: say('name') }), ' unsaved')
    access = { read: true, create: false, update: false, delete: false }
    await act(async () => {
      await revalidate()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: say('edit') })).not.toBeInTheDocument()
    expect(requests).toEqual([])
    await waitFor(() => expect(screen.getByRole('status', { name: 'Collection preview' })).toHaveTextContent('1:Acme'))
    await user.click(screen.getByRole('button', { name: say('read') }))
    expect(await screen.findByRole('dialog', { name: say('readTitle', { name: 'Acme' }) })).toBeVisible()
  })
  it('reconciles the collection after a confirmed write and retries only the read when refresh fails', async () => {
    record = null
    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0, shouldRetryOnError: false }}>
        <Harness preview />
      </SWRConfig>
    )
    await waitFor(() => expect(screen.getByRole('status', { name: 'Collection preview' })).toHaveTextContent('0:'))
    const user = await start('createTitle')
    await fillNew(user)
    pageFailure = true
    await user.click(screen.getByRole('button', { name: say('save') }))
    await screen.findByRole('dialog', { name: say('saved') })
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(await screen.findByText(say('refreshFailed'))).toBeVisible()
    expect(screen.getByRole('status', { name: 'Collection preview' })).toHaveTextContent('unavailable')
    pageFailure = false
    await user.click(screen.getByRole('button', { name: String(i18n.t('common.retry')) }))
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'Collection preview' })).toHaveTextContent('1:New supplier')
    )
    expect(requests.filter((item) => item.method === 'POST')).toHaveLength(1)
  })
  it('edits and removes a supplier through the actual platform table and reconciles its owner-backed page', async () => {
    mountPage()
    const user = userEvent.setup()
    await chooseRowAction(user, 'editAction', 'Acme')
    const name = await screen.findByRole('textbox', { name: say('name') })
    await user.clear(name)
    await user.type(name, 'Updated supplier')
    await user.click(screen.getByRole('button', { name: say('save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await chooseRowAction(user, 'deleteAction', 'Updated supplier')
    await screen.findByText('https://upstream.example/v1')
    await waitFor(() => expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: say('confirmDelete') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(await screen.findByText(String(i18n.t('platform.suppliers.empty')))).toBeVisible()
    expect(
      screen.queryByRole('button', { name: say('moreAction', { name: 'Updated supplier' }) })
    ).not.toBeInTheDocument()
    expect(requests.find((item) => item.method === 'DELETE')?.body).toEqual({ revision: 102 })
  })
  // Catches an edit that waits for the owner before the table changes, or a pending row that hides its state.
  it('shows an edit in the table at once and keeps it once the owner accepts', async () => {
    let finish!: () => void
    updateBarrier = new Promise<void>((resolve) => {
      finish = resolve
    })
    mountPage()
    const user = userEvent.setup()
    await chooseRowAction(user, 'editAction', 'Acme')
    const name = await screen.findByRole('textbox', { name: say('name') })
    await user.clear(name)
    await user.type(name, 'Renamed supplier')
    await user.click(screen.getByRole('button', { name: say('save') }))
    const row = (await screen.findByText('Renamed supplier')).closest('tr')!
    expect(row).toHaveAttribute('aria-busy', 'true')
    expect(record?.name).toBe('Acme')
    await act(async () => finish())
    await waitFor(() => expect(row).not.toHaveAttribute('aria-busy'))
    expect(screen.getByText('Renamed supplier')).toBeVisible()
    expect(record?.name).toBe('Renamed supplier')
  })
  // Catches a refused delete that loses the row or the reader's dialog instead of restoring both.
  it('removes a row at once and restores it with the dialog when the owner refuses the delete', async () => {
    deleteBlocked = true
    mountPage()
    const user = userEvent.setup()
    await chooseRowAction(user, 'deleteAction', 'Acme')
    await screen.findByText('https://upstream.example/v1')
    await waitFor(() => expect(screen.getByRole('button', { name: say('confirmDelete') })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: say('confirmDelete') }))
    expect(await screen.findByRole('alert')).toHaveTextContent(say('conflict'))
    // The reopened modal hides the table from the accessibility tree; the restored row is behind it.
    expect(screen.getByRole('button', { name: say('moreAction', { name: 'Acme' }), hidden: true })).toBeInTheDocument()
    expect(record).not.toBeNull()
  })
  it('waits for IME completion and debounces remote search, with immediate Enter and clear', async () => {
    mountPage()
    await screen.findByRole('button', { name: say('moreAction', { name: 'Acme' }) })
    const user = userEvent.setup()
    const search = screen.getByRole('searchbox', { name: String(i18n.t('common.search')) })
    fireEvent.compositionStart(search)
    await user.type(search, 'Ac')
    await act(async () => new Promise((resolve) => setTimeout(resolve, 350)))
    expect(pageQueries).toEqual([''])
    fireEvent.compositionEnd(search, { data: 'Ac' })
    expect(pageQueries).toEqual([''])
    await waitFor(() => expect(pageQueries).toEqual(['', 'Ac']))
    await user.type(search, 'me')
    await user.keyboard('{Enter}')
    await waitFor(() => expect(pageQueries).toEqual(['', 'Ac', 'Acme']))
    await act(async () => new Promise((resolve) => setTimeout(resolve, 350)))
    expect(pageQueries).toEqual(['', 'Ac', 'Acme'])
    await user.keyboard('{Escape}')
    expect(search).toHaveValue('')
    await waitFor(() => expect(pageQueries.at(-1)).toBe(''))
  })
  it.each(['en-US', 'zh-CN'])('supports named read-only detail and hides ungranted writes in %s', async (language) => {
    await i18n.changeLanguage(language)
    access = { read: true, create: false, update: false, delete: false }
    mount()
    await start('read')
    const dialog = await screen.findByRole('dialog', { name: say('readTitle', { name: 'Acme' }) })
    expect(within(dialog).getByText('exact/model')).toBeVisible()
    expect(screen.queryByRole('button', { name: say('save') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: say('delete') })).not.toBeInTheDocument()
  })
})
