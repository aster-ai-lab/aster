import { documentResultHandoffFixture } from '@test-mocks/renderer/documentResultHandoff'
import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { workspaceFileImportFixture } from '@test-mocks/renderer/workspaceFileImport'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { StrictMode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { PERSONAL_CENTER, writeCenterId } from '@renderer/data/memberOrgs'
import { DocumentResultSaveHost } from '@renderer/pages/documents/DocumentResultSaveHost'
import { listLocalEntries } from '@renderer/pages/documents/useLocalDocuments'

import { writeOfficeDocx } from '../pdfToolsOffice'
import { OutputActions } from './OutputActions'

vi.unmock('@a3s-lab/ui')
const ROOT = '/workspace/knowledge'
let boundary: ReturnType<typeof workspaceFileImportFixture>
let bytes: Uint8Array
beforeEach(async () => {
  await i18n.changeLanguage('en-US')
  writeCenterId(PERSONAL_CENTER)
  MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', ROOT)
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/home/test' }
  })
  boundary = workspaceFileImportFixture(ROOT, [`${ROOT}/项目资料`, `${ROOT}/项目资料/导出`])
  vi.stubGlobal('fetch', boundary.fetch)
  bytes = await writeOfficeDocx('满江红\n怒发冲冠')
})
afterEach(() => {
  vi.unstubAllGlobals()
  MockUsePreferenceUtils.resetMocks()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  delete (globalThis as { __A3S_APPLET_RUNTIME__?: unknown }).__A3S_APPLET_RUNTIME__
})

function setupApplet() {
  const native = documentResultHandoffFixture()
  vi.stubGlobal('__TAURI_INTERNALS__', native)
  Object.defineProperty(globalThis, '__A3S_APPLET_RUNTIME__', {
    configurable: true,
    value: {
      apiBaseUrl: 'http://127.0.0.1:9',
      appletTicket: 'cd'.repeat(32),
      appId: 'pdf',
      tenant: 'toyou',
      permissions: []
    }
  })
  return native
}

it('saves an isolated applet result through the main Documents folder picker and acknowledges real bytes', async () => {
  setupApplet()
  render(
    <>
      <DocumentResultSaveHost />
      <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:result" />
    </>
  )
  const { user, dialog } = await openFolder()
  await user.click(within(dialog).getByRole('button', { name: 'Save here' }))
  await screen.findByText('Saved to 项目资料 / 导出')
  const listing = await listLocalEntries(ROOT, `${ROOT}/项目资料/导出`)
  expect(listing.entries.some((entry) => entry.name === 'result.docx')).toBe(true)
  expect(boundary.stored.get(`${ROOT}/项目资料/导出/result.docx`)).toEqual(bytes)
  expect(screen.getByRole('link', { name: /Download result.docx/ })).toHaveAttribute('href', 'blob:result')
})

// Catches a stuck applet after cancel or a lost native acknowledgement after the file was written.
it('cancels without writing and retries a failed acknowledgement without importing the file twice', async () => {
  const native = setupApplet()
  render(
    <StrictMode>
      <DocumentResultSaveHost />
      <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:result" />
    </StrictMode>
  )
  const cancelled = await openFolder()
  await cancelled.user.click(within(cancelled.dialog).getByRole('button', { name: 'Cancel' }))
  await waitFor(() => expect(screen.getByRole('button', { name: /Save result.docx to documents/ })).toBeEnabled())
  expect(boundary.stored.size).toBe(0)
  expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument()
  const reopened = await openFolder()
  native.refuseNextAcknowledgement()
  await reopened.user.click(within(reopened.dialog).getByRole('button', { name: 'Save here' }))
  await screen.findByText('Saved to 项目资料 / 导出')
  expect(boundary.stored.size).toBe(1)
  expect(boundary.stored.get(`${ROOT}/项目资料/导出/result.docx`)).toEqual(bytes)
  expect(screen.queryByRole('alert')).not.toBeInTheDocument()
})

// Catches startup bridge timing and losing accepted result bytes when the main window remounts.
it('receives a late native bridge and recovers an unfinished save after the main host remounts', async () => {
  const host = render(<DocumentResultSaveHost />)
  setupApplet()
  render(
    <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:result" />
  )
  const { user } = await openFolder()
  host.unmount()
  render(<DocumentResultSaveHost />)
  const recovered = await screen.findByRole('dialog', { name: 'Save to documents' })
  await user.click(await within(recovered).findByRole('button', { name: '项目资料' }))
  await user.click(await within(recovered).findByRole('button', { name: '导出' }))
  await waitFor(() => expect(within(recovered).getByRole('button', { name: 'Save here' })).toBeEnabled())
  await user.click(within(recovered).getByRole('button', { name: 'Save here' }))
  await screen.findByText('Saved to 项目资料 / 导出')
  expect(boundary.stored.get(`${ROOT}/项目资料/导出/result.docx`)).toEqual(bytes)
})

// Catches an expired native request permanently blocking the main window's next save picker.
it('keeps download after an expired request and admits a new result save', async () => {
  const native = setupApplet()
  const view = render(
    <>
      <DocumentResultSaveHost />
      <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:old" />
    </>
  )
  const expired = await openFolder()
  native.expireCurrentRequest()
  await expired.user.click(within(expired.dialog).getByRole('button', { name: 'Save here' }))
  await screen.findByRole('alert')
  expect(screen.getByRole('link', { name: /Download result.docx/ })).toHaveAttribute('href', 'blob:old')
  expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument()
  view.rerender(
    <>
      <DocumentResultSaveHost />
      <OutputActions
        file={{ name: 'next.docx', mediaType: 'application/octet-stream', bytes: bytes.slice() }}
        url="blob:new"
      />
    </>
  )
  const next = await openFolder()
  await next.user.click(within(next.dialog).getByRole('button', { name: 'Save here' }))
  await screen.findByText('Saved to 项目资料 / 导出')
  expect(boundary.stored.get(`${ROOT}/项目资料/导出/next.docx`)).toEqual(bytes)
})

async function openFolder() {
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: /Save .* to documents/ }))
  const dialog = await screen.findByRole('dialog', { name: 'Save to documents' })
  await user.click(await within(dialog).findByRole('button', { name: '项目资料' }))
  await user.click(await within(dialog).findByRole('button', { name: '导出' }))
  await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Save here' })).toBeEnabled())
  return { user, dialog }
}

describe('save converted results to Knowledge documents', () => {
  // Catches a different destination tree, truncated output bytes and premature success before persistence.
  it('writes exact Office bytes to a selected nested folder, waits for acknowledgment and exposes them in the document listing', async () => {
    render(
      <OutputActions
        file={{
          name: 'result.docx',
          mediaType: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
          bytes
        }}
        url="blob:result"
      />
    )
    expect(boundary.stored.size).toBe(0)
    const download = screen.getByRole('link', { name: /Download result.docx/ })
    const { user, dialog } = await openFolder()
    const release = boundary.holdNext()
    await user.click(within(dialog).getByRole('button', { name: 'Save here' }))
    expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument()
    expect(download).toHaveAttribute('download', 'result.docx')
    release()
    await screen.findByText('Saved to 项目资料 / 导出')
    expect(boundary.stored.get(`${ROOT}/项目资料/导出/result.docx`)).toEqual(bytes)
    const listing = await listLocalEntries(ROOT, `${ROOT}/项目资料/导出`)
    expect(listing.entries).toContainEqual(
      expect.objectContaining({ name: 'result.docx', kind: 'file', sizeBytes: bytes.byteLength })
    )
  })

  // Catches lost results after refusal and accidental replacement of an existing Knowledge file.
  it('keeps download and chosen folder after a refused write, supports retry and refuses duplicate names', async () => {
    render(
      <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:result" />
    )
    const download = screen.getByRole('link', { name: /Download result.docx/ })
    const { user, dialog } = await openFolder()
    boundary.refuseNext(403)
    await user.click(within(dialog).getByRole('button', { name: 'Save here' }))
    await within(dialog).findByRole('alert')
    expect(boundary.stored.size).toBe(0)
    expect(download).toHaveAttribute('href', 'blob:result')
    await user.click(within(dialog).getByRole('button', { name: 'Save here' }))
    await screen.findByText('Saved to 项目资料 / 导出')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    const reopened = await openFolder()
    await reopened.user.click(within(reopened.dialog).getByRole('button', { name: 'Save here' }))
    await within(reopened.dialog).findByText('A file with this name already exists. Choose another folder.')
    expect(boundary.stored.get(`${ROOT}/项目资料/导出/result.docx`)).toEqual(bytes)
  })

  // Catches a late response marking a newer conversion revision as saved.
  it('withdraws an old save dialog and success state when result bytes change', async () => {
    const view = render(
      <OutputActions file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes }} url="blob:old" />
    )
    const { user, dialog } = await openFolder()
    const release = boundary.holdNext()
    await user.click(within(dialog).getByRole('button', { name: 'Save here' }))
    view.rerender(
      <OutputActions
        file={{ name: 'result.docx', mediaType: 'application/octet-stream', bytes: new Uint8Array([9]) }}
        url="blob:new"
      />
    )
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await act(async () => {
      release()
      await Promise.resolve()
    })
    expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: /Download result.docx/ })).toHaveAttribute('href', 'blob:new')
  })
})
