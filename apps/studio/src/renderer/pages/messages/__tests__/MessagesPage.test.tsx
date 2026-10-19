import * as ui from '@a3s-lab/ui'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import { dataApiService } from '@data/DataApiService'
import { publishMemberProfile } from '@renderer/components/account/memberProfile'
import { BossRoomRuntime } from '@renderer/components/app/BossRoomRuntime'
import { requestOpenMessage } from '@renderer/components/app/messageAttentionHost'
import type * as memberOrgs from '@renderer/data/memberOrgs'
import type { MemberAccount } from '@renderer/data/memberOrgs'
import { encodeChatRecord } from '@renderer/im/forwardRecord'
import { ImApiError, type ImMember } from '@renderer/im/imApi'
import { BUILTIN_TOBOT_ID, __resetSharedImStore, ImStore } from '@renderer/im/imStore'
import type { StreamHandlers } from '@renderer/im/imStream'
import { RECALL_WINDOW_MS } from '@renderer/im/imView'
import { decodeBossWire, encodeBossWire } from '@renderer/im/quote'
import { TOBOSS_PROFILE_KEY } from '@renderer/im/tobotProfile'
import { listOrgMembers } from '@renderer/pages/org/orgApi'
import {
  BOSS_GAP_MS,
  BOSS_QUEUE_CAP,
  bossRoomPrompt,
  offerSharedBossJob,
  resetBossRuntimeForTests,
  type BossJob
} from '@renderer/services/bossRoom'
import { resetTobotStreamForTests } from '@renderer/services/tobotStream'

import {
  DIRECT,
  detail,
  entry,
  fakeApi,
  type FakeApi,
  GROUP,
  ME,
  message,
  ORG,
  PEER,
  THIRD,
  TRANSFER
} from '../../../im/__tests__/fakeImApi'
import { FileCard, MessageFileMark, ThinkingIndicator } from '../MessageBody'
import MessagesPage from '../MessagesPage'
import type * as tobotFiles from '../tobotFiles'

const closedMailbox = {
  turnContext: {
    directives: [
      'ToBot is not allowed to manage these tasks. Do not use member-work tools or invent tasks.',
      'ToBot is not allowed to manage this mailbox. Do not use member-mail tools or invent messages.',
      'ToBot is not allowed to use the member phone. Do not invent a phone number, place a call, or send an SMS. ToBot is not allowed to manage the wallet. Do not invent a balance, pay, transfer, or change money.'
    ]
  }
}

const harnessComponents = ['system', 'tools', 'budget', 'compact', 'infer']

const tobotUpload = vi.hoisted(() => ({ uploadTobotFile: vi.fn(), readAgentFile: vi.fn() }))

vi.mock('../tobotFiles', async (importOriginal) => {
  const actual = await importOriginal<typeof tobotFiles>()
  return {
    ...actual,
    uploadTobotFile: tobotUpload.uploadTobotFile,
    readAgentFile: tobotUpload.readAgentFile
  }
})

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      options && 'count' in options ? `${key}:${options.count}` : key,
    i18n: { language: 'zh-CN' }
  })
}))

const account = vi.hoisted(() => ({
  logoutMemberAccount: vi.fn(async () => undefined),
  returnToLogin: vi.fn(),
  readMemberAccount: vi.fn<() => Promise<MemberAccount>>(async () => ({ displayName: '', phone: '' }))
}))

const memberOrgsApi = vi.hoisted(() => ({
  listMemberOrgs: vi.fn(async () => [] as Array<{ id: string; name: string; logo: string; creator: boolean }>)
}))

vi.mock('@renderer/pages/org/orgApi', () => ({
  listOrgMembers: vi.fn(async () => [])
}))

vi.mock('@renderer/data/memberOrgs', async (importOriginal) => ({
  ...(await importOriginal<typeof memberOrgs>()),
  ...account,
  listMemberOrgs: memberOrgsApi.listMemberOrgs
}))

let api: FakeApi

function mount(
  options: {
    runtime?: boolean
    org?: string
    configure?: (api: FakeApi) => void
    capture?: { emit: (event: string, data: Record<string, unknown>) => void }
    stream?: { current: StreamHandlers | null }
  } = {}
) {
  globalThis.localStorage.setItem('a3s.org.center', options.org ?? ORG)
  api = fakeApi()
  options.configure?.(api)
  const store = new ImStore({
    createApi: () => api,
    runtimeAvailable: () => options.runtime ?? true,
    openStream: (_api, _device, handlers) => {
      if (options.stream) options.stream.current = handlers
      if (options.capture) {
        options.capture.emit = (event, data) => handlers.onFrame({ event, data: JSON.stringify(data) })
      }
      return () => undefined
    },
    deviceId: () => 'studio-test',
    isVisible: () => true,
    createObjectUrl: () => 'blob:attachment',
    revokeObjectUrl: () => undefined,
    syncDelayMs: 0
  })
  __resetSharedImStore(store)
  render(<MessagesPage />)
  return store
}

async function openDirect() {
  const row = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
  fireEvent.click(row)
  await screen.findByText('hello 1')
}

function previewOf(conversationId: string): globalThis.HTMLElement {
  const row = globalThis.document.querySelector(`[data-conversation-id="${conversationId}"]`)
  const preview = row?.querySelector('[data-ui="messages.conversation-preview"]')
  if (!preview) throw new Error(`missing preview ${conversationId}`)
  return preview as globalThis.HTMLElement
}

/** One direct chat and one group, each served by the real test transport. */
function withDirectAndGroup(fake: FakeApi, group: Partial<ReturnType<typeof entry>> = {}) {
  fake.conversations.mockResolvedValue([
    entry(),
    entry({
      id: GROUP,
      kind: 'group',
      title: '项目组',
      peerUserId: null,
      memberCount: 3,
      lastSequence: 1,
      lastReadSequence: 1,
      unread: 0,
      updatedAt: Date.parse('2026-10-02T07:00:00Z'),
      preview: {
        kind: 'text',
        messageId: `m-${GROUP}-1`,
        sequence: 1,
        recalled: false,
        createdAt: Date.parse('2026-10-02T09:00:00Z'),
        sender: { kind: 'member', id: PEER },
        text: 'group hello',
        name: null,
        code: null,
        params: {}
      },
      ...group
    })
  ])
  fake.detail.mockImplementation(async (conversationId: string) =>
    conversationId === GROUP
      ? detail({
          id: GROUP,
          kind: 'group',
          title: '项目组',
          memberCount: 3,
          members: [
            { userId: ME, role: 'owner', left: false, lastReadSequence: 1 },
            { userId: PEER, role: 'member', left: false, lastReadSequence: 1 },
            { userId: THIRD, role: 'member', left: false, lastReadSequence: 1 }
          ]
        })
      : detail()
  )
  fake.messages.mockImplementation(async (conversationId: string) =>
    conversationId === GROUP
      ? [message(1, { conversationId: GROUP, body: { kind: 'text', text: 'group hello' } })]
      : [message(2), message(1)]
  )
}

async function openGroup() {
  fireEvent.click(await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' }))
  await screen.findByText('group hello')
}

async function sendDraft(text: string) {
  fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
    target: { value: text }
  })
  fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
}

describe('MessagesPage', () => {
  beforeEach(async () => {
    const { Calendar } = await vi.importActual<typeof ui>('@a3s-lab/ui')
    vi.spyOn(ui, 'Calendar').mockImplementation(Calendar)
    globalThis.localStorage.clear()
    const { clearCachedPortraits } = await import('../messagePortraitCache')
    clearCachedPortraits()
    publishMemberProfile(null)
    account.readMemberAccount.mockReset().mockResolvedValue({ displayName: '', phone: '' })
    vi.mocked(listOrgMembers).mockReset().mockResolvedValue([])
    memberOrgsApi.listMemberOrgs.mockReset().mockResolvedValue([])
    vi.spyOn(dataApiService, 'get').mockRejectedValue(new Error('offline'))
    vi.spyOn(dataApiService, 'post').mockRejectedValue(new Error('offline'))
  })
  afterEach(() => {
    cacheService.setPersist('ui.emoji.recently_used', [])
    __resetSharedImStore()
    resetBossRuntimeForTests()
    resetTobotStreamForTests()
    vi.restoreAllMocks()
    Reflect.deleteProperty(globalThis.window, 'api')
  })

  it('shows a sign-in state instead of demo data outside the desktop shell', () => {
    mount({ runtime: false })
    expect(globalThis.document.querySelector('[data-ui="messages.state"]')).toHaveAttribute(
      'data-status',
      'unavailable'
    )
    expect(globalThis.document.querySelector('[data-ui="messages.conversation"]')).toBeNull()
    expect(screen.getByText('messages.state.unavailable_title')).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: 'messages.compose_placeholder' })).toBeNull()
  })

  it('shows the full name, the latest ToBot line and its time on a minimum-height row', async () => {
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          {
            id: 'assistant-1',
            role: 'assistant',
            status: 'success',
            searchableText: 'ignored',
            createdAt: '2026-10-02T08:00:02.000Z',
            data: { parts: [{ type: 'text', text: '## 核心能力\n\n**代码开发**' }] }
          }
        ]
      } as never
    })
    mount({ org: 'personal' })
    const row = await waitFor(() => {
      const found = globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)
      expect(found?.querySelector('[data-ui="messages.conversation-preview"]')).toHaveTextContent('核心能力 代码开发')
      return found as globalThis.HTMLElement
    })
    expect(row.className).toContain('min-h-[68px]')
    const title = row.querySelector('[data-ui="messages.conversation-title"]')
    expect(title).toHaveTextContent('messages.file_transfer')
    expect(title?.className).toContain('min-w-0')
    expect(title?.className).toContain('truncate')
    expect(title).toHaveAttribute('title', 'messages.file_transfer')
    expect(row.querySelector('[data-ui="messages.conversation-time"]')).toHaveTextContent('10/02')
  })

  it('opens the built-in ToBot thread in the personal center', async () => {
    mount({ org: 'personal' })
    const row = globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)
    expect(row).toHaveTextContent('messages.file_transfer')
    expect(row?.querySelector('[data-a3s-bot-avatar="true"]')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'messages.action.new' })).toBeNull()
    expect(api.conversations).not.toHaveBeenCalled()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    const start = await waitFor(() => {
      const found = globalThis.document.querySelector('[data-ui="messages.thread-start"]')
      expect(found).toBeTruthy()
      return found as globalThis.HTMLElement
    })
    expect(start).toHaveTextContent('messages.thread_start_transfer')
    const heading = screen.getByRole('heading', { level: 1 })
    expect(heading).toHaveTextContent('messages.file_transfer')
    expect(heading).toHaveAttribute('title', 'messages.file_transfer')
    expect(heading.className).toContain('flex-1')
    expect(heading.querySelector('.truncate')).toBeNull()
    expect(globalThis.document.querySelector('[data-ui="messages.state"]')).toBeNull()
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'messages.leave' })).toBeNull()
  })

  it('sends a personal ToBot message to the long-lived a3s-code session', async () => {
    let sent = false
    let turnBody: unknown
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return (
        sent
          ? {
              items: [
                {
                  id: 'assistant-1',
                  role: 'assistant',
                  status: 'success',
                  searchableText: 'agent says hi',
                  data: { parts: [{ type: 'text', text: 'agent says hi' }] }
                },
                {
                  id: 'user-1',
                  role: 'user',
                  status: 'success',
                  searchableText: 'hello bot',
                  data: { parts: [{ type: 'text', text: 'hello bot' }] }
                }
              ]
            }
          : { items: [] }
      ) as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path === '/agent-sessions/tobot/turns') {
        sent = true
        turnBody = options
      }
      return { mode: 'started' } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: 'hello bot' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const reply = await within(
      globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    ).findByText('agent says hi')
    const replyTurn = reply.closest('[data-ui="messages.tobot-turn"]')
    expect(replyTurn).toHaveAttribute('data-agent-streaming', 'false')
    expect(reply.closest('[data-ui="messages.bubble"]')).toBeNull()
    expect(replyTurn?.parentElement).toHaveClass('w-full', 'max-w-[70%]', 'items-start')
    const mine = screen.getByText('hello bot', { selector: '[data-ui="messages.bubble"]' })
    expect(mine).toHaveClass('bg-[#171717]')
    expect(mine.querySelector('[data-ui="messages.bubble-tail"]')).toHaveAttribute('data-tail', 'end')
    expect(mine.querySelector('[data-ui="messages.bubble-tail"]')?.className).toContain('-right-[6px]')
    expect(mine.querySelector('[data-ui="messages.bubble-tail"]')?.className).toContain('border-l-[#171717]')
    const writeText = vi.fn().mockResolvedValue(undefined)
    const previousClipboard = Object.getOwnPropertyDescriptor(globalThis.navigator, 'clipboard')
    Object.defineProperty(globalThis.navigator, 'clipboard', { configurable: true, value: { writeText } })
    try {
      fireEvent.contextMenu(reply)
      const menu = globalThis.document.querySelector('[data-ui="messages.bubble-menu"]') as globalThis.HTMLElement
      expect(within(menu).getByRole('menuitem', { name: 'common.copy' }).querySelector('svg')).toBeTruthy()
      expect(within(menu).queryByRole('menuitem', { name: 'messages.recall_action' })).toBeNull()
      expect(within(menu).queryByRole('menuitem', { name: 'messages.save_finding' })).toBeNull()
      fireEvent.click(within(menu).getByRole('menuitem', { name: 'common.copy' }))
      await waitFor(() => expect(writeText).toHaveBeenCalledWith('agent says hi'))
    } finally {
      if (previousClipboard) Object.defineProperty(globalThis.navigator, 'clipboard', previousClipboard)
      else delete (globalThis.navigator as { clipboard?: globalThis.Clipboard }).clipboard
    }
    expect(turnBody).toEqual({
      body: { userMessageParts: [{ type: 'text', text: 'hello bot' }], ...closedMailbox, harnessComponents }
    })
    expect(api.send).not.toHaveBeenCalled()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'messages.action.file' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'messages.action.image' })).toBeEnabled()
  })

  it('stages a file and an image until the user sends them with a caption', async () => {
    let sent = false
    let turnBody: unknown
    vi.spyOn(globalThis.URL, 'createObjectURL').mockReturnValue('blob:shot')
    tobotUpload.uploadTobotFile.mockImplementation(async (file: globalThis.File) => ({
      type: 'file' as const,
      url: `file:///tmp/a3s-studio-composer/id_${file.name}`,
      filename: file.name,
      mediaType: file.type || 'application/octet-stream',
      sizeBytes: file.size
    }))
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return (
        sent
          ? {
              items: [
                {
                  id: 'assistant-file',
                  role: 'assistant',
                  status: 'success',
                  searchableText: 'got the file',
                  data: { parts: [{ type: 'text', text: 'got the file' }] }
                },
                {
                  id: 'user-file',
                  role: 'user',
                  status: 'success',
                  searchableText: 'see this',
                  data: {
                    parts: [
                      {
                        type: 'file',
                        url: 'file:///tmp/a3s-studio-composer/id_shot.png',
                        filename: 'shot.png',
                        mediaType: 'image/png',
                        sizeBytes: 4
                      },
                      { type: 'text', text: 'see this' }
                    ]
                  }
                }
              ]
            }
          : { items: [] }
      ) as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path === '/agent-sessions/tobot/turns') {
        sent = true
        turnBody = options
      }
      return { mode: 'started' } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    const composer = await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    const image = new globalThis.File(['abcd'], 'shot.png', { type: 'image/png' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.image-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [image] }
      }
    )
    const pending = await waitFor(() => {
      const tray = globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')
      expect(tray).toBeTruthy()
      return tray as globalThis.HTMLElement
    })
    expect(pending.querySelector('img')).toHaveAttribute('src', 'blob:shot')
    expect(pending.querySelector('img')?.className).toContain('size-[72px]')
    expect(within(pending).getByRole('button', { name: 'common.close' })).toBeInTheDocument()
    fireEvent.click(within(pending).getByRole('button', { name: 'shot.png' }))
    const shotPreview = await screen.findByTestId('image-preview-dialog')
    expect(shotPreview.querySelector('img')).toHaveAttribute('src', 'blob:shot')
    fireEvent.click(within(shotPreview).getByRole('button', { name: 'preview.close' }))
    expect(screen.queryByTestId('image-preview-dialog')).not.toBeInTheDocument()
    expect(tobotUpload.uploadTobotFile).not.toHaveBeenCalled()
    expect(turnBody).toBeUndefined()
    fireEvent.change(composer, { target: { value: 'see this' } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(turnBody).toEqual({
        body: {
          userMessageParts: [
            {
              type: 'file',
              url: 'file:///tmp/a3s-studio-composer/id_shot.png',
              filename: 'shot.png',
              mediaType: 'image/png',
              sizeBytes: 4
            },
            { type: 'text', text: 'see this' }
          ],
          ...closedMailbox,
          harnessComponents
        }
      })
    )
    const thread = globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    expect(await within(thread).findByRole('img', { name: 'shot.png' })).toHaveAttribute('src', 'blob:shot')
    expect(await within(thread).findByText('got the file')).toBeInTheDocument()
    expect(api.upload).not.toHaveBeenCalled()
    expect(api.send).not.toHaveBeenCalled()
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toBeNull()

    sent = false
    const note = new globalThis.File(['abcd'], 'note.txt', { type: 'text/plain' })
    fireEvent.paste(composer, { clipboardData: { files: [note] } })
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toHaveTextContent('note.txt')
    expect(tobotUpload.uploadTobotFile).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(turnBody).toEqual({
        body: {
          userMessageParts: [
            {
              type: 'file',
              url: 'file:///tmp/a3s-studio-composer/id_note.txt',
              filename: 'note.txt',
              mediaType: 'text/plain',
              sizeBytes: 4
            }
          ],
          ...closedMailbox,
          harnessComponents
        }
      })
    )
    expect(await screen.findByRole('button', { name: 'note.txt' })).toHaveAttribute('data-ui', 'messages.file')
  })

  it('rejects an empty ToBot attachment before the agent turn', async () => {
    tobotUpload.uploadTobotFile.mockClear()
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [new globalThis.File([], 'empty.txt')] }
      }
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('messages.error.empty_file')
    expect(tobotUpload.uploadTobotFile).not.toHaveBeenCalled()
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toBeNull()
  })

  it('sends a quoted image to ToBot as a file the agent can read', async () => {
    vi.spyOn(globalThis.URL, 'createObjectURL').mockReturnValue('blob:quoted')
    tobotUpload.readAgentFile.mockResolvedValue(new globalThis.Blob(['abcd'], { type: 'image/png' }))
    tobotUpload.uploadTobotFile.mockImplementation(async (file: globalThis.File) => ({
      type: 'file' as const,
      url: `file:///tmp/a3s-studio-composer/id_${file.name}`,
      filename: file.name,
      mediaType: file.type || 'image/png',
      sizeBytes: file.size
    }))
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          {
            id: 'user-shot',
            role: 'user',
            status: 'success',
            searchableText: '',
            createdAt: '2026-10-06T10:00:00.000Z',
            data: {
              parts: [
                {
                  type: 'file',
                  url: 'file:///tmp/a3s-studio-composer/id_shot.png',
                  filename: 'shot.png',
                  mediaType: 'image/png',
                  sizeBytes: 4
                }
              ]
            }
          }
        ]
      } as never
    })
    let turnBody: { body: { userMessageParts: Array<{ type: string; filename?: string; text?: string }> } } | undefined
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path === '/agent-sessions/tobot/turns') {
        turnBody = options as typeof turnBody
      }
      return { mode: 'started' } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    fireEvent.contextMenu(await screen.findByRole('img', { name: 'shot.png' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.quote' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '看不见图片？' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(turnBody?.body.userMessageParts.some((part) => part.type === 'file')).toBe(true))
    const parts = turnBody?.body.userMessageParts ?? []
    expect(parts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ type: 'file', filename: 'shot.png', mediaType: 'image/png' }),
        expect.objectContaining({ type: 'text', text: expect.stringContaining('看不见图片？') })
      ])
    )
    expect(tobotUpload.readAgentFile).toHaveBeenCalledWith('/tmp/a3s-studio-composer/id_shot.png')
  })

  it('sends an emoji after ToBot finishes the turn that rejected it', async () => {
    let turnPosts = 0
    let turnReads = 0
    let turnBody: unknown
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      const text = String(path)
      if (text.endsWith('/turn')) {
        turnReads += 1
        return { status: turnReads === 1 ? 'attached' : 'not-found' } as never
      }
      if (!text.endsWith('/messages')) return {} as never
      return { items: [] } as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path !== '/agent-sessions/tobot/turns') return { mode: 'started' } as never
      turnPosts += 1
      if (turnPosts === 1) throw new Error('Load failed')
      turnBody = options
      return { mode: 'started' } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    const composer = await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(composer, { target: { value: '😂' } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(turnPosts).toBe(2), { timeout: 3000 })
    expect(turnBody).toEqual({
      body: { userMessageParts: [{ type: 'text', text: '😂' }], ...closedMailbox, harnessComponents }
    })
    expect(screen.queryByRole('alert')).toBeNull()
    expect(composer).toHaveValue('')
  })

  it('offers retry when the inbox fails to load', async () => {
    mount({ configure: (fake) => fake.conversations.mockRejectedValueOnce(new Error('upstream down')) })
    const state = await waitFor(() => {
      const found = globalThis.document.querySelector('[data-ui="messages.state"][data-status="error"]')
      expect(found).toBeTruthy()
      return found!
    })
    expect(state).toHaveTextContent('messages.state.error_title')
    fireEvent.click(screen.getByRole('button', { name: 'messages.retry' }))
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
  })

  it('sends a denied member session back to sign in instead of reporting a load failure', async () => {
    mount({ configure: (fake) => fake.me.mockRejectedValueOnce(new ImApiError('member session denied', 401)) })
    const state = await waitFor(() => {
      const found = globalThis.document.querySelector('[data-ui="messages.state"][data-status="signed-out"]')
      expect(found).toBeTruthy()
      return found!
    })
    expect(state).toHaveTextContent('messages.state.signed_out_title')
    expect(state).not.toHaveTextContent('messages.state.error_title')
    fireEvent.click(screen.getByRole('button', { name: 'messages.state.sign_in_again' }))
    await waitFor(() => expect(account.returnToLogin).toHaveBeenCalled())
    expect(account.logoutMemberAccount).toHaveBeenCalled()
  })

  it('keeps the personal ToBot out of a team inbox', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry(),
          entry({ id: TRANSFER, kind: 'transfer', title: null, peerUserId: null, memberCount: 1, preview: null })
        ])
      }
    })
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)).toBeNull()
    expect(api.fileTransfer).not.toHaveBeenCalled()
  })

  it('shows an empty team inbox without a built-in ToBot', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([])
        fake.fileTransfer.mockRejectedValueOnce(new Error('down'))
      }
    })
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.empty"]')).toBeTruthy())
    expect(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()
    expect(api.fileTransfer).not.toHaveBeenCalled()
  })

  it('shows a clearable no-results state for searches', async () => {
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.change(screen.getByPlaceholderText('messages.search_placeholder'), { target: { value: 'zzz' } })
    const noResults = globalThis.document.querySelector('[data-ui="messages.no-results"]') as globalThis.HTMLElement
    expect(noResults).toHaveTextContent('messages.no_results')
    fireEvent.click(within(noResults).getByRole('button', { name: 'messages.clear_search' }))
    expect(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)).toBeTruthy()
  })

  it('asks to pick a conversation, then opens a new chat at its beginning', async () => {
    mount({ configure: (fake) => fake.messages.mockResolvedValue([]) })
    const row = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    expect(globalThis.document.querySelector('[data-ui="messages.thread-empty"]')).toHaveTextContent(
      'messages.pick_conversation_hint'
    )
    fireEvent.click(row)
    const start = await waitFor(() => {
      const found = globalThis.document.querySelector('[data-ui="messages.thread-start"]')
      expect(found).toBeTruthy()
      return found!
    })
    expect(start).toHaveTextContent('messages.thread_start_direct')
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeEnabled()
  })

  it('lists real conversations with peer names, previews and unread counts', async () => {
    mount()
    const row = await waitFor(() => {
      const found = globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)
      expect(found).toBeTruthy()
      return found!
    })
    expect(row).toHaveTextContent('周予安')
    expect(row).toHaveTextContent('hello 2')
    expect(row.className).toContain('min-h-[68px]')
    const title = row.querySelector('[data-ui="messages.conversation-title"]')
    expect(title).toHaveTextContent('周予安')
    expect(title?.className).toContain('min-w-0')
    expect(title?.className).toContain('truncate')
    expect(title).toHaveAttribute('title', '周予安')
    expect(row.querySelector('[data-ui="messages.conversation-preview"]')).toHaveTextContent('hello 2')
    expect(row.querySelector('[data-ui="messages.conversation-time"]')).toHaveTextContent('10/02')
    expect(row.querySelector('[data-ui="messages.unread-count"]')).toHaveTextContent('1')
    const avatar = row.querySelector('[data-ui="messages.person-avatar"][data-kind="text"]')
    expect(avatar).toHaveTextContent('周予')
    expect(screen.queryByRole('button', { name: 'messages.action.call' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'messages.action.screenshot' })).toBeNull()
  })

  it('shows the saved account photo on my messages and a directory photo on a peer', async () => {
    const mine = 'data:image/svg+xml;base64,TUlORQ'
    const peer = 'data:image/png;base64,UEVFUg'
    account.readMemberAccount.mockResolvedValue({
      displayName: 'RoyLin',
      phone: '18770221825',
      avatar: mine
    })
    vi.mocked(listOrgMembers).mockResolvedValue([
      { id: PEER, name: '周予安', phone: '13900000000', avatar: peer }
    ] as never)
    mount()
    const title = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    const row = title.closest('[data-ui="messages.conversation"]')
    if (!row) throw new Error('The colleague conversation must exist')
    await waitFor(() => {
      const mark = row?.querySelector('[data-ui="messages.person-avatar"][data-kind]')
      expect(mark).toHaveAttribute('data-kind', 'image')
      expect(mark?.querySelector('img')).toHaveAttribute('src', peer)
    })
    api.messages.mockResolvedValueOnce([
      message(2, { sender: { kind: 'member', id: ME }, body: { kind: 'text', text: 'mine-line' } })
    ])
    fireEvent.click(row)
    await screen.findByText('mine-line')
    await waitFor(() => {
      const images = [
        ...globalThis.document.querySelectorAll('[data-ui="messages.row"] [data-ui="messages.person-avatar"] img')
      ]
      expect(images.some((img) => img.getAttribute('src') === mine)).toBe(true)
    })
    expect(globalThis.document.body).not.toHaveTextContent('1825')
  })

  it('opens a thread, marks it read and shows read state for my messages', async () => {
    mount()
    api.messages.mockResolvedValueOnce([message(2, { sender: { kind: 'member', id: ME } }), message(1)])
    const row = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.click(row)
    await screen.findByText('hello 1')
    const heading = screen.getByRole('heading', { level: 1 })
    expect(heading).toHaveTextContent('周予安')
    expect(heading).toHaveAttribute('title', '周予安')
    expect(heading.className).toContain('flex-1')
    expect(api.markRead).toHaveBeenCalledWith(DIRECT)
    await waitFor(() =>
      expect(globalThis.document.querySelector('[data-ui="messages.status"]')).toHaveTextContent('messages.status.read')
    )
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.unread-count"]')).toBeNull())
  })

  it('sends text through the API and shows the server message', async () => {
    mount()
    await openDirect()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: 'hello thread' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(api.send).toHaveBeenCalledWith(DIRECT, expect.stringMatching(/^studio-/), {
        kind: 'text',
        text: 'hello thread'
      })
    )
    const bubble = await screen.findByText('hello thread', { selector: '[data-ui="messages.bubble"]' })
    expect(bubble).toHaveClass('bg-[#171717]')
    expect(bubble.querySelector('[data-ui="messages.bubble-tail"]')).toHaveAttribute('data-tail', 'end')
    const incoming = screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' })
    expect(incoming.querySelector('[data-ui="messages.bubble-tail"]')).toHaveAttribute('data-tail', 'start')
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toHaveValue('')
  })

  it('copies a bubble from the menu and recalls only my own message', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    const previous = Object.getOwnPropertyDescriptor(globalThis.navigator, 'clipboard')
    Object.defineProperty(globalThis.navigator, 'clipboard', { configurable: true, value: { writeText } })
    try {
      mount()
      await openDirect()
      const incoming = await screen.findByText('hello 1', { selector: '[data-ui="messages.bubble"]' })
      fireEvent.contextMenu(incoming)
      const incomingMenu = globalThis.document.querySelector(
        '[data-ui="messages.bubble-menu"]'
      ) as globalThis.HTMLElement
      expect(incomingMenu).toBeTruthy()
      expect(globalThis.document.querySelector('[data-ui="messages.context-menu"]')).toBeNull()
      const copy = within(incomingMenu).getByRole('menuitem', { name: 'common.copy' })
      expect(copy.querySelector('svg')).toBeTruthy()
      expect(within(incomingMenu).queryByRole('menuitem', { name: 'messages.recall_action' })).toBeNull()
      expect(within(incomingMenu).queryByRole('menuitem', { name: 'messages.save_finding' })).toBeNull()
      expect(within(incomingMenu).getByRole('menuitem', { name: 'messages.delete' }).querySelector('svg')).toBeTruthy()
      fireEvent.keyDown(globalThis.document, { key: 'Escape' })
      expect(globalThis.document.querySelector('[data-ui="messages.bubble-menu"]')).toBeNull()

      fireEvent.contextMenu(incoming)
      fireEvent.click(
        within(
          globalThis.document.querySelector('[data-ui="messages.bubble-menu"]') as globalThis.HTMLElement
        ).getByRole('menuitem', {
          name: 'common.copy'
        })
      )
      await waitFor(() => expect(writeText).toHaveBeenCalledWith('hello 1'))
      expect(globalThis.document.querySelector('[data-ui="messages.bubble-menu"]')).toBeNull()

      writeText.mockRejectedValueOnce(new Error('denied'))
      fireEvent.contextMenu(incoming)
      fireEvent.click(screen.getByRole('menuitem', { name: 'common.copy' }))
      expect(await screen.findByRole('alert')).toHaveTextContent('common.copy_failed')

      fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
        target: { value: 'hello thread' }
      })
      fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
      await waitFor(() => {
        expect(api.send).toHaveBeenCalled()
        expect(screen.getAllByText('hello thread', { selector: '[data-ui="messages.bubble"]' })).toHaveLength(1)
      })
      const mine = screen.getByText('hello thread', { selector: '[data-ui="messages.bubble"]' })
      fireEvent.contextMenu(mine)
      const recall = within(
        globalThis.document.querySelector('[data-ui="messages.bubble-menu"]') as globalThis.HTMLElement
      ).getByRole('menuitem', { name: 'messages.recall_action' })
      expect(recall.querySelector('svg')).toBeTruthy()
      fireEvent.click(recall)
      await waitFor(() => expect(api.recall).toHaveBeenCalledWith(`m-${DIRECT}-3`))
      await waitFor(() =>
        expect([...globalThis.document.querySelectorAll('[data-ui="messages.row"]')].at(-1)).toHaveTextContent(
          'messages.recall_self'
        )
      )
      expect(globalThis.document.querySelector('[data-ui="messages.bubble-menu"]')).toBeNull()

      fireEvent.contextMenu(incoming)
      fireEvent.click(screen.getByRole('menuitem', { name: 'messages.delete' }))
      await waitFor(() => expect(screen.queryByText('hello 1', { selector: '[data-ui="messages.bubble"]' })).toBeNull())
      expect(api.recall).not.toHaveBeenCalledWith(`m-${DIRECT}-1`)
      expect(screen.getByText('hello 2', { selector: '[data-ui="messages.bubble"]' })).toBeTruthy()
    } finally {
      globalThis.localStorage.removeItem('messages.dismissed.v1')
      if (previous) Object.defineProperty(globalThis.navigator, 'clipboard', previous)
      else delete (globalThis.navigator as { clipboard?: globalThis.Clipboard }).clipboard
    }
  })

  it('uses the document icon for spreadsheet and word files', () => {
    const { unmount } = render(<FileCard name="预算.xlsx" sizeBytes={1024} onOpen={() => undefined} />)
    const card = globalThis.document.querySelector('[data-ui="messages.file"]') as globalThis.HTMLElement
    const icon = card.querySelector('[data-file-icon]')
    expect(icon).toHaveAttribute('data-file-icon', 'excel')
    const mark = icon?.querySelector('svg')
    expect(mark).toHaveClass('h-9', 'w-8')
    const name = within(card).getByText('预算.xlsx')
    if (!mark) throw new Error('The document icon must exist')
    expect(mark.compareDocumentPosition(name) & globalThis.Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    unmount()
    render(<MessageFileMark name="说明.docx" />)
    expect(globalThis.document.querySelector('[data-file-icon]')).toHaveAttribute('data-file-icon', 'word')
  })

  it('uploads a picked file and recalls my own message', async () => {
    mount()
    await openDirect()
    const input = globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement
    const file = new globalThis.File(['abc'], 'report.pdf', { type: 'application/pdf' })
    fireEvent.change(input, { target: { files: [file] } })
    const pending = globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')
    expect(pending).toHaveTextContent('report.pdf')
    expect(pending?.querySelector('[data-file-icon]')).toHaveAttribute('data-file-icon', 'pdf')
    expect(api.upload).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.upload).toHaveBeenCalledWith(file, 'report.pdf'))
    const card = await screen.findByRole('button', { name: 'report.pdf' })
    expect(card).toHaveAttribute('data-ui', 'messages.file')
    expect(card.querySelector('[data-file-icon]')).toHaveAttribute('data-file-icon', 'pdf')

    fireEvent.click(screen.getByRole('button', { name: 'messages.recall_action' }))
    await waitFor(() => expect(api.recall).toHaveBeenCalled())
    await waitFor(() =>
      expect([...globalThis.document.querySelectorAll('[data-ui="messages.row"]')].at(-1)).toHaveTextContent(
        'messages.recall_self'
      )
    )
    expect(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)).toHaveTextContent(
      'messages.recall_self'
    )
  })

  it('stages files dropped on the composer and keeps the draft', async () => {
    mount()
    await openDirect()
    const shell = globalThis.document.querySelector('[data-ui="messages.composer-shell"]') as globalThis.HTMLElement
    const box = screen.getByRole('textbox', { name: 'messages.compose_placeholder' })
    const report = new globalThis.File(['abc'], 'report.pdf', { type: 'application/pdf' })
    const note = new globalThis.File(['hi'], 'note.txt', { type: 'text/plain' })
    const files = { types: ['Files'], files: [report, note], dropEffect: 'none' }
    fireEvent.change(box, { target: { value: 'keep this' } })

    fireEvent.dragEnter(shell, { dataTransfer: { types: ['text/plain'], files: [] } })
    expect(shell).not.toHaveAttribute('data-drag-over')

    fireEvent.dragEnter(shell, { dataTransfer: files })
    fireEvent.dragEnter(shell, { dataTransfer: files })
    expect(shell).toHaveAttribute('data-drag-over', 'true')
    expect(shell).toHaveTextContent('chat.input.drop_files')
    expect(shell.querySelector('[data-ui="composer.file-drop"]')).toHaveClass('border-[#171717]')
    fireEvent.dragLeave(shell, { dataTransfer: files })
    expect(shell).toHaveAttribute('data-drag-over', 'true')

    fireEvent.drop(shell, { dataTransfer: files })
    const pending = globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')
    expect(pending).toHaveTextContent('report.pdf')
    expect(pending).toHaveTextContent('note.txt')
    expect(
      [...pending!.querySelectorAll('[data-file-icon]')].map((icon) => icon.getAttribute('data-file-icon'))
    ).toEqual(['pdf', 'text'])
    expect(box).toHaveValue('keep this')
    expect(shell).not.toHaveAttribute('data-drag-over')
    expect(globalThis.document.activeElement).toBe(box)
    expect(api.upload).not.toHaveBeenCalled()

    fireEvent.drop(shell, { dataTransfer: { types: ['text/plain'], files: [] } })
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toBe(pending)
    expect(box).toHaveValue('keep this')

    const late = new globalThis.File(['late'], 'late.pdf', { type: 'application/pdf' })
    fireEvent.drop(shell, { dataTransfer: { types: [], files: [late] } })
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toHaveTextContent('late.pdf')
    expect(api.upload).not.toHaveBeenCalled()
  })

  it('reports an empty or oversized drop without uploading it', async () => {
    mount()
    await openDirect()
    const shell = globalThis.document.querySelector('[data-ui="messages.composer-shell"]') as globalThis.HTMLElement
    fireEvent.drop(shell, { dataTransfer: { types: ['Files'], files: [new globalThis.File([], 'empty.pdf')] } })
    expect(await screen.findByRole('alert')).toHaveTextContent('messages.error.empty_file')

    const huge = new globalThis.File(['x'], 'big.bin')
    Object.defineProperty(huge, 'size', { value: 25 * 1024 * 1024 })
    fireEvent.drop(shell, { dataTransfer: { types: ['Files'], files: [huge] } })
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('messages.error.too_large'))
    expect(globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]')).toBeNull()
    expect(api.upload).not.toHaveBeenCalled()
  })

  it('reports oversized attachments without calling the server', async () => {
    mount()
    await openDirect()
    const huge = new globalThis.File(['x'], 'big.bin')
    Object.defineProperty(huge, 'size', { value: 25 * 1024 * 1024 })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [huge] }
      }
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('messages.error.too_large')
    expect(api.upload).not.toHaveBeenCalled()
  })

  it('puts the conversation pinned last at the front of the list', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({
            id: GROUP,
            kind: 'group',
            title: '近期群',
            peerUserId: null,
            memberCount: 3,
            preview: null,
            updatedAt: 9_000,
            pinned: true
          }),
          entry({ updatedAt: 1_000 })
        ])
      }
    })
    await screen.findByText('近期群')
    fireEvent.contextMenu(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.pin' }))
    await waitFor(() => {
      const ids = [...globalThis.document.querySelectorAll('[data-ui="messages.pinned"] [data-conversation-id]')].map(
        (node) => node.getAttribute('data-conversation-id')
      )
      expect(ids).toEqual([DIRECT, GROUP])
    })
  })

  it('pins, mutes and marks unread through the server', async () => {
    mount()
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    const panel = globalThis.document.querySelector('[data-ui="messages.info"]') as globalThis.HTMLElement
    expect(panel).toBeTruthy()
    expect(panel).toHaveClass('absolute')
    expect(screen.queryByRole('button', { name: 'messages.action.info' })).toBeNull()
    fireEvent.click(within(panel).getByRole('button', { name: 'messages.pin_chat' }))
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.pinned"]')).toBeTruthy())
    expect(api.setPinned).toHaveBeenCalledWith(DIRECT, true)

    fireEvent.click(within(panel).getByRole('button', { name: 'messages.mute' }))
    await waitFor(() => expect(api.setMuted).toHaveBeenCalledWith(DIRECT, true))

    fireEvent.click(within(panel).getByRole('button', { name: 'messages.unread' }))
    await waitFor(() => expect(api.markUnread).toHaveBeenCalledWith(DIRECT))
    expect(
      globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"] [data-ui="messages.unread-count"]`)
    ).toHaveTextContent('1')
  })

  it('creates a group from directory people', async () => {
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    api.conversations.mockResolvedValue([
      entry(),
      entry({
        id: GROUP,
        kind: 'group',
        title: '林知夏、周予安、陈可',
        peerUserId: null,
        memberCount: 3,
        preview: null
      })
    ])
    api.detail.mockResolvedValue(detail({ id: GROUP, kind: 'group', title: '林知夏、周予安、陈可', memberCount: 3 }))
    api.messages.mockResolvedValue([])
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.new' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.group_chat' }))
    const peer = await screen.findByRole('checkbox', { name: '周予安' })
    expect(globalThis.document.querySelector('[data-ui="messages.tobot-option"]')).toBeNull()
    expect(globalThis.document.querySelector('[data-ui="messages.toboss-option"]')).toBeNull()
    expect(screen.queryByRole('checkbox', { name: 'messages.file_transfer' })).toBeNull()
    expect(screen.queryByRole('checkbox', { name: 'messages.toboss' })).toBeNull()
    fireEvent.click(peer)
    expect(peer.closest('label')).toHaveClass('bg-[#f2f3f5]')
    fireEvent.click(screen.getByRole('checkbox', { name: '陈可' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.create_group' }))
    await waitFor(() =>
      expect(api.createGroup).toHaveBeenCalledWith('林知夏messages.list_separator周予安messages.list_separator陈可', [
        PEER,
        THIRD
      ])
    )
    await waitFor(() =>
      expect(globalThis.document.querySelector(`[data-conversation-id="${GROUP}"]`)).toHaveAttribute(
        'aria-current',
        'page'
      )
    )
  })

  it('shows ToBoss to the team creator and hides it from everyone else', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: false }])
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    await waitFor(() => expect(memberOrgsApi.listMemberOrgs).toHaveBeenCalled())
    expect(globalThis.document.querySelector('[data-conversation-id="toboss"]')).toBeNull()

    cleanup()
    __resetSharedImStore()
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    mount({ org: 'personal' })
    await screen.findByText('messages.file_transfer', { selector: '[data-ui="messages.conversation-title"]' })
    expect(globalThis.document.querySelector('[data-conversation-id="toboss"]')).toBeNull()

    cleanup()
    __resetSharedImStore()
    mount()
    const row = await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' })
    const boss = row.closest('[data-conversation-id="toboss"]')
    expect(boss).toBeTruthy()
    const face = boss?.querySelector('[data-ui="messages.toboss-avatar"]')
    expect(face).toHaveAttribute('data-avatar-variant', 'boss')
    expect(face?.querySelector('.a3s-bot-avatar__crown')).toBeTruthy()
    expect(face?.querySelector('.a3s-bot-avatar__tie')).toBeTruthy()
    expect(boss?.querySelector('[data-ui="messages.tobot-avatar"]')).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)).toBeNull()
    expect(api.fileTransfer).not.toHaveBeenCalled()

    fireEvent.contextMenu(boss!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.tobot.edit' }))
    const dialog = await screen.findByRole('dialog')
    expect(dialog.querySelector('[data-avatar-variant="boss"]')).toBeTruthy()
    expect(within(dialog).queryByRole('radio', { name: 'messages.tobot.shape.cube' })).toBeNull()
    expect(within(dialog).queryByRole('radio', { name: 'messages.tobot.color.violet' })).toBeNull()
  })

  it('lets the creator add ToBoss to a group without calling add members', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' })
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.new' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.group_chat' }))
    expect(globalThis.document.querySelector('[data-ui="messages.toboss-option"]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'common.close' }))

    await openGroup()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.add_member' }))
    const dialog = await screen.findByRole('dialog')
    const option = within(dialog).getByRole('checkbox', { name: 'messages.toboss' })
    expect(option.closest('label')?.querySelector('[data-avatar-variant="boss"]')).toBeTruthy()
    fireEvent.click(option)
    fireEvent.click(within(dialog).getByRole('button', { name: 'messages.add_member' }))
    await waitFor(() =>
      expect(globalThis.localStorage.getItem(`messages.toboss.groups.${ORG}`)).toBe(JSON.stringify([GROUP]))
    )
    expect(api.addMembers).not.toHaveBeenCalled()
    expect(
      globalThis.document.querySelector(`[data-conversation-id="${GROUP}"] [data-avatar-variant="boss"]`)
    ).toBeTruthy()
  })

  function bossTurnMock() {
    let postedText = ''
    let directives: string[] = []
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return (
        postedText
          ? {
              items: [
                {
                  id: 'assistant-boss',
                  role: 'assistant',
                  status: 'success',
                  searchableText: 'boss says hi',
                  data: { parts: [{ type: 'text', text: 'boss says hi' }] }
                },
                {
                  id: 'user-boss',
                  role: 'user',
                  status: 'success',
                  searchableText: postedText,
                  data: { parts: [{ type: 'text', text: postedText }] }
                }
              ]
            }
          : { items: [] }
      ) as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, init) => {
      if (path === `/agent-sessions/toboss-room-${GROUP}/turns`) {
        const body = (init?.body ?? {}) as {
          userMessageParts?: Array<{ text?: string }>
          turnContext?: { directives?: string[] }
        }
        postedText = (body.userMessageParts ?? []).map((part) => part.text ?? '').join('\n')
        directives = body.turnContext?.directives ?? []
      }
      if (path === '/agent-sessions/toboss/turns') {
        throw new Error('group turn used the private session')
      }
      return { mode: 'started' } as never
    })
    return {
      text: () => postedText,
      directives: () => directives
    }
  }

  it('asks ToBoss when the group mentions it', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    const turn = bossTurnMock()
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await openGroup()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '@messages.toboss hello group' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const thread = globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread).findByText('boss says hi')
    expect(turn.text()).toContain('group hello')
    expect(turn.text()).toContain('@messages.toboss hello group')
    expect(turn.text()).toContain('Do not continue an earlier private reply')
    expect(turn.directives().some((line) => line.includes('This turn is a group'))).toBe(true)
    expect(thread.querySelector('[data-ui="messages.toboss-avatar"][data-avatar-variant="boss"]')).toBeTruthy()
    expect(api.send).toHaveBeenCalled()
    expect(api.addMembers).not.toHaveBeenCalled()
  })

  it('posts a signed-in group turn on that member room session', async () => {
    const member = '225e2510-5521-4aa7-b96c-e0b0c0da6037'
    const roomTurns = `/agent-sessions/toboss-${member}-room-${GROUP}/turns`
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    let postedText = ''
    const turns: string[] = []
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      if (!postedText) return { items: [] } as never
      return {
        items: [
          {
            id: 'assistant-boss',
            role: 'assistant',
            status: 'success',
            searchableText: 'boss says hi',
            data: { parts: [{ type: 'text', text: 'boss says hi' }] }
          },
          {
            id: 'user-boss',
            role: 'user',
            status: 'success',
            searchableText: postedText,
            data: { parts: [{ type: 'text', text: postedText }] }
          }
        ]
      } as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, init) => {
      const name = String(path)
      turns.push(name)
      if (name === '/agent-sessions/toboss/ensure') return { sessionId: `toboss-${member}` } as never
      if (name === roomTurns) {
        const body = (init?.body ?? {}) as { userMessageParts?: Array<{ text?: string }> }
        postedText = (body.userMessageParts ?? []).map((part) => part.text ?? '').join('\n')
      }
      if (name === '/agent-sessions/toboss/turns' || name === `/agent-sessions/toboss-room-${GROUP}/turns`) {
        throw new Error('group turn used the private session')
      }
      return { mode: 'started' } as never
    })
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await openGroup()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '@messages.toboss hello group' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(turns).toContain(roomTurns))
    await waitFor(() => {
      const recorded = api.send.mock.calls.some((call) => {
        const body = call[2] as { text?: string }
        return decodeBossWire(body.text ?? '')?.kind === 'reply'
      })
      expect(recorded).toBe(true)
    })
    expect(turns).not.toContain('/agent-sessions/toboss/turns')
    expect(turns).not.toContain(`/agent-sessions/toboss-room-${GROUP}/turns`)
  })

  it('writes the group answer after ToBoss finishes the turn', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    let postedText = ''
    let reads = 0
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      const name = String(path)
      if (name.endsWith('/turn')) return (reads === 1 ? { status: 'attached' } : {}) as never
      if (!name.endsWith('/messages')) return {} as never
      if (!postedText) return { items: [] } as never
      reads += 1
      const pending = reads === 1
      return {
        items: [
          {
            id: 'assistant-boss',
            role: 'assistant',
            status: pending ? 'pending' : 'success',
            searchableText: pending ? '' : 'boss says hi',
            data: { parts: pending ? [] : [{ type: 'text', text: 'boss says hi' }] }
          },
          {
            id: 'user-boss',
            role: 'user',
            status: 'success',
            searchableText: postedText,
            data: { parts: [{ type: 'text', text: postedText }] }
          }
        ]
      } as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, init) => {
      if (path === `/agent-sessions/toboss-room-${GROUP}/turns`) {
        const body = (init?.body ?? {}) as { userMessageParts?: Array<{ text?: string }> }
        postedText = (body.userMessageParts ?? []).map((part) => part.text ?? '').join('\n')
      }
      if (path === '/agent-sessions/toboss/turns') {
        throw new Error('group turn used the private session')
      }
      return { mode: 'started' } as never
    })
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await openGroup()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '@messages.toboss hello group' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(
      () => {
        const recorded = api.send.mock.calls.some((call) => {
          const body = call[2] as { text?: string }
          return decodeBossWire(body.text ?? '')?.kind === 'reply'
        })
        expect(recorded).toBe(true)
      },
      { timeout: 3000 }
    )
    const sent = api.send.mock.calls.find(
      (call) => decodeBossWire((call[2] as { text?: string }).text ?? '')?.kind === 'reply'
    )
    expect(sent?.[0]).toBe(GROUP)
    if (!sent) throw new Error('Resident reply was not sent')
    expect(decodeBossWire((sent[2] as { text: string }).text)).toEqual({ text: 'boss says hi', kind: 'reply' })
    expect(reads).toBeGreaterThan(1)
  })

  function mockPendingBossTurn() {
    let postedText = ''
    let directives: string[] = []
    let reads = 0
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      const name = String(path)
      if (name.endsWith('/turn')) return (reads === 1 ? { status: 'attached' } : {}) as never
      if (!name.endsWith('/messages')) return {} as never
      if (!postedText) return { items: [] } as never
      reads += 1
      const pending = reads === 1
      return {
        items: [
          {
            id: 'assistant-boss',
            role: 'assistant',
            status: pending ? 'pending' : 'success',
            searchableText: pending ? '' : 'boss says hi',
            data: { parts: pending ? [] : [{ type: 'text', text: 'boss says hi' }] }
          },
          {
            id: 'user-boss',
            role: 'user',
            status: 'success',
            searchableText: postedText,
            data: { parts: [{ type: 'text', text: postedText }] }
          }
        ]
      } as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, init) => {
      if (String(path).endsWith('/turns')) {
        const body = (init?.body ?? {}) as {
          userMessageParts?: Array<{ text?: string }>
          turnContext?: { directives?: string[] }
        }
        postedText = (body.userMessageParts ?? []).map((part) => part.text ?? '').join('\n')
        directives = body.turnContext?.directives ?? []
      }
      return { mode: 'started' } as never
    })
    return { text: () => postedText, directives: () => directives }
  }

  function groupMention(fake: FakeApi) {
    withDirectAndGroup(fake, {
      unread: 2,
      lastSequence: 2,
      lastReadSequence: 0,
      preview: {
        kind: 'text',
        messageId: 'm-ask',
        sequence: 2,
        recalled: false,
        createdAt: Date.now(),
        sender: { kind: 'member', id: PEER },
        text: '@messages.toboss 你说说',
        name: null,
        code: null,
        params: {}
      }
    })
    fake.messages.mockImplementation(async (conversationId: string) =>
      conversationId === GROUP
        ? [
            message(1, {
              conversationId: GROUP,
              body: { kind: 'text', text: 'group hello' },
              createdAt: Date.now() - 60_000
            }),
            message(2, {
              id: 'm-ask',
              conversationId: GROUP,
              sender: { kind: 'member', id: PEER },
              body: { kind: 'text', text: '@messages.toboss 你说说' },
              createdAt: Date.now()
            })
          ]
        : [message(2), message(1)]
    )
  }

  async function expectRecordedRoomAnswer(turn: { text: () => string; directives: () => string[] }) {
    await waitFor(
      () => {
        const recorded = api.send.mock.calls.some((call) => {
          const body = call[2] as { text?: string }
          return decodeBossWire(body.text ?? '')?.kind === 'reply'
        })
        expect(recorded).toBe(true)
      },
      { timeout: 3000 }
    )
    const sent = api.send.mock.calls.find(
      (call) => decodeBossWire((call[2] as { text?: string }).text ?? '')?.kind === 'reply'
    )
    expect(sent?.[0]).toBe(GROUP)
    if (!sent) throw new Error('Resident reply was not sent')
    expect(decodeBossWire((sent[2] as { text: string }).text)).toEqual({ text: 'boss says hi', kind: 'reply' })
    expect(turn.text()).toContain('group hello')
    expect(turn.text()).toContain('@messages.toboss 你说说')
    expect(turn.text()).toContain('Do not continue an earlier private reply')
    expect(turn.directives().some((line) => line.includes('This turn is a group'))).toBe(true)
    expect(api.markRead).not.toHaveBeenCalledWith(GROUP)
  }

  it('records a ToBoss answer when the group is not open', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    const turn = mockPendingBossTurn()
    mount({ configure: groupMention })
    await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' })
    await expectRecordedRoomAnswer(turn)
    expect(globalThis.document.querySelector(`[data-conversation-id="${GROUP}"]`)).not.toHaveAttribute(
      'aria-current',
      'page'
    )
  })

  it('records a ToBoss answer from the shell when messages is closed', async () => {
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    const turn = mockPendingBossTurn()
    const fake = fakeApi()
    groupMention(fake)
    const store = new ImStore({
      createApi: () => fake,
      runtimeAvailable: () => true,
      openStream: () => () => undefined,
      deviceId: () => 'studio-test',
      isVisible: () => true,
      createObjectUrl: () => 'blob:attachment',
      revokeObjectUrl: () => undefined,
      syncDelayMs: 0
    })
    __resetSharedImStore(store)
    api = fake
    render(<BossRoomRuntime />)
    store.start(ORG)
    await waitFor(() => expect(store.getSnapshot().status).toBe('ready'))
    store.setTeamCreator(true)
    await expectRecordedRoomAnswer(turn)
  })

  it('keeps a group turn out of the ToBoss private thread', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    const prompt = bossRoomPrompt({
      room: '项目组',
      asker: '周予安',
      ask: '你说说',
      lines: [{ speaker: '周予安', text: '群里的一句' }]
    })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      const target = String(path)
      if (!target.includes('/agent-sessions/toboss') || !target.endsWith('/messages')) return {} as never
      return {
        items: [
          {
            id: 'a2',
            role: 'assistant',
            status: 'success',
            createdAt: '2026-10-07T15:46:29.980Z',
            data: { parts: [{ type: 'text', text: '群里的表格' }] }
          },
          {
            id: 'u2',
            role: 'user',
            status: 'success',
            createdAt: '2026-10-07T15:46:29.000Z',
            data: { parts: [{ type: 'text', text: prompt }] }
          },
          {
            id: 'a1',
            role: 'assistant',
            status: 'success',
            createdAt: '2026-10-07T12:53:56.370Z',
            data: { parts: [{ type: 'text', text: '私聊回答' }] }
          },
          {
            id: 'u1',
            role: 'user',
            status: 'success',
            createdAt: '2026-10-07T12:53:56.000Z',
            data: { parts: [{ type: 'text', text: '私聊问题' }] }
          }
        ]
      } as never
    })
    mount()
    const title = await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' })
    const row = title.closest('[data-conversation-id="toboss"]')
    await waitFor(() =>
      expect(row?.querySelector('[data-ui="messages.conversation-preview"]')).toHaveTextContent('私聊回答')
    )
    fireEvent.click(title)
    expect(await screen.findByText('私聊问题')).toBeTruthy()
    const thread = globalThis.document.querySelector('[data-ui="messages.thread-scroll"]')
    expect(thread?.textContent).toContain('私聊回答')
    expect(thread?.textContent).not.toContain('群里的一句')
    expect(thread?.textContent).not.toContain('群里的表格')
    expect(screen.queryByText('群里的表格')).toBeNull()
  })

  it('renders a ToBoss group reply with task conversation markdown', async () => {
    const answer = [
      '| 项 | 值 |',
      '| --- | --- |',
      '| 核心 | 组织 |',
      '',
      '```ts',
      'const n = 1',
      '```',
      '',
      '```mermaid',
      'flowchart LR',
      'A-->B',
      '```'
    ].join('\n')
    mount({
      configure: (fake) => {
        withDirectAndGroup(fake)
        fake.messages.mockImplementation(async (conversationId: string) =>
          conversationId === GROUP
            ? [
                message(1, {
                  conversationId: GROUP,
                  sender: { kind: 'member', id: PEER },
                  body: { kind: 'text', text: encodeBossWire(answer, 'reply') }
                })
              ]
            : [message(2), message(1)]
        )
      }
    })
    fireEvent.click(await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' }))
    expect(await screen.findByRole('columnheader', { name: '项' })).toBeTruthy()
    expect(screen.getByRole('cell', { name: '组织' })).toBeTruthy()
    expect(globalThis.document.body.textContent).toContain('const n = 1')
    const prose = globalThis.document.querySelector('[data-task-prose]')
    expect(prose).toBeTruthy()
    const column = prose?.closest('[data-ui="messages.bubble"]')?.parentElement?.parentElement
    expect(column?.className).toContain('w-full')
    expect(column?.className).toContain('max-w-[70%]')
    expect(globalThis.document.querySelector('[data-ui="messages.bubble"].whitespace-pre-wrap')).toBeNull()
    await waitFor(() => expect(globalThis.document.querySelector('[data-diagram-kind="flowchart"]')).toBeTruthy())
  })

  it('reads the room when a member mentions ToBoss', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    const turn = bossTurnMock()
    mount({
      configure: (fake) => {
        withDirectAndGroup(fake)
        fake.messages.mockImplementation(async (conversationId: string) =>
          conversationId === GROUP
            ? [
                message(1, { conversationId: GROUP, body: { kind: 'text', text: 'group hello' } }),
                message(2, {
                  conversationId: GROUP,
                  sender: { kind: 'member', id: PEER },
                  body: { kind: 'text', text: '@messages.toboss 你说' },
                  createdAt: Date.now()
                })
              ]
            : [message(2), message(1)]
        )
      }
    })
    await openGroup()
    await waitFor(() => expect(turn.text()).toContain('group hello'))
    expect(turn.text()).toContain('@messages.toboss 你说')
    expect(turn.text()).toContain('周予安')
    expect(turn.text()).toContain('Do not continue an earlier private reply')
    expect(turn.directives().some((line) => line.includes('This turn is a group'))).toBe(true)
  })

  it('lets a member mention ToBoss without running a turn', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: false }])
    mount({
      configure: (fake) => {
        withDirectAndGroup(fake)
        fake.messages.mockImplementation(async (conversationId: string) =>
          conversationId === GROUP
            ? [message(1, { conversationId: GROUP, body: { kind: 'text', text: encodeBossWire('', 'join') } })]
            : [message(2), message(1)]
        )
      }
    })
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.click(screen.getByText('项目组', { selector: '[data-ui="messages.conversation"] span' }))
    expect(await screen.findByText('messages.toboss.joined')).toBeTruthy()
    expect(await screen.findByRole('heading', { name: '项目组 (4)' })).toBeTruthy()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '@messages.toboss 你说' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(vi.mocked(dataApiService.post).mock.calls.some((call) => String(call[0]).includes('toboss'))).toBe(false)
  })

  it('reloads colleagues when the group picker opens', async () => {
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    api.directory.mockResolvedValue([
      { userId: ME, displayName: '林知夏', email: 'me@example.com', departmentId: 'd-1', position: null },
      { userId: PEER, displayName: '周予安', email: 'peer@example.com', departmentId: 'd-2', position: '设计' },
      { userId: THIRD, displayName: '陈可', email: 'third@example.com', departmentId: 'd-2', position: null },
      { userId: 'new-member', displayName: '新同事', email: '', departmentId: 'd-1', position: null }
    ])
    vi.mocked(listOrgMembers).mockResolvedValue([
      { id: 'new-member', name: '新同事', phone: '13900001111', avatar: '' }
    ] as never)
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.new' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.group_chat' }))
    expect(await screen.findByRole('checkbox', { name: '新同事' })).toBeTruthy()
  })

  it('starts a direct chat from the organization chart', async () => {
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.new' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.org' }))
    const dialog = await waitFor(() => {
      const found = globalThis.document.querySelector('[data-ui="messages.org-dialog"]')
      expect(found).toBeTruthy()
      return found!
    })
    expect(dialog).toHaveTextContent('总部')
    expect(dialog).toHaveTextContent('设计部')
    fireEvent.click(screen.getByRole('button', { name: /设计部/ }))
    const people = [...dialog.querySelectorAll('[data-ui="messages.org-person"]')]
    expect(people.map((node) => node.textContent)).toEqual([
      expect.stringContaining('周予安'),
      expect.stringContaining('陈可')
    ])
    await act(async () => {
      fireEvent.click(people[0])
    })
    await waitFor(() => expect(api.startDirect).toHaveBeenCalledWith(PEER))
    await waitFor(() =>
      expect(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)).toHaveAttribute(
        'aria-current',
        'page'
      )
    )
  })

  it('keeps the resizable list and composer', async () => {
    mount()
    const separator = screen.getByRole('separator', { name: 'messages.resize' })
    const column = separator.parentElement
    expect(column).toHaveStyle({ width: '260px' })
    fireEvent.mouseDown(separator, { clientX: 400 })
    fireEvent.mouseMove(globalThis.document, { clientX: 460 })
    fireEvent.mouseUp(globalThis.document)
    expect(column).toHaveStyle({ width: '320px' })

    await openDirect()
    const composerEdge = screen.getByRole('separator', { name: 'messages.resize_input' })
    fireEvent.mouseDown(composerEdge, { clientY: 400 })
    fireEvent.mouseMove(globalThis.document, { clientY: 340 })
    fireEvent.mouseUp(globalThis.document)
    expect(globalThis.document.querySelector('[data-ui="messages.composer"]')).toHaveStyle({ height: '132px' })
  })

  it('opens the emoji panel above the composer without clipping it', async () => {
    mount()
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.emoji' }))
    const panel = globalThis.document.querySelector('[data-ui="messages.emoji-panel"]')
    expect(panel).toBeTruthy()
    const picker = panel as globalThis.HTMLElement
    await waitFor(() => expect(picker).toHaveTextContent('😀'))
    expect(picker).toHaveTextContent('😅')
    expect(picker).toHaveTextContent('emoji_picker.categories.smileys_emotion')
    expect(within(picker).getByRole('searchbox', { name: 'emoji_picker.search' })).toBeInTheDocument()
    expect(within(picker).getByRole('button', { name: 'emoji_picker.categories.smileys_emotion' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    let node = panel?.parentElement
    while (node) {
      expect(String(node.className)).not.toContain('overflow-hidden')
      node = node.parentElement
    }
    const face = within(picker).getAllByRole('button', { name: '😀' })[0]
    expect(face).not.toHaveAttribute('title')
    const tip = face.closest('[data-testid="tooltip"]')
    expect(tip).toHaveAttribute('data-title', '嘿嘿')
    expect(tip?.querySelector('[data-testid="tooltip-content"]')).toHaveTextContent('嘿嘿')
    fireEvent.click(within(picker).getByRole('button', { name: 'emoji_picker.categories.animals_nature' }))
    expect(within(picker).getByRole('button', { name: 'emoji_picker.categories.animals_nature' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(picker).toHaveTextContent('emoji_picker.categories.animals_nature')
    fireEvent.change(within(picker).getByRole('searchbox', { name: 'emoji_picker.search' }), {
      target: { value: '苦笑' }
    })
    await waitFor(() => {
      expect(picker).toHaveTextContent('😅')
      expect(picker).not.toHaveTextContent('😀')
    })
    fireEvent.click(within(picker).getByRole('button', { name: 'emoji_picker.categories.smileys_emotion' }))
    await waitFor(() => expect(picker).toHaveTextContent('😀'))
    const box = screen.getByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(box, { target: { value: 'ab' } })
    if (!(box instanceof globalThis.HTMLTextAreaElement)) throw new Error('The composer must provide text selection')
    box.setSelectionRange(1, 1)
    fireEvent.click(within(picker).getAllByRole('button', { name: '😀' })[0])
    expect(box).toHaveValue('a😀b')
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.emoji-panel"]')).toBeNull())
  })

  it('offers hide-window screenshot on the composer without a settings item', async () => {
    mount()
    await openDirect()
    expect(screen.getByRole('button', { name: 'workbench.composer.capture' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture_options' }))
    const menu = screen.getByRole('menu')
    expect(screen.getByRole('menuitem', { name: 'workbench.composer.capture_hide' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'common.settings' })).not.toBeInTheDocument()
    expect(menu.parentElement).toBe(globalThis.document.body)
  })

  it('keeps each conversation draft when switching chats', async () => {
    mount({ configure: withDirectAndGroup })
    await openDirect()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '只留在周予安' }
    })
    await openGroup()
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toHaveValue('')
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '只留在项目组' }
    })
    fireEvent.click(screen.getByText('周予安', { selector: '[data-ui="messages.conversation"] span' }))
    expect(await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })).toHaveValue('只留在周予安')
    fireEvent.click(screen.getByText('项目组', { selector: '[data-ui="messages.conversation"] span' }))
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toHaveValue('只留在项目组')
  })

  it('shows a remark in the list and saves it for this member only', async () => {
    mount()
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.remark' }))
    fireEvent.blur(screen.getByRole('textbox', { name: 'messages.remark' }))
    expect(api.setRemark).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'messages.remark' }))
    const remark = screen.getByRole('textbox', { name: 'messages.remark' })
    fireEvent.change(remark, { target: { value: '临时' } })
    fireEvent.keyDown(remark, { key: 'Escape' })
    expect(api.setRemark).not.toHaveBeenCalled()
    expect(screen.queryByRole('textbox', { name: 'messages.remark' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'messages.remark' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.remark' }), { target: { value: '小周' } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'messages.remark' }), { key: 'Enter' })
    await waitFor(() => expect(api.setRemark).toHaveBeenCalledWith(DIRECT, '小周'))
    await waitFor(() => {
      const title = globalThis.document.querySelector(
        `[data-conversation-id="${DIRECT}"] [data-ui="messages.conversation-title"]`
      )
      expect(title).toHaveTextContent('小周')
      expect(title).toHaveAttribute('title', '小周')
    })
  })

  it('renames a group, sets a nickname, and mutes the group or one member', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({ id: GROUP, kind: 'group', title: '设计评审', peerUserId: null, preview: null, unread: 0 })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '设计评审',
            memberCount: 2,
            members: [
              { userId: ME, role: 'owner', left: false, lastReadSequence: 0 },
              { userId: PEER, role: 'member', left: false, lastReadSequence: 0 }
            ]
          })
        )
        fake.messages.mockResolvedValue([])
      }
    })
    fireEvent.click(await screen.findByText('设计评审'))
    await screen.findByRole('button', { name: 'messages.action.more' })
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    const info = globalThis.document.querySelector('[data-ui="messages.info"]') as globalThis.HTMLElement
    expect(info).toHaveClass('absolute', 'w-[300px]', 'transition-transform')
    await waitFor(() => expect(info).toHaveAttribute('data-state', 'open'))
    expect(screen.getByRole('textbox', { name: 'messages.search_members' })).toBeInTheDocument()
    expect(globalThis.document.querySelector('[data-ui="messages.member-grid"]')).toBeTruthy()
    fireEvent.click(await screen.findByRole('button', { name: 'messages.rename' }))
    const rename = screen.getByRole('textbox', { name: 'messages.rename' })
    fireEvent.change(rename, { target: { value: '   ' } })
    fireEvent.keyDown(rename, { key: 'Enter' })
    expect(api.rename).not.toHaveBeenCalled()
    expect(screen.getByRole('textbox', { name: 'messages.rename' })).toBeInTheDocument()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.rename' }), { target: { value: '新群名' } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'messages.rename' }), { key: 'Enter' })
    await waitFor(() => expect(api.rename).toHaveBeenCalledWith(GROUP, '新群名'))

    const memberRow = globalThis.document.querySelector(`[data-ui="messages.member"]`) as globalThis.HTMLElement
    fireEvent.click(
      within(memberRow.parentElement as globalThis.HTMLElement).getAllByRole('button', {
        name: 'messages.member_actions'
      })[0]
    )
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.transfer_owner' }))
    expect(api.transferOwner).not.toHaveBeenCalled()
    fireEvent.click(
      within(screen.getByRole('dialog', { name: 'messages.transfer_confirm_title' })).getByRole('button', {
        name: 'messages.transfer_owner'
      })
    )
    await waitFor(() => expect(api.transferOwner).toHaveBeenCalledWith(GROUP, PEER))

    fireEvent.click(screen.getAllByRole('button', { name: 'messages.member_actions' })[0])
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.role.make_admin' }))
    await waitFor(() => expect(api.setMemberRole).toHaveBeenCalledWith(GROUP, PEER, 'admin'))

    fireEvent.click(screen.getByRole('button', { name: 'messages.nickname' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.nickname' }), { target: { value: '夏夏' } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'messages.nickname' }), { key: 'Enter' })
    await waitFor(() => expect(api.setNickname).toHaveBeenCalledWith(GROUP, '夏夏'))

    fireEvent.click(screen.getByRole('button', { name: 'messages.speech.mute_all' }))
    await waitFor(() => expect(api.setMuteAll).toHaveBeenCalledWith(GROUP, true))

    fireEvent.click(screen.getAllByRole('button', { name: 'messages.member_actions' })[0])
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.speech.ban' }))
    await waitFor(() => expect(api.setMemberSpeech).toHaveBeenCalledWith(GROUP, PEER, true))

    expect(screen.queryByRole('button', { name: 'messages.leave' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'messages.dissolve' }))
    fireEvent.click(
      within(screen.getByRole('dialog', { name: 'messages.dissolve_confirm_title' })).getByRole('button', {
        name: 'messages.dissolve'
      })
    )
    await waitFor(() => expect(api.dissolve).toHaveBeenCalledWith(GROUP))
    await waitFor(() => expect(screen.queryByRole('button', { name: 'messages.action.more' })).toBeNull())
  })

  it('disables the composer when the group is muted or the member is muted', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({ id: GROUP, kind: 'group', title: '设计评审', peerUserId: null, preview: null, unread: 0 })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '设计评审',
            muteAll: true,
            members: [
              { userId: ME, role: 'member', left: false, lastReadSequence: 0 },
              { userId: PEER, role: 'owner', left: false, lastReadSequence: 0, nickname: '群昵称' }
            ]
          })
        )
        fake.messages.mockResolvedValue([
          message(1, {
            conversationId: GROUP,
            sender: { kind: 'member', id: PEER },
            body: { kind: 'text', text: 'hello' }
          })
        ])
      }
    })
    fireEvent.click(await screen.findByText('设计评审'))
    expect(await screen.findByText('群昵称')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    expect(screen.getByRole('button', { name: 'messages.leave' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'messages.dissolve' })).toBeNull()
    expect(screen.queryByRole('textbox', { name: 'messages.rename' })).toBeNull()
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeDisabled()
    expect(globalThis.document.querySelector('[data-ui="messages.speech-block"]')).toHaveTextContent(
      'messages.speech.muted_all'
    )
  })

  it('shows a typing line and a presence dot, and hides the dot when presence fails', async () => {
    const capture: { emit: (event: string, data: Record<string, unknown>) => void } = { emit: () => undefined }
    mount({
      capture,
      configure: (fake) => {
        fake.presence.mockResolvedValue([PEER])
      }
    })
    await openDirect()
    await waitFor(() =>
      expect(
        globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"] [data-ui="messages.presence"]`)
      ).toBeTruthy()
    )
    capture.emit('im.ephemeral.typing', { conversation_id: DIRECT, user_id: PEER, ttl_secs: 30 })
    expect(await screen.findByText('messages.typing')).toBeInTheDocument()
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), { target: { value: 'hi' } })
    await waitFor(() => expect(api.typing).toHaveBeenCalledWith(DIRECT))

    cleanup()
    __resetSharedImStore()
    mount({ configure: (fake) => fake.renewPresence.mockResolvedValue(false) })
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    await waitFor(() => expect(api.renewPresence).toHaveBeenCalled())
    expect(globalThis.document.querySelector('[data-ui="messages.presence"]')).toBeNull()
  })

  it('pins, mutes and marks unread from the conversation context menu', async () => {
    mount()
    const row = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.contextMenu(row.closest('[data-ui="messages.conversation"]')!)
    const menu = globalThis.document.querySelector('[data-ui="messages.context-menu"]')
    expect(menu).toBeTruthy()
    const pin = within(menu as globalThis.HTMLElement).getByRole('menuitem', { name: 'messages.pin' })
    expect(pin.querySelector('svg')).toBeTruthy()
    fireEvent.click(pin)
    await waitFor(() => expect(api.setPinned).toHaveBeenCalledWith(DIRECT, true))
    expect(globalThis.document.querySelector('[data-ui="messages.pinned"]')).toBeTruthy()

    fireEvent.contextMenu(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.mute' }))
    await waitFor(() => expect(api.setMuted).toHaveBeenCalledWith(DIRECT, true))
    expect(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)?.querySelector('svg')).toBeTruthy()

    fireEvent.contextMenu(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.unread' }))
    await waitFor(() => expect(api.markUnread).toHaveBeenCalledWith(DIRECT))
    expect(
      globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"] [data-ui="messages.unread-count"]`)
    ).toHaveTextContent('1')
  })

  it('pins the personal ToBot row without an organization API', async () => {
    mount({ org: 'personal' })
    const row = globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)
    expect(row).toBeTruthy()
    fireEvent.contextMenu(row!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.pin' }))
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.pinned"]')).toBeTruthy())
    fireEvent.contextMenu(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.unread' }))
    await waitFor(() =>
      expect(
        globalThis.document.querySelector(
          `[data-conversation-id="${BUILTIN_TOBOT_ID}"] [data-ui="messages.unread-count"]`
        )
      ).toHaveTextContent('1')
    )
  })

  it('keeps a stored personal ToBot out of a team group', async () => {
    globalThis.localStorage.setItem(`messages.tobot.groups.${ORG}`, JSON.stringify([GROUP]))
    const post = vi.spyOn(dataApiService, 'post').mockResolvedValue({ mode: 'started' })
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            peerUserId: null,
            memberCount: 3,
            preview: null,
            updatedAt: Date.parse('2026-10-02T09:00:00Z')
          })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            memberCount: 3,
            members: [
              { userId: ME, role: 'owner', left: false, lastReadSequence: 0 },
              { userId: PEER, role: 'member', left: false, lastReadSequence: 0 },
              { userId: THIRD, role: 'member', left: false, lastReadSequence: 0 }
            ]
          })
        )
        fake.messages.mockResolvedValue([])
      }
    })
    const title = await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' })
    const row = title.closest('[data-ui="messages.conversation"]')
    await waitFor(() =>
      expect(row?.querySelector('[data-ui="messages.group-avatar"]')).toHaveAttribute('data-faces', '3')
    )
    expect(row?.querySelector('[data-a3s-bot-avatar="true"]')).toBeNull()
    fireEvent.contextMenu(row!)
    expect(screen.queryByRole('menuitem', { name: 'messages.invite_tobot' })).toBeNull()
    expect(screen.queryByRole('menuitem', { name: 'messages.remove_tobot' })).toBeNull()

    fireEvent.click(row!)
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('项目组 (3)')
    fireEvent.change(await screen.findByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '@messages.file_transfer hello group' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/turns')).toBe(false)
    expect(globalThis.document.querySelector('[data-ui="messages.tobot-turn"]')).toBeNull()
    expect(api.addMembers).not.toHaveBeenCalled()
  })

  it('does not offer mentions in a direct chat', async () => {
    mount()
    await openDirect()
    const box = screen.getByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(box, { target: { value: '@周', selectionStart: 2, selectionEnd: 2 } })
    expect(screen.queryByRole('listbox', { name: 'messages.mention.picker' })).toBeNull()
  })

  it('mentions a group member and does not wake the personal ToBot', async () => {
    globalThis.localStorage.setItem(`messages.tobot.groups.${ORG}`, JSON.stringify([GROUP]))
    const post = vi.spyOn(dataApiService, 'post').mockResolvedValue({ mode: 'started' })
    vi.spyOn(dataApiService, 'get').mockResolvedValue({ items: [] })
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            peerUserId: null,
            memberCount: 3,
            lastSequence: 2,
            lastReadSequence: 1,
            unread: 1,
            preview: {
              kind: 'text',
              messageId: 'm-group-2',
              sequence: 2,
              recalled: false,
              createdAt: Date.parse('2026-10-02T09:00:00Z'),
              sender: { kind: 'member', id: PEER },
              text: '@林知夏 看一下',
              name: null,
              code: null,
              params: {}
            }
          })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            memberCount: 3,
            members: [
              { userId: ME, role: 'owner', left: false, lastReadSequence: 1 },
              { userId: PEER, role: 'member', left: false, lastReadSequence: 0 },
              { userId: THIRD, role: 'member', left: false, lastReadSequence: 0 }
            ]
          })
        )
        fake.messages.mockResolvedValue([])
      }
    })
    const row = (await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' })).closest(
      '[data-ui="messages.conversation"]'
    )
    expect(row?.querySelector('[data-ui="messages.mention-you"]')).toHaveTextContent('messages.mention.you')
    fireEvent.click(row!)
    await waitFor(() => expect(row?.querySelector('[data-ui="messages.mention-you"]')).toBeNull())

    const box = (await screen.findByRole('textbox', {
      name: 'messages.compose_placeholder'
    })) as globalThis.HTMLTextAreaElement
    fireEvent.change(box, { target: { value: '@', selectionStart: 1, selectionEnd: 1 } })
    await screen.findByRole('listbox', { name: 'messages.mention.picker' })
    const optionNamed = (name: string) => screen.getByRole('option', { name: (value) => value.includes(name) })
    await waitFor(() => expect(optionNamed('周予安')).toBeInTheDocument())
    expect(optionNamed('messages.mention.all')).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: (value) => value.includes('messages.file_transfer') })).toBeNull()
    expect(screen.queryByRole('option', { name: (value) => value.includes('林知夏') })).toBeNull()

    fireEvent.change(box, { target: { value: '@周', selectionStart: 2, selectionEnd: 2 } })
    fireEvent.click(screen.getByRole('option', { name: (value) => value.includes('周予安') }))
    await waitFor(() => expect(box.value).toBe('@周予安 '))
    fireEvent.change(box, { target: { value: '@周予安 你好', selectionStart: 7, selectionEnd: 7 } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const mention = await screen.findByText('@周予安', { selector: '[data-ui="messages.mention"]' })
    expect(mention).toHaveAttribute('data-mention-me', 'false')
    expect(mention).toHaveAttribute('data-mention-target', 'member')
    expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/turns')).toBe(false)

    fireEvent.change(box, { target: { value: 'hello group', selectionStart: 11, selectionEnd: 11 } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/turns')).toBe(false)

    fireEvent.change(box, {
      target: { value: '@messages.file_transfer 在吗', selectionStart: 28, selectionEnd: 28 }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/turns')).toBe(false)
  })

  it('does not send a team group image to the personal ToBot', async () => {
    globalThis.localStorage.setItem(`messages.tobot.groups.${ORG}`, JSON.stringify([GROUP]))
    tobotUpload.uploadTobotFile.mockReset()
    tobotUpload.uploadTobotFile.mockImplementation(async (file: globalThis.File) => ({
      type: 'file' as const,
      url: `file:///tmp/a3s-studio-composer/id_${file.name}`,
      filename: file.name,
      mediaType: file.type || 'application/octet-stream',
      sizeBytes: file.size
    }))
    let turnBody: { body: { userMessageParts: Array<{ type: string; filename?: string; text?: string }> } } | undefined
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path === '/agent-sessions/tobot/turns') turnBody = options as typeof turnBody
      return { mode: 'started' } as never
    })
    vi.spyOn(dataApiService, 'get').mockResolvedValue({ items: [] })
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            peerUserId: null,
            memberCount: 3,
            lastSequence: 2,
            lastReadSequence: 2,
            unread: 0
          })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '项目组',
            memberCount: 3,
            members: [
              { userId: ME, role: 'owner', left: false, lastReadSequence: 2 },
              { userId: PEER, role: 'member', left: false, lastReadSequence: 0 },
              { userId: THIRD, role: 'member', left: false, lastReadSequence: 0 }
            ]
          })
        )
        fake.messages.mockResolvedValue([
          message(1, {
            conversationId: GROUP,
            recalled: true,
            body: {
              kind: 'image',
              attachment: { name: 'old.png', sizeBytes: 3, objectKey: 'im/old.png', mime: 'image/png' }
            }
          }),
          message(2, {
            conversationId: GROUP,
            body: {
              kind: 'image',
              attachment: { name: 'board.png', sizeBytes: 4, objectKey: 'im/board.png', mime: 'image/png' }
            }
          }),
          message(3, {
            conversationId: GROUP,
            body: {
              kind: 'voice',
              attachment: { name: 'clip.m4a', sizeBytes: 4, objectKey: 'im/clip.m4a', mime: 'audio/mp4' },
              durationSecs: 1
            }
          })
        ])
        fake.download.mockResolvedValue(new globalThis.Blob(['png'], { type: 'image/png' }))
      }
    })
    const row = (await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' })).closest(
      '[data-ui="messages.conversation"]'
    )
    fireEvent.click(row!)
    await screen.findByRole('button', { name: 'board.png' })
    const box = screen.getByRole('textbox', { name: 'messages.compose_placeholder' })
    fireEvent.change(box, { target: { value: 'hello group' } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(tobotUpload.uploadTobotFile).not.toHaveBeenCalled()
    expect(turnBody).toBeUndefined()

    const prompt = '@messages.file_transfer 看这张图'
    fireEvent.change(box, { target: { value: prompt } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(2))
    expect(tobotUpload.uploadTobotFile).not.toHaveBeenCalled()
    expect(turnBody).toBeUndefined()
  })

  it('shows sent direct text in that thread and list preview, and keeps it when the group sends different text', async () => {
    const directText = '单聊原文-甲'
    const groupText = '群聊原文-乙'
    mount({ configure: withDirectAndGroup })
    await openDirect()
    await sendDraft(directText)
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread()).findByText(directText)
    await waitFor(() => expect(previewOf(DIRECT)).toHaveTextContent(directText))

    await openGroup()
    await sendDraft(groupText)
    await within(thread()).findByText(groupText)
    await waitFor(() => expect(previewOf(GROUP)).toHaveTextContent(groupText))
    expect(previewOf(DIRECT)).toHaveTextContent(directText)
    expect(previewOf(DIRECT)).not.toHaveTextContent(groupText)
    expect(within(thread()).queryByText(directText)).toBeNull()
  })

  it('opens a preview when a sent file card is clicked and downloads from that preview', async () => {
    mount()
    await openDirect()
    const file = new globalThis.File(['abc'], 'brief.pdf', { type: 'application/pdf' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [file] }
      }
    )
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const card = await waitFor(() => {
      const found = screen.getByRole('button', { name: 'brief.pdf' })
      expect(found).toBeEnabled()
      expect(found).toHaveAttribute('data-ui', 'messages.file')
      expect(found.querySelector('svg')).toHaveClass('h-9', 'w-8')
      return found
    })
    expect(previewOf(DIRECT)).toHaveTextContent('brief.pdf')
    const saved: Array<{ name: string; href: string }> = []
    const click = globalThis.HTMLAnchorElement.prototype.click
    globalThis.HTMLAnchorElement.prototype.click = function (this: globalThis.HTMLAnchorElement) {
      saved.push({ name: this.download, href: this.href })
    }
    try {
      fireEvent.click(card)
      const preview = await screen.findByRole('dialog')
      expect(preview).toHaveAttribute('data-ui', 'messages.file-preview')
      expect(preview).toHaveTextContent('brief.pdf')
      expect(saved).toHaveLength(0)
      const download = await within(preview).findByRole('button', { name: 'messages.download' })
      await waitFor(() => expect(download).toBeEnabled())
      expect(api.download).toHaveBeenCalledWith(`m-${DIRECT}-3`)
      fireEvent.click(download)
      await waitFor(() => expect(saved).toHaveLength(1))
    } finally {
      globalThis.HTMLAnchorElement.prototype.click = click
    }
    expect(saved[0]?.name).toBe('brief.pdf')
    expect(saved[0]?.href.startsWith('blob:')).toBe(true)
  })

  it('previews a file staged in the composer without uploading it', async () => {
    mount()
    await openDirect()
    const note = new globalThis.File(['hello note'], 'note.txt', { type: 'text/plain' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [note] }
      }
    )
    const tray = globalThis.document.querySelector('[data-ui="messages.screenshot-pending"]') as globalThis.HTMLElement
    const opener = within(tray).getByRole('button', { name: 'note.txt' })
    const icon = opener.querySelector('[data-file-icon]')
    expect(icon).toHaveAttribute('data-file-icon', 'text')
    expect(icon?.querySelector('svg')).toHaveClass('h-9', 'w-8')
    const label = within(opener).getByText('note.txt')
    if (!icon) throw new Error('The attachment icon must exist')
    expect(icon.compareDocumentPosition(label) & globalThis.Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    fireEvent.click(opener)
    const preview = await screen.findByRole('dialog')
    expect(preview).toHaveAttribute('data-ui', 'messages.file-preview')
    expect(await within(preview).findByText('hello note')).toBeInTheDocument()
    expect(api.upload).not.toHaveBeenCalled()
    fireEvent.click(within(preview).getByRole('button', { name: 'common.close' }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(tray).toHaveTextContent('note.txt')
  })

  it('opens a larger view of the sent image', async () => {
    mount()
    await openDirect()
    const image = new globalThis.File(['png'], 'board.png', { type: 'image/png' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.image-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [image] }
      }
    )
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(api.send).toHaveBeenCalledWith(DIRECT, expect.stringMatching(/^studio-/), {
        kind: 'image',
        attachment: expect.objectContaining({ name: 'board.png', mime: 'image/png' })
      })
    )
    const opener = await waitFor(() => {
      const found = screen.getByRole('button', { name: 'board.png' })
      expect(found).toHaveAttribute('data-ui', 'messages.image')
      expect(within(found).getByRole('img', { name: 'board.png' })).toHaveAttribute('src', 'blob:attachment')
      return found
    })
    fireEvent.click(opener)
    const preview = await screen.findByTestId('image-preview-dialog')
    expect(preview).toHaveClass('z-[1100]')
    expect(preview).not.toHaveClass('z-[80]')
    const large = preview.querySelector('img')
    expect(large).toHaveAttribute('alt', 'board.png')
    expect(large).toHaveAttribute('src', 'blob:attachment')
    const saved: string[] = []
    const click = globalThis.HTMLAnchorElement.prototype.click
    globalThis.HTMLAnchorElement.prototype.click = function (this: globalThis.HTMLAnchorElement) {
      saved.push(this.download)
    }
    try {
      fireEvent.click(within(preview).getByRole('button', { name: 'messages.download' }))
      await waitFor(() => expect(saved).toEqual(['board.png']))
    } finally {
      globalThis.HTMLAnchorElement.prototype.click = click
    }
    fireEvent.click(within(preview).getByRole('button', { name: 'preview.close' }))
    expect(screen.queryByTestId('image-preview-dialog')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'board.png' })).toBeInTheDocument()
  })

  it('plays a sent video from its attachment', async () => {
    mount()
    await openDirect()
    const video = new globalThis.File(['vid'], 'clip.mp4', { type: 'video/mp4' })
    fireEvent.change(
      globalThis.document.querySelector('[data-ui="messages.file-input"]') as globalThis.HTMLInputElement,
      {
        target: { files: [video] }
      }
    )
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(api.send).toHaveBeenCalledWith(DIRECT, expect.stringMatching(/^studio-/), {
        kind: 'file',
        attachment: expect.objectContaining({ name: 'clip.mp4', mime: 'video/mp4' })
      })
    )
    const card = await screen.findByRole('button', { name: 'clip.mp4' })
    expect(card).toHaveAttribute('data-ui', 'messages.file')
    expect(card.querySelector('[data-file-icon]')).toHaveAttribute('data-file-icon', 'video')
    expect(globalThis.document.querySelector('[data-ui="messages.thread"] video')).toBeNull()
    fireEvent.click(card)
    const preview = await screen.findByRole('dialog')
    expect(preview).toHaveAttribute('data-ui', 'messages.file-preview')
    const player = await waitFor(() => {
      const found = preview.querySelector('video[data-ui="messages.video"]')
      expect(found).toHaveAttribute('src', 'blob:attachment')
      expect(found).toHaveAttribute('aria-label', 'clip.mp4')
      return found as globalThis.HTMLVideoElement
    })
    expect(player.controls).toBe(true)
  })

  it('opens the group mention picker, inserts the chosen user, and shows mention-you until that thread is read', async () => {
    mount({
      configure: (fake) =>
        withDirectAndGroup(fake, {
          lastSequence: 2,
          lastReadSequence: 1,
          unread: 1,
          preview: {
            kind: 'text',
            messageId: `m-${GROUP}-2`,
            sequence: 2,
            recalled: false,
            createdAt: Date.parse('2026-10-02T09:00:00Z'),
            sender: { kind: 'member', id: PEER },
            text: '@林知夏 看一下',
            name: null,
            code: null,
            params: {}
          }
        })
    })
    const groupRow = () =>
      screen
        .getByText('项目组', { selector: '[data-ui="messages.conversation"] span' })
        .closest('[data-ui="messages.conversation"]')
    await screen.findByText('项目组', { selector: '[data-ui="messages.conversation"] span' })
    expect(groupRow()?.querySelector('[data-ui="messages.mention-you"]')).toHaveTextContent('messages.mention.you')

    await openDirect()
    const box = screen.getByRole('textbox', { name: 'messages.compose_placeholder' }) as globalThis.HTMLTextAreaElement
    fireEvent.change(box, { target: { value: '@周', selectionStart: 2, selectionEnd: 2 } })
    expect(screen.queryByRole('listbox', { name: 'messages.mention.picker' })).toBeNull()
    expect(groupRow()?.querySelector('[data-ui="messages.mention-you"]')).toHaveTextContent('messages.mention.you')
    fireEvent.change(box, { target: { value: '' } })

    fireEvent.click(groupRow()!)
    await screen.findByText('group hello')
    await waitFor(() => expect(groupRow()?.querySelector('[data-ui="messages.mention-you"]')).toBeNull())

    fireEvent.change(box, { target: { value: '@周', selectionStart: 2, selectionEnd: 2 } })
    await screen.findByRole('listbox', { name: 'messages.mention.picker' })
    expect(screen.queryByRole('option', { name: (value) => value.includes('林知夏') })).toBeNull()
    fireEvent.click(screen.getByRole('option', { name: (value) => value.includes('周予安') }))
    await waitFor(() => expect(box.value).toBe('@周予安 '))
    fireEvent.change(box, { target: { value: '@周予安 请看', selectionStart: 7, selectionEnd: 7 } })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const mention = await screen.findByText('@周予安', { selector: '[data-ui="messages.mention"]' })
    expect(mention).toHaveAttribute('data-mention-target', 'member')
    expect(mention).toHaveTextContent('@周予安')
  })

  it('shows elapsed seconds on the thinking chip after a short wait', () => {
    vi.useFakeTimers()
    try {
      render(<ThinkingIndicator />)
      const label = globalThis.document.querySelector('[data-ui="messages.thinking-label"]')
      expect(label).toHaveTextContent('messages.status.thinking')
      expect(screen.getByRole('status')).toHaveTextContent('messages.status.thinking')
      act(() => {
        vi.advanceTimersByTime(3000)
      })
      expect(label).toHaveTextContent('messages.status.thinking_elapsed')
      expect(screen.getByRole('status')).toHaveTextContent('messages.status.thinking')
    } finally {
      vi.useRealTimers()
    }
  })

  it('streams a ToBot reply and renders it as task markdown', async () => {
    const listeners = new Map<string, (payload: unknown) => void>()
    vi.stubGlobal('api', {
      ipcApi: {
        request: vi.fn(async (route: string) => {
          if (route === 'ai.stream.attach') return { ok: true, data: { status: 'not-found' } }
          return { ok: true, data: undefined }
        }),
        on: (event: string, callback: (payload: unknown) => void) => {
          listeners.set(event, callback)
          return () => listeners.delete(event)
        }
      }
    })
    let phase: 'empty' | 'pending' | 'done' = 'empty'
    const assistant = (status: string, text: string) => ({
      id: 'assistant-1',
      role: 'assistant',
      status,
      searchableText: text,
      data: { parts: text ? [{ type: 'text', text }] : [] }
    })
    const user = {
      id: 'user-1',
      role: 'user',
      status: 'success',
      searchableText: '你有什么能力？',
      data: { parts: [{ type: 'text', text: '你有什么能力？' }] }
    }
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      if (phase === 'empty') return { items: [] } as never
      if (phase === 'pending') return { items: [assistant('pending', ''), user] } as never
      return { items: [assistant('success', '## 核心能力\n\n**代码开发**'), user] } as never
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path) => {
      if (path === '/agent-sessions/tobot/turns') phase = 'pending'
      return { mode: 'started' } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    fireEvent.change(await screen.findByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '你有什么能力？' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    const thinking = await screen.findByRole('status')
    expect(thinking).toHaveTextContent('messages.status.thinking')
    const chip = thinking.closest('[data-ui="messages.thinking"]')
    expect(chip).not.toBeNull()
    expect(chip?.querySelectorAll('[data-ui="messages.thinking-dot"]')).toHaveLength(3)
    expect(chip?.closest('[data-ui="messages.bubble"]')).toBeNull()
    const emit = (delta: string) => {
      act(() => {
        listeners.get('ai.stream.chunk')?.({
          topicId: 'agent-session:tobot',
          anchorMessageId: 'assistant-1',
          chunk: { type: 'text-delta', id: 't', delta }
        })
      })
    }
    emit('## 核心能力\n\n**代码')
    const partial = await waitFor(() => {
      const node = globalThis.document.querySelector('[data-ui="messages.tobot-turn"] [data-task-prose]')
      expect(node?.textContent).toContain('核心能力')
      return node as globalThis.HTMLElement
    })
    expect(globalThis.document.querySelector('[data-ui="messages.thinking"]')).toBeNull()
    expect(partial.textContent).not.toContain('##')
    const streamingTurn = partial.closest('[data-ui="messages.tobot-turn"]')
    expect(streamingTurn).toHaveAttribute('data-agent-streaming', 'true')
    expect(streamingTurn?.parentElement).toHaveClass('w-full', 'max-w-[70%]', 'items-start')
    const userBubble = within(
      globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    )
      .getByText('你有什么能力？')
      .closest('[data-ui="messages.bubble"]')
    expect(userBubble?.parentElement).toHaveClass('items-end')
    expect(userBubble?.parentElement).not.toHaveClass('max-w-[70%]')
    emit('开发**')
    await waitFor(() => {
      const node = globalThis.document.querySelector('[data-ui="messages.tobot-turn"] [data-task-prose]')
      expect(node?.textContent).toContain('代码开发')
      expect(node?.textContent).not.toContain('**')
    })
    phase = 'done'
    act(() => {
      listeners.get('ai.stream.done')?.({ topicId: 'agent-session:tobot', status: 'success' })
    })
    await waitFor(() => {
      const node = globalThis.document.querySelector('[data-ui="messages.tobot-turn"] [data-task-prose]')
      expect(node?.textContent).toContain('核心能力')
      expect(node?.textContent).toContain('代码开发')
      expect(node?.closest('[data-ui="messages.tobot-turn"]')).toHaveAttribute('data-agent-streaming', 'false')
    })
  })

  it('shows one thinking chip and marks an older pending ToBot turn failed', async () => {
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          { id: 'a2', role: 'assistant', status: 'pending', searchableText: '', data: { parts: [] } },
          {
            id: 'u2',
            role: 'user',
            status: 'success',
            searchableText: '我们昨天聊了啥?',
            data: { parts: [{ type: 'text', text: '我们昨天聊了啥?' }] }
          },
          { id: 'a1', role: 'assistant', status: 'pending', searchableText: '', data: { parts: [] } },
          {
            id: 'u1',
            role: 'user',
            status: 'success',
            searchableText: '你好',
            data: { parts: [{ type: 'text', text: '你好' }] }
          }
        ]
      } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread()).findByText('我们昨天聊了啥?')
    expect(within(thread()).getByText('你好')).toBeTruthy()
    expect(globalThis.document.querySelectorAll('[data-ui="messages.thinking"]')).toHaveLength(1)
    expect(within(thread()).getAllByText('messages.tobot.turn_failed')).toHaveLength(1)
  })

  it('shows only the signed-in member ToBot transcript and ignores another session stream', async () => {
    const member = 'tobot-11111111-1111-4111-8111-111111111111'
    const listeners = new Map<string, (payload: unknown) => void>()
    vi.stubGlobal('api', {
      ipcApi: {
        request: vi.fn(async (route: string) => {
          if (route === 'ai.stream.attach') return { ok: true, data: { status: 'not-found' } }
          return { ok: true, data: undefined }
        }),
        on: (event: string, callback: (payload: unknown) => void) => {
          listeners.set(event, callback)
          return () => listeners.delete(event)
        }
      }
    })
    const line = (id: string, text: string) => ({
      id,
      role: 'assistant',
      status: 'success',
      searchableText: text,
      data: { parts: [{ type: 'text', text }] }
    })
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path) => {
      if (path === '/agent-sessions/tobot/ensure') return { sessionId: member } as never
      return { mode: 'started' } as never
    })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      const route = String(path)
      if (!route.endsWith('/messages')) return {} as never
      if (route.includes(member)) return { items: [line('mine', 'only-this-member')] } as never
      return { items: [line('shared', 'shared-secret')] } as never
    })
    mount({ org: 'personal' })
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread()).findByText('only-this-member')
    expect(within(thread()).queryByText('shared-secret')).toBeNull()
    await waitFor(() => expect(listeners.has('ai.stream.chunk')).toBe(true))
    act(() => {
      listeners.get('ai.stream.chunk')?.({
        topicId: 'agent-session:tobot',
        anchorMessageId: 'leak',
        chunk: { type: 'text-delta', id: 't', delta: 'shared-stream' }
      })
    })
    expect(within(thread()).queryByText('shared-stream')).toBeNull()
    expect(within(thread()).queryByText('shared-secret')).toBeNull()
    act(() => {
      listeners.get('ai.stream.chunk')?.({
        topicId: `agent-session:${member}`,
        anchorMessageId: 'live-mine',
        chunk: { type: 'text-delta', id: 't', delta: 'only-this-stream' }
      })
    })
    expect(within(thread()).getByText('only-this-stream')).toBeTruthy()
    expect(within(thread()).queryByText('shared-stream')).toBeNull()
  })

  it('pages older ToBot history into the dialog and keeps the live thread on the latest page', async () => {
    const line = (id: string, text: string, file?: string) => ({
      id,
      role: 'user',
      status: 'success',
      searchableText: text,
      data: {
        parts: [
          { type: 'text', text },
          ...(file ? [{ type: 'file', url: `file:///tmp/${file}`, filename: file, mediaType: 'application/pdf' }] : [])
        ]
      }
    })
    vi.spyOn(dataApiService, 'post').mockResolvedValue({ mode: 'started' })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path, options) => {
      if (!String(path).endsWith('/messages')) return {} as never
      const query = options?.query as { limit?: number; cursor?: string } | undefined
      if (query?.limit === 200 && query.cursor === '1')
        return { items: [line('older', '更早的那句', 'older.pdf')] } as never
      if (query?.limit === 200) return { items: [line('latest', '刚刚这句')], nextCursor: '1' } as never
      return { items: [line('latest', '刚刚这句')] } as never
    })
    mount({ org: 'personal' })
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread()).findByText('刚刚这句')
    fireEvent.click(screen.getByRole('button', { name: 'messages.history.open' }))
    const history = await screen.findByRole('dialog')
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    await within(history).findByText('更早的那句')
    expect(within(history).getByText('刚刚这句')).toBeTruthy()
    expect(within(history).getByText('messages.preview.file older.pdf')).toBeTruthy()
    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.file' }))
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(history).getByText('messages.preview.file older.pdf')).toBeTruthy()
    expect(within(history).queryByText('更早的那句')).toBeNull()
    expect(within(history).queryByText('刚刚这句')).toBeNull()
    expect(within(thread()).getByText('刚刚这句')).toBeTruthy()
    expect(within(thread()).queryByText('更早的那句')).toBeNull()
  })

  it('asks before resetting ToBot and clears the thread only after confirm', async () => {
    const post = vi.spyOn(dataApiService, 'post').mockResolvedValue({ ok: true })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          {
            id: 'a1',
            role: 'assistant',
            status: 'success',
            searchableText: '昨天聊了价格',
            data: { parts: [{ type: 'text', text: '昨天聊了价格' }] }
          }
        ]
      } as never
    })
    mount()
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    expect(screen.queryByRole('button', { name: 'messages.tobot.reset' })).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()

    cleanup()
    __resetSharedImStore()
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await within(thread()).findByText('昨天聊了价格')
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.tobot.reset' }))
    const dialog = await screen.findByRole('dialog')
    expect(dialog).toHaveTextContent('messages.tobot.reset_body')
    fireEvent.click(within(dialog).getByRole('button', { name: 'common.cancel' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/reset')).toBe(false)
    expect(within(thread()).getByText('昨天聊了价格')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.tobot.reset' }))
    fireEvent.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'messages.tobot.reset_action' })
    )
    await waitFor(() => expect(post.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/reset')).toBe(true))
    await waitFor(() => expect(within(thread()).queryByText('昨天聊了价格')).toBeNull())
    expect(previewOf(BUILTIN_TOBOT_ID)).not.toHaveTextContent('昨天聊了价格')
  })

  it('shows an allow or deny card when ToBot asks to use a tool', async () => {
    const request = vi.fn(async (route: string) => {
      if (route === 'ai.stream.attach') return { ok: true, data: { status: 'not-found' } }
      if (route === 'ai.tool.respond_approval') return { ok: true, data: { ok: true } }
      return { ok: true, data: undefined }
    })
    const listeners = new Map<string, (payload: unknown) => void>()
    vi.stubGlobal('api', {
      ipcApi: {
        request,
        on: (event: string, callback: (payload: unknown) => void) => {
          listeners.set(event, callback)
          return () => listeners.delete(event)
        }
      }
    })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          { id: 'a1', role: 'assistant', status: 'pending', searchableText: '', data: { parts: [] } },
          {
            id: 'u1',
            role: 'user',
            status: 'success',
            searchableText: '看这张图',
            data: { parts: [{ type: 'text', text: '看这张图' }] }
          }
        ]
      } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('status')
    act(() => {
      listeners.get('ai.stream.chunk')?.({
        topicId: 'agent-session:tobot',
        anchorMessageId: 'a1',
        chunk: { type: 'tool-input-available', toolCallId: 'call-1', toolName: 'bash' }
      })
      listeners.get('ai.stream.chunk')?.({
        topicId: 'agent-session:tobot',
        anchorMessageId: 'a1',
        chunk: { type: 'tool-approval-request', approvalId: 'approval-1', toolCallId: 'call-1' }
      })
    })
    const card = await screen.findByText('messages.tobot.approval')
    expect(card.closest('[data-ui="messages.tobot-approval"]')).toHaveTextContent('bash')
    expect(globalThis.document.querySelector('[data-ui="messages.thinking"]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'messages.tobot.approval_allow' }))
    await waitFor(() =>
      expect(request).toHaveBeenCalledWith(
        'ai.tool.respond_approval',
        expect.objectContaining({ approvalId: 'approval-1', approved: true, topicId: 'agent-session:tobot' })
      )
    )
    expect(screen.queryByText('messages.tobot.approval')).toBeNull()
  })

  it('keeps the approval card after ToBot has already written', async () => {
    const listeners = new Map<string, (payload: unknown) => void>()
    vi.stubGlobal('api', {
      ipcApi: {
        request: vi.fn(async (route: string) =>
          route === 'ai.stream.attach' ? { ok: true, data: { status: 'not-found' } } : { ok: true, data: undefined }
        ),
        on: (event: string, callback: (payload: unknown) => void) => {
          listeners.set(event, callback)
          return () => listeners.delete(event)
        }
      }
    })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          { id: 'a1', role: 'assistant', status: 'pending', searchableText: '', data: { parts: [] } },
          {
            id: 'u1',
            role: 'user',
            status: 'success',
            searchableText: '你好',
            data: { parts: [{ type: 'text', text: '你好' }] }
          }
        ]
      } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('status')
    const emit = (chunk: Record<string, unknown>) => {
      act(() => {
        listeners.get('ai.stream.chunk')?.({
          topicId: 'agent-session:tobot',
          anchorMessageId: 'a1',
          chunk
        })
      })
    }
    emit({ type: 'text-delta', id: 't', delta: '我是小朱Bot。先看一下工作区。' })
    emit({ type: 'tool-input-available', toolCallId: 'call-1', toolName: 'bash', input: { command: 'find .' } })
    emit({ type: 'tool-approval-request', approvalId: 'approval-1', toolCallId: 'call-1' })
    const thread = () => globalThis.document.querySelector('[data-ui="messages.thread"]') as globalThis.HTMLElement
    await waitFor(() => expect(within(thread()).getByText(/先看一下工作区/)).toBeTruthy())
    expect(
      within(thread()).getByText('messages.tobot.approval').closest('[data-ui="messages.tobot-approval"]')
    ).toHaveTextContent('bash')
    expect(globalThis.document.querySelector('[data-task-tool-name="bash"]')).toBeTruthy()
    expect(globalThis.document.querySelector('[data-ui="messages.thinking"]')).toBeNull()
  })

  it('scrolls the thread to the bottom when the user sends', async () => {
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return { items: [] } as never
    })
    vi.spyOn(dataApiService, 'post').mockResolvedValue({ mode: 'started' })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    const scroller = globalThis.document.querySelector('[data-ui="messages.thread-scroll"]') as globalThis.HTMLElement
    let scrollTop = 40
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, get: () => 2400 })
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, get: () => 500 })
    Object.defineProperty(scroller, 'scrollTop', {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = value
      }
    })
    fireEvent.scroll(scroller)
    expect(scrollTop).toBe(40)
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: '你好' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() => expect(scrollTop).toBe(2400))
  })

  it('edits the ToBot name, face, and personality from the row menu', async () => {
    const post = vi.spyOn(dataApiService, 'post').mockResolvedValue({ mode: 'started' })
    mount()
    await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.contextMenu(globalThis.document.querySelector(`[data-conversation-id="${DIRECT}"]`)!)
    expect(screen.queryByRole('menuitem', { name: 'messages.tobot.edit' })).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()

    cleanup()
    __resetSharedImStore()
    mount({ org: 'personal' })
    const row = globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)
    expect(row).toBeTruthy()
    fireEvent.contextMenu(row!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.tobot.edit' }))
    const dialog = await screen.findByRole('dialog')
    expect(dialog).toHaveAttribute('data-ui', 'messages.tobot-profile')
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'messages.tobot.name' }), {
      target: { value: '小团' }
    })
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'messages.tobot.personality' }), {
      target: { value: '温和，只用短句' }
    })
    fireEvent.click(within(dialog).getByRole('radio', { name: 'messages.tobot.shape.drop' }))
    fireEvent.click(within(dialog).getByRole('radio', { name: 'messages.tobot.expression.glad' }))
    fireEvent.click(within(dialog).getByRole('button', { name: 'messages.tobot.save' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

    const titled = globalThis.document.querySelector(
      `[data-conversation-id="${BUILTIN_TOBOT_ID}"] [data-ui="messages.conversation-title"]`
    )
    expect(titled).toHaveTextContent('小团')
    expect(
      globalThis.document.querySelector(
        `[data-conversation-id="${BUILTIN_TOBOT_ID}"] [data-ui="messages.tobot-avatar"]`
      )
    ).toHaveAttribute('data-shape', 'drop')

    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    fireEvent.change(await screen.findByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: 'hello bot' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
    await waitFor(() =>
      expect(post.mock.calls.find((call) => call[0] === '/agent-sessions/tobot/turns')?.[1]).toEqual({
        body: {
          userMessageParts: [{ type: 'text', text: 'hello bot' }],
          hostContext: { personaName: '小团', persona: '温和，只用短句' },
          ...closedMailbox,
          harnessComponents
        }
      })
    )
    await waitFor(() =>
      expect(post.mock.calls.filter((call) => call[0] === '/agent-sessions/tobot/ensure').at(-1)?.[1]).toEqual({
        body: { hostContext: { personaName: '小团', persona: '温和，只用短句' }, harnessComponents }
      })
    )
  })

  it('keeps the phone and wallet controls off the ToBot header', async () => {
    mount()
    await openDirect()
    expect(screen.queryByRole('button', { name: 'messages.tobot.phone' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'messages.tobot.wallet' })).toBeNull()
    expect(globalThis.document.querySelector(`[data-conversation-id="${TRANSFER}"]`)).toBeNull()

    cleanup()
    __resetSharedImStore()
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('textbox', { name: 'messages.compose_placeholder' })
    expect(screen.queryByRole('button', { name: 'messages.tobot.phone' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'messages.tobot.wallet' })).toBeNull()
    expect(screen.getByRole('button', { name: 'messages.history.open' })).toBeInTheDocument()
  })

  it('searches the open chat and jumps to the matching message', async () => {
    mount({
      configure: (fake) => {
        fake.messages.mockResolvedValue([
          message(2),
          message(1),
          message(3, {
            body: {
              kind: 'file',
              attachment: { name: 'notes.pdf', sizeBytes: 4, objectKey: 'im/notes.pdf', mime: 'application/pdf' }
            }
          }),
          message(4, {
            body: {
              kind: 'file',
              attachment: {
                name: 'budget.xlsx',
                sizeBytes: 4,
                objectKey: 'im/budget.xlsx',
                mime: 'application/vnd.ms-excel'
              }
            }
          })
        ])
      }
    })
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.history.open' }))
    const history = await screen.findByRole('dialog')
    expect(history).toHaveTextContent('messages.history.title')
    const content =
      history.querySelector('[data-testid="dialog-content"]') ??
      history.querySelector('[data-slot="dialog-content"]') ??
      history
    expect(content.className).toContain('h-[560px]')
    expect(content.className.split(/\s+/)).not.toContain('relative')
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    const pane = history.querySelector('[data-ui="messages.history"]') as globalThis.HTMLElement
    expect(within(pane).getByText('hello 1')).toBeInTheDocument()
    expect(within(pane).getByText('hello 2')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file notes.pdf')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file budget.xlsx')).toBeInTheDocument()
    fireEvent.change(within(history).getByRole('searchbox', { name: 'messages.history.search' }), {
      target: { value: 'budget' }
    })
    expect(within(pane).queryByText('hello 1')).toBeNull()
    expect(within(pane).queryByText('messages.preview.file notes.pdf')).toBeNull()
    fireEvent.click(within(pane).getByRole('button', { name: /budget/ }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(globalThis.document.querySelector(`[data-message-id="m-${DIRECT}-4"]`)).toHaveAttribute(
      'data-message-focus',
      'true'
    )
  })

  it('filters chat history with a date range and keeps that range across tabs', async () => {
    mount({
      configure: (fake) => {
        fake.messages.mockResolvedValue([
          message(2),
          message(1),
          message(3, {
            body: {
              kind: 'file',
              attachment: { name: 'notes.pdf', sizeBytes: 4, objectKey: 'im/notes.pdf', mime: 'application/pdf' }
            }
          }),
          message(4, {
            createdAt: Date.parse('2026-10-01T12:00:00Z'),
            body: {
              kind: 'file',
              attachment: { name: 'old.pdf', sizeBytes: 4, objectKey: 'im/old.pdf', mime: 'application/pdf' }
            }
          })
        ])
      }
    })
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.history.open' }))
    const history = await screen.findByRole('dialog')
    const pane = history.querySelector('[data-ui="messages.history"]') as globalThis.HTMLElement
    const search = () => within(history).getByRole('searchbox', { name: 'messages.history.search' })
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(pane).getByText('hello 1')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file notes.pdf')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file old.pdf')).toBeInTheDocument()
    fireEvent.change(search(), { target: { value: 'notes' } })
    expect(within(pane).queryByText('hello 1')).toBeNull()
    expect(within(pane).queryByText('messages.preview.file old.pdf')).toBeNull()
    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.date' }))
    const picker = history.querySelector('[data-ui="messages.history-range"]') as globalThis.HTMLElement
    expect(picker).toBeTruthy()
    expect(search()).toHaveValue('')
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.date' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(pane).getByText('hello 1')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file notes.pdf')).toBeInTheDocument()
    fireEvent.click(within(picker).getByRole('button', { name: '2026-10-01' }))
    fireEvent.click(within(picker).getByRole('button', { name: 'common.confirm' }))
    await waitFor(() => expect(history.querySelector('[data-ui="messages.history-range"]')).toBeNull())
    expect(within(pane).queryByText('hello 1')).toBeNull()
    expect(within(pane).queryByText('messages.preview.file notes.pdf')).toBeNull()
    expect(within(pane).getByText('messages.preview.file old.pdf')).toBeInTheDocument()
    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.date' }))
    fireEvent.click(
      within(history.querySelector('[data-ui="messages.history-range"]') as globalThis.HTMLElement).getByRole(
        'button',
        {
          name: 'messages.history.range_clear'
        }
      )
    )
    expect(within(pane).getByText('hello 1')).toBeInTheDocument()
    expect(within(pane).getByText('messages.preview.file notes.pdf')).toBeInTheDocument()
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.date' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )

    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.date' }))
    fireEvent.click(
      within(history.querySelector('[data-ui="messages.history-range"]') as globalThis.HTMLElement).getByRole(
        'button',
        {
          name: '2026-10-01'
        }
      )
    )
    fireEvent.click(
      within(history.querySelector('[data-ui="messages.history-range"]') as globalThis.HTMLElement).getByRole(
        'button',
        {
          name: 'common.confirm'
        }
      )
    )
    fireEvent.change(search(), { target: { value: 'old' } })
    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.image' }))
    await waitFor(() => expect(history.querySelector('[data-ui="messages.history-range"]')).toBeNull())
    expect(search()).toHaveValue('')
    expect(within(history).getByRole('button', { name: 'messages.history.date' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.image' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(pane).getByRole('button', { name: 'messages.history.range_clear' })).toHaveTextContent('2026-10-01')
  })

  it('clears chat history filters and resets them when the dialog closes', async () => {
    mount()
    await openDirect()
    fireEvent.click(screen.getByRole('button', { name: 'messages.history.open' }))
    const history = await screen.findByRole('dialog')
    const pane = () => history.querySelector('[data-ui="messages.history"]') as globalThis.HTMLElement
    const search = () => within(history).getByRole('searchbox', { name: 'messages.history.search' })

    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(pane()).getByText('hello 1')).toBeInTheDocument()
    expect(within(pane()).queryByText('messages.history.no_match')).toBeNull()
    expect(pane().querySelector('[data-ui="messages.history-clear"]')).toBeNull()
    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.all' }))
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(pane()).getByText('hello 1')).toBeInTheDocument()

    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.file' }))
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(pane()).queryByText('hello 1')).toBeNull()
    expect(within(pane()).getByText('messages.history.no_match')).toBeInTheDocument()
    fireEvent.click(pane().querySelector('[data-ui="messages.history-clear"]') as globalThis.HTMLElement)
    expect(within(pane()).getByText('hello 1')).toBeInTheDocument()
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(history.querySelector('[data-ui="messages.history-clear"]')).toBeNull()

    fireEvent.change(search(), { target: { value: 'hello 2' } })
    expect(search()).toHaveValue('hello 2')
    expect(within(pane()).queryByText('hello 1')).toBeNull()
    expect(within(pane()).getByText('hello 2')).toBeInTheDocument()
    fireEvent.click(history.querySelector('[data-ui="messages.history-clear"]') as globalThis.HTMLElement)
    expect(search()).toHaveValue('')
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(pane()).getByText('hello 1')).toBeInTheDocument()
    expect(history.querySelector('[data-ui="messages.history-clear"]')).toBeNull()

    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.date' }))
    const picker = history.querySelector('[data-ui="messages.history-range"]') as globalThis.HTMLElement
    fireEvent.click(within(picker).getByRole('button', { name: '2026-10-01' }))
    fireEvent.click(within(picker).getByRole('button', { name: 'common.confirm' }))
    await waitFor(() => expect(history.querySelector('[data-ui="messages.history-range"]')).toBeNull())
    expect(within(pane()).getByText('messages.history.no_match')).toBeInTheDocument()
    expect(within(pane()).getByRole('button', { name: 'messages.history.range_clear' })).toHaveTextContent('2026-10-01')
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    fireEvent.click(pane().querySelector('[data-ui="messages.history-clear"]') as globalThis.HTMLElement)
    expect(within(pane()).queryByRole('button', { name: 'messages.history.range_clear' })).toBeNull()
    expect(within(history).getByRole('button', { name: 'messages.history.date' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(history).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(within(pane()).getByText('hello 1')).toBeInTheDocument()
    expect(history.querySelector('[data-ui="messages.history-clear"]')).toBeNull()

    fireEvent.click(within(history).getByRole('button', { name: 'messages.history.image' }))
    fireEvent.change(search(), { target: { value: 'hello' } })
    fireEvent.click(within(history).getByRole('button', { name: 'common.close' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    fireEvent.click(screen.getByRole('button', { name: 'messages.history.open' }))
    const again = await screen.findByRole('dialog')
    const againPane = again.querySelector('[data-ui="messages.history"]') as globalThis.HTMLElement
    expect(within(again).getByRole('button', { name: 'messages.history.all' })).toHaveAttribute('aria-pressed', 'true')
    expect(within(again).getByRole('button', { name: 'messages.history.file' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(again).getByRole('button', { name: 'messages.history.image' })).toHaveAttribute(
      'aria-pressed',
      'false'
    )
    expect(within(again).getByRole('searchbox', { name: 'messages.history.search' })).toHaveValue('')
    expect(within(againPane).getByText('hello 1')).toBeInTheDocument()
    expect(again.querySelector('[data-ui="messages.history-clear"]')).toBeNull()
  })

  it('forwards the selected records into another chat', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry(),
          entry({ id: GROUP, kind: 'group', title: '项目组', peerUserId: null, memberCount: 3, preview: null })
        ])
      }
    })
    await openDirect()
    fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.forward' }))
    const picked = screen.getByRole('checkbox', { name: 'hello 1' })
    expect(picked).toHaveAttribute('aria-checked', 'true')
    expect(picked.getAttribute('style')).toContain('background-color: rgb(23, 23, 23)')
    expect(picked.querySelector('path')).toHaveAttribute('stroke', '#ffffff')
    fireEvent.click(screen.getByRole('checkbox', { name: 'hello 2' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.forward_each' }))
    const picker = await screen.findByRole('dialog')
    expect(within(picker).queryByRole('button', { name: '周予安' })).toBeNull()
    expect(
      within(picker).getByRole('button', { name: '项目组' }).querySelector('[data-ui="messages.group-avatar"]')
    ).toBeTruthy()
    fireEvent.click(within(picker).getByRole('button', { name: '项目组' }))
    fireEvent.click(within(picker).getByRole('button', { name: 'messages.forward_send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(2))
    expect(api.send.mock.calls.map((call) => [call[0], call[2]])).toEqual([
      [GROUP, { kind: 'text', text: 'hello 1' }],
      [GROUP, { kind: 'text', text: 'hello 2' }]
    ])
    expect(globalThis.document.querySelector('[data-ui="messages.forward-bar"]')).toBeNull()
  })

  it('sends one combined chat record', async () => {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry(),
          entry({ id: GROUP, kind: 'group', title: '项目组', peerUserId: null, memberCount: 3, preview: null })
        ])
      }
    })
    await openDirect()
    fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.forward' }))
    fireEvent.click(screen.getByRole('checkbox', { name: 'hello 2' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.forward_merge' }))
    const picker = await screen.findByRole('dialog')
    fireEvent.click(within(picker).getByRole('button', { name: '项目组' }))
    fireEvent.click(within(picker).getByRole('button', { name: 'messages.forward_send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(1))
    const body = api.send.mock.calls[0][2] as { kind: string; text: string }
    expect(body.kind).toBe('text')
    expect(body.text).toContain('[[a3s-chat-record]]')
    expect(body.text).toContain('周予安\thello 1')
    expect(body.text).toContain('周予安\thello 2')
    expect(api.send.mock.calls[0][0]).toBe(GROUP)
  })

  it('filters forward targets by department and shows the colleague avatar', async () => {
    mount({
      configure: (fake) => {
        fake.startDirect.mockResolvedValue('cccccccc-0000-4000-8000-000000000099')
      }
    })
    await openDirect()
    fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.forward' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.forward_each' }))
    const picker = await screen.findByRole('dialog')
    fireEvent.click(within(picker).getByRole('button', { name: '设计部' }))
    const person = within(picker).getByRole('button', { name: '陈可' })
    expect(person.querySelector('[data-ui="messages.person-avatar"]')).toBeTruthy()
    expect(within(picker).queryByRole('button', { name: '林知夏' })).toBeNull()
    fireEvent.change(within(picker).getByRole('searchbox', { name: 'messages.forward_search' }), {
      target: { value: '陈可' }
    })
    fireEvent.click(within(picker).getByRole('button', { name: '陈可' }))
    fireEvent.click(within(picker).getByRole('button', { name: 'messages.forward_send' }))
    await waitFor(() => expect(api.send).toHaveBeenCalled())
    expect(api.startDirect).toHaveBeenCalledWith(THIRD)
    expect(api.send.mock.calls[0][0]).toBe('cccccccc-0000-4000-8000-000000000099')
  })

  it('does not offer the personal ToBot as a team forward target', async () => {
    const posts: unknown[] = []
    vi.spyOn(dataApiService, 'post').mockImplementation(async (path, options) => {
      if (path === '/agent-sessions/tobot/turns') posts.push(options)
      return { mode: 'started' } as never
    })
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry(),
          entry({ id: TRANSFER, kind: 'transfer', title: null, peerUserId: null, memberCount: 1, preview: null })
        ])
      }
    })
    await openDirect()
    fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.forward' }))
    fireEvent.click(screen.getByRole('checkbox', { name: 'hello 2' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.forward_merge' }))
    const picker = await screen.findByRole('dialog')
    expect(within(picker).queryByRole('button', { name: 'messages.file_transfer' })).toBeNull()
    expect(posts).toHaveLength(0)
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(api.send).not.toHaveBeenCalled()
  })

  it('shows a merged chat record as a card', async () => {
    const text = encodeChatRecord('林知夏和周予安的聊天记录', [
      { sender: '周予安', text: 'hello 1' },
      { sender: '林知夏', text: 'hello 2' }
    ])
    mount({
      configure: (fake) => {
        fake.messages.mockResolvedValue([message(1, { body: { kind: 'text', text } })])
      }
    })
    const row = await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' })
    fireEvent.click(row)
    const card = await screen.findByRole('button', { name: /林知夏和周予安的聊天记录/ })
    expect(card).toHaveAttribute('data-ui', 'messages.chat-record')
    expect(card.textContent).not.toContain('[[a3s-chat-record]]')
    fireEvent.click(card)
    const view = await screen.findByRole('dialog')
    expect(within(view).getByText('hello 2')).toBeInTheDocument()
  })

  it('quotes a message inside the reply and hides the wire suffix', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    const previous = Object.getOwnPropertyDescriptor(globalThis.navigator, 'clipboard')
    Object.defineProperty(globalThis.navigator, 'clipboard', { configurable: true, value: { writeText } })
    try {
      mount()
      await openDirect()
      fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
      expect(screen.getByRole('menuitem', { name: 'messages.forward' })).toBeInTheDocument()
      fireEvent.click(screen.getByRole('menuitem', { name: 'messages.quote' }))
      const draft = globalThis.document.querySelector('[data-ui="messages.quote-draft"]') as globalThis.HTMLElement
      expect(draft).toHaveTextContent('周予安')
      expect(draft).toHaveTextContent('hello 1')
      expect(screen.getByRole('button', { name: 'messages.send' })).toBeDisabled()
      fireEvent.keyDown(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), { key: 'Escape' })
      expect(globalThis.document.querySelector('[data-ui="messages.quote-draft"]')).toBeNull()

      fireEvent.contextMenu(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' }))
      fireEvent.click(screen.getByRole('menuitem', { name: 'messages.quote' }))
      fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
        target: { value: '收到' }
      })
      fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
      await waitFor(() => expect(api.send).toHaveBeenCalled())
      const sent = api.send.mock.calls.at(-1)?.[2] as { kind: string; text?: string }
      expect(sent.kind).toBe('text')
      expect(sent.text?.startsWith('收到')).toBe(true)
      expect(sent.text).toContain('A3SQ1:')
      const bubble = await screen.findByText('收到', { selector: '[data-ui="messages.bubble"]' })
      const quote = bubble.querySelector('[data-ui="messages.quote"]')
      expect(quote).toHaveTextContent('周予安')
      expect(quote).toHaveTextContent('hello 1')
      expect(quote).toHaveAttribute('data-quote-id', `m-${DIRECT}-1`)
      expect(globalThis.document.body.textContent?.includes('A3SQ1')).toBe(false)

      fireEvent.contextMenu(bubble)
      fireEvent.click(screen.getByRole('menuitem', { name: 'common.copy' }))
      await waitFor(() => expect(writeText).toHaveBeenCalledWith('收到'))
    } finally {
      if (previous) Object.defineProperty(globalThis.navigator, 'clipboard', previous)
      else delete (globalThis.navigator as { clipboard?: globalThis.Clipboard }).clipboard
    }
  })

  function threadPane(): globalThis.HTMLElement {
    const pane = globalThis.document.querySelector('[data-ui="messages.thread"]')
    if (!pane) throw new Error('missing thread')
    return pane as globalThis.HTMLElement
  }

  function memberRow(name: string): globalThis.HTMLElement {
    const row = [...globalThis.document.querySelectorAll('[data-ui="messages.member"]')].find((item) =>
      item.textContent?.includes(name)
    )
    if (!row) throw new Error(`missing member ${name}`)
    return row as globalThis.HTMLElement
  }

  function mountManagedGroup(members: ImMember[], muteAll = false) {
    mount({
      configure: (fake) => {
        fake.conversations.mockResolvedValue([
          entry({ id: GROUP, kind: 'group', title: '设计评审', peerUserId: null, preview: null, unread: 0 })
        ])
        fake.detail.mockResolvedValue(
          detail({
            id: GROUP,
            kind: 'group',
            title: '设计评审',
            muteAll,
            memberCount: members.length,
            members
          })
        )
        fake.messages.mockResolvedValue([
          message(1, {
            conversationId: GROUP,
            sender: { kind: 'member', id: PEER },
            body: { kind: 'text', text: 'hello' }
          })
        ])
      }
    })
  }

  async function openManagedGroup() {
    fireEvent.click(await screen.findByText('设计评审'))
    await screen.findByText('hello')
  }

  async function openBossGroup() {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    globalThis.localStorage.setItem(`messages.toboss.groups.${ORG}`, JSON.stringify([GROUP]))
    vi.spyOn(dataApiService, 'post').mockImplementation(() => new Promise(() => undefined))
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' })
    await openGroup()
  }

  function askBoss(text: string) {
    fireEvent.change(screen.getByRole('textbox', { name: 'messages.compose_placeholder' }), {
      target: { value: `@messages.toboss ${text}` }
    })
    fireEvent.click(screen.getByRole('button', { name: 'messages.send' }))
  }

  it('offers recall only inside the 5-minute window and the recall control is absent at 5 minutes', async () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date('2026-10-13T08:00:00.000Z'))
    try {
      const now = Date.now()
      mount({
        configure: (fake) => {
          fake.messages.mockResolvedValue([
            message(1, {
              sender: { kind: 'member', id: ME },
              body: { kind: 'text', text: 'inside window' },
              createdAt: now - RECALL_WINDOW_MS + 1
            }),
            message(2, {
              sender: { kind: 'member', id: ME },
              body: { kind: 'text', text: 'at boundary' },
              createdAt: now - RECALL_WINDOW_MS
            }),
            message(3, {
              sender: { kind: 'member', id: PEER },
              body: { kind: 'text', text: 'peer line' },
              createdAt: now - 1000
            }),
            message(4, {
              sender: { kind: 'member', id: ME },
              body: { kind: 'system', code: 'no-such-code', params: {} },
              createdAt: now - 1000
            })
          ])
        }
      })
      fireEvent.click(await screen.findByText('周予安', { selector: '[data-ui="messages.conversation"] span' }))
      await screen.findByText('inside window', { selector: '[data-ui="messages.bubble"]' })
      const recalls = globalThis.document.querySelectorAll('[data-ui="messages.recall"]')
      expect(recalls).toHaveLength(1)
      expect(recalls[0]?.closest('[data-message-id]')?.textContent).toContain('inside window')
      fireEvent.contextMenu(screen.getByText('inside window', { selector: '[data-ui="messages.bubble"]' }))
      expect(screen.getByRole('menuitem', { name: 'messages.recall_action' })).toBeInTheDocument()
      fireEvent.keyDown(globalThis.document, { key: 'Escape' })

      fireEvent.contextMenu(screen.getByText('at boundary', { selector: '[data-ui="messages.bubble"]' }))
      expect(screen.queryByRole('menuitem', { name: 'messages.recall_action' })).toBeNull()
      expect(globalThis.document.querySelectorAll('[data-ui="messages.recall"]')).toHaveLength(1)
      fireEvent.keyDown(globalThis.document, { key: 'Escape' })

      fireEvent.contextMenu(screen.getByText('peer line', { selector: '[data-ui="messages.bubble"]' }))
      expect(screen.queryByRole('menuitem', { name: 'messages.recall_action' })).toBeNull()
      expect(screen.getByText('messages.system.unknown')).toBeInTheDocument()
      expect(globalThis.document.querySelectorAll('[data-ui="messages.recall"]')).toHaveLength(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('restores the bubble after a failed delete', async () => {
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          {
            id: 'keep-line',
            role: 'user',
            status: 'success',
            searchableText: 'keep this bubble',
            data: { parts: [{ type: 'text', text: 'keep this bubble' }] }
          }
        ]
      } as never
    })
    let rejectDelete = () => undefined
    const deleted = vi.spyOn(dataApiService, 'delete').mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectDelete = () => reject(new Error('delete failed'))
        })
    )
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await within(threadPane()).findByText('keep this bubble')
    fireEvent.contextMenu(
      within(threadPane()).getByText('keep this bubble', { selector: '[data-ui="messages.bubble"]' })
    )
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.delete' }))
    await waitFor(() => expect(within(threadPane()).queryByText('keep this bubble')).toBeNull())
    expect(deleted.mock.calls.some((call) => call[0] === '/agent-sessions/tobot/messages/keep-line')).toBe(true)
    act(() => rejectDelete())
    await within(threadPane()).findByText('keep this bubble')
  })

  it('retries a failed send and can discard it', async () => {
    let attempts = 0
    mount({
      configure: (fake) => {
        fake.send.mockImplementation(async (conversationId, _clientMsgId, body) => {
          attempts += 1
          if (attempts === 1) throw new Error('down')
          return message(9, {
            conversationId,
            sender: { kind: 'member', id: ME },
            createdAt: Date.now(),
            body: body.kind === 'text' ? body : { kind: 'text', text: 'retry me' }
          })
        })
      }
    })
    await openDirect()
    await sendDraft('retry me')
    expect(await within(threadPane()).findByText('messages.status.failed')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'messages.retry' }))
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(2))
    expect(within(threadPane()).getByText('retry me', { selector: '[data-ui="messages.bubble"]' })).toBeInTheDocument()
    expect(within(threadPane()).queryByText('messages.status.failed')).toBeNull()

    cleanup()
    __resetSharedImStore()
    mount({
      configure: (fake) => {
        fake.send.mockRejectedValue(new Error('down'))
      }
    })
    await openDirect()
    await sendDraft('drop me')
    expect(await within(threadPane()).findByText('messages.status.failed')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'messages.discard' }))
    await waitFor(() => expect(within(threadPane()).queryByText('drop me')).toBeNull())
    expect(api.send).toHaveBeenCalledTimes(1)
  })

  it('disables a member composer under mute-all while the owner and admin composers stay enabled', async () => {
    const composer = () => screen.getByRole('textbox', { name: 'messages.compose_placeholder' })
    mountManagedGroup(
      [
        { userId: ME, role: 'member', left: false, lastReadSequence: 0 },
        { userId: PEER, role: 'owner', left: false, lastReadSequence: 0 }
      ],
      true
    )
    await openManagedGroup()
    expect(composer()).toBeDisabled()
    expect(globalThis.document.querySelector('[data-ui="messages.speech-block"]')).toHaveTextContent(
      'messages.speech.muted_all'
    )

    cleanup()
    __resetSharedImStore()
    mountManagedGroup(
      [
        { userId: ME, role: 'owner', left: false, lastReadSequence: 0 },
        { userId: PEER, role: 'member', left: false, lastReadSequence: 0 }
      ],
      true
    )
    await openManagedGroup()
    expect(composer()).toBeEnabled()
    expect(globalThis.document.querySelector('[data-ui="messages.speech-block"]')).toBeNull()

    cleanup()
    __resetSharedImStore()
    mountManagedGroup(
      [
        { userId: ME, role: 'admin', left: false, lastReadSequence: 0 },
        { userId: PEER, role: 'owner', left: false, lastReadSequence: 0 }
      ],
      true
    )
    await openManagedGroup()
    expect(composer()).toBeEnabled()
    expect(globalThis.document.querySelector('[data-ui="messages.speech-block"]')).toBeNull()
  })

  it('blocks the composer when that member has an individual speech ban', async () => {
    mountManagedGroup([
      { userId: ME, role: 'member', left: false, lastReadSequence: 0, speechBanned: true },
      { userId: PEER, role: 'owner', left: false, lastReadSequence: 0 }
    ])
    await openManagedGroup()
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeDisabled()
    expect(globalThis.document.querySelector('[data-ui="messages.speech-block"]')).toHaveTextContent(
      'messages.speech.banned'
    )
  })

  it('does not let the owner be removed or banned, and an admin cannot ban or remove another admin or the owner', async () => {
    mountManagedGroup([
      { userId: ME, role: 'admin', left: false, lastReadSequence: 0 },
      { userId: PEER, role: 'owner', left: false, lastReadSequence: 0 },
      { userId: THIRD, role: 'admin', left: false, lastReadSequence: 0 }
    ])
    await openManagedGroup()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    expect(within(memberRow('周予安')).queryByRole('button', { name: 'messages.member_actions' })).toBeNull()
    expect(within(memberRow('陈可')).queryByRole('button', { name: 'messages.member_actions' })).toBeNull()

    cleanup()
    __resetSharedImStore()
    mountManagedGroup([
      { userId: ME, role: 'owner', left: false, lastReadSequence: 0 },
      { userId: PEER, role: 'member', left: false, lastReadSequence: 0 }
    ])
    await openManagedGroup()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    expect(within(memberRow('林知夏')).queryByRole('button', { name: 'messages.member_actions' })).toBeNull()
    fireEvent.click(within(memberRow('周予安')).getByRole('button', { name: 'messages.member_actions' }))
    expect(screen.getByRole('menuitem', { name: 'messages.speech.ban' })).toBeInTheDocument()
    expect(screen.getByRole('menuitem', { name: 'messages.remove_member' })).toBeInTheDocument()
  })

  it('opens a confirm for remove-member and leave, and cancel does not call the server', async () => {
    mountManagedGroup([
      { userId: ME, role: 'owner', left: false, lastReadSequence: 0 },
      { userId: PEER, role: 'member', left: false, lastReadSequence: 0 }
    ])
    await openManagedGroup()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(within(memberRow('周予安')).getByRole('button', { name: 'messages.member_actions' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.remove_member' }))
    const removeDialog = await screen.findByRole('dialog', { name: 'messages.remove_confirm_title' })
    fireEvent.click(within(removeDialog).getByRole('button', { name: 'common.cancel' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'messages.remove_confirm_title' })).toBeNull())
    expect(api.removeMember).not.toHaveBeenCalled()

    fireEvent.click(within(memberRow('周予安')).getByRole('button', { name: 'messages.member_actions' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'messages.remove_member' }))
    fireEvent.click(
      within(await screen.findByRole('dialog', { name: 'messages.remove_confirm_title' })).getByRole('button', {
        name: 'messages.remove_member'
      })
    )
    await waitFor(() => expect(api.removeMember).toHaveBeenCalledWith(GROUP, PEER))

    cleanup()
    __resetSharedImStore()
    mountManagedGroup([
      { userId: ME, role: 'member', left: false, lastReadSequence: 0 },
      { userId: PEER, role: 'owner', left: false, lastReadSequence: 0 }
    ])
    await openManagedGroup()
    fireEvent.click(screen.getByRole('button', { name: 'messages.action.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'messages.leave' }))
    const leaveDialog = await screen.findByRole('dialog', { name: 'messages.leave_confirm_title' })
    fireEvent.click(within(leaveDialog).getByRole('button', { name: 'common.cancel' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'messages.leave_confirm_title' })).toBeNull())
    expect(api.leave).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'messages.leave' }))
    fireEvent.click(
      within(await screen.findByRole('dialog', { name: 'messages.leave_confirm_title' })).getByRole('button', {
        name: 'messages.leave'
      })
    )
    await waitFor(() => expect(api.leave).toHaveBeenCalledWith(GROUP))
  })

  it('shows the personal ToBot path for a no-org session and not a team-inbox failure', async () => {
    mount({ org: 'personal' })
    const row = globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)
    expect(row).toHaveTextContent('messages.file_transfer')
    await screen.findByText('messages.thread_start_transfer')
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeInTheDocument()
    expect(globalThis.document.querySelector('[data-ui="messages.state"]')).toBeNull()
    expect(screen.queryByText('messages.state.error_title')).toBeNull()
    expect(api.conversations).not.toHaveBeenCalled()
  })

  it('shows the reconnecting banner and not the error boundary when the stream closes', async () => {
    const stream: { current: StreamHandlers | null } = { current: null }
    mount({ stream })
    await openDirect()
    expect(globalThis.document.querySelector('[data-ui="messages.offline"]')).toHaveTextContent('messages.reconnecting')
    expect(globalThis.document.querySelector('[data-status="error"]')).toBeNull()
    act(() => stream.current?.onOpen())
    await waitFor(() => expect(globalThis.document.querySelector('[data-ui="messages.offline"]')).toBeNull())
    act(() => stream.current?.onClose())
    expect(globalThis.document.querySelector('[data-ui="messages.offline"]')).toHaveTextContent('messages.reconnecting')
    expect(globalThis.document.querySelector('[data-ui="messages.state"]')).toBeNull()
    expect(screen.queryByText('messages.state.error_title')).toBeNull()
    expect(screen.getByRole('textbox', { name: 'messages.compose_placeholder' })).toBeEnabled()
  })

  it('submits a denial and clears the card when ToBot tool approval is denied', async () => {
    const request = vi.fn(async (route: string) => {
      if (route === 'ai.stream.attach') return { ok: true, data: { status: 'not-found' } }
      if (route === 'ai.tool.respond_approval') return { ok: true, data: { ok: true } }
      return { ok: true, data: undefined }
    })
    const listeners = new Map<string, (payload: unknown) => void>()
    vi.stubGlobal('api', {
      ipcApi: {
        request,
        on: (event: string, callback: (payload: unknown) => void) => {
          listeners.set(event, callback)
          return () => listeners.delete(event)
        }
      }
    })
    vi.spyOn(dataApiService, 'get').mockImplementation(async (path) => {
      if (!String(path).endsWith('/messages')) return {} as never
      return {
        items: [
          { id: 'a1', role: 'assistant', status: 'pending', searchableText: '', data: { parts: [] } },
          {
            id: 'u1',
            role: 'user',
            status: 'success',
            searchableText: '看这张图',
            data: { parts: [{ type: 'text', text: '看这张图' }] }
          }
        ]
      } as never
    })
    mount({ org: 'personal' })
    fireEvent.click(globalThis.document.querySelector(`[data-conversation-id="${BUILTIN_TOBOT_ID}"]`)!)
    await screen.findByRole('status')
    act(() => {
      listeners.get('ai.stream.chunk')?.({
        topicId: 'agent-session:tobot',
        anchorMessageId: 'a1',
        chunk: { type: 'tool-input-available', toolCallId: 'call-1', toolName: 'bash' }
      })
      listeners.get('ai.stream.chunk')?.({
        topicId: 'agent-session:tobot',
        anchorMessageId: 'a1',
        chunk: { type: 'tool-approval-request', approvalId: 'approval-1', toolCallId: 'call-1' }
      })
    })
    await screen.findByText('messages.tobot.approval')
    fireEvent.click(screen.getByRole('button', { name: 'messages.tobot.approval_deny' }))
    await waitFor(() =>
      expect(request).toHaveBeenCalledWith(
        'ai.tool.respond_approval',
        expect.objectContaining({ approvalId: 'approval-1', approved: false, topicId: 'agent-session:tobot' })
      )
    )
    expect(screen.queryByText('messages.tobot.approval')).toBeNull()
  })

  it('saves a ToBoss name and personality and offers no face controls', async () => {
    memberOrgsApi.listMemberOrgs.mockResolvedValue([{ id: ORG, name: '团队', logo: '', creator: true }])
    mount()
    fireEvent.click(await screen.findByText('messages.toboss', { selector: '[data-ui="messages.conversation-title"]' }))
    fireEvent.contextMenu(screen.getByRole('heading', { level: 1 }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).queryByRole('radiogroup', { name: 'messages.tobot.shape' })).toBeNull()
    expect(within(dialog).queryByRole('radiogroup', { name: 'messages.tobot.expression' })).toBeNull()
    expect(within(dialog).queryByRole('radiogroup', { name: 'messages.tobot.color' })).toBeNull()
    expect(within(dialog).queryByRole('radiogroup', { name: 'messages.tobot.eyes' })).toBeNull()
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'messages.tobot.name' }), {
      target: { value: '团团' }
    })
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'messages.tobot.personality' }), {
      target: { value: '说话短' }
    })
    fireEvent.click(within(dialog).getByRole('button', { name: 'messages.tobot.save' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    const stored = JSON.parse(globalThis.localStorage.getItem(TOBOSS_PROFILE_KEY) ?? '{}') as {
      name?: string
      personality?: string
    }
    expect(stored.name).toBe('团团')
    expect(stored.personality).toBe('说话短')
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('团团')
  })

  it('shows the soon, full, and queued ToBoss notices', async () => {
    await openBossGroup()
    askBoss('first ask')
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(1))
    askBoss('second ask')
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(2))
    expect(screen.getByText('messages.toboss.soon')).toBeInTheDocument()

    cleanup()
    __resetSharedImStore()
    resetBossRuntimeForTests()
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date('2026-10-13T09:00:00.000Z'))
    try {
      await openBossGroup()
      askBoss('queued first')
      await waitFor(() => expect(api.send).toHaveBeenCalledTimes(1))
      vi.setSystemTime(Date.now() + BOSS_GAP_MS)
      askBoss('queued second')
      await waitFor(() => expect(api.send).toHaveBeenCalledTimes(2))
      expect(screen.getByText('messages.toboss.queued')).toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }

    cleanup()
    __resetSharedImStore()
    resetBossRuntimeForTests()
    const occupy = (senderId: string, ask: string) => {
      const job: BossJob = {
        key: `${GROUP}:${senderId}:${ask}`,
        conversationId: GROUP,
        senderId,
        ask,
        prompt: ask
      }
      return offerSharedBossJob(job, Date.now()).decision
    }
    expect(occupy('runner', 'held turn')).toBe('start')
    for (let index = 0; index < BOSS_QUEUE_CAP; index += 1) {
      expect(occupy(`waiter-${index}`, `wait ${index}`)).toBe('queued')
    }
    await openBossGroup()
    askBoss('one more')
    await waitFor(() => expect(api.send).toHaveBeenCalledTimes(1))
    expect(screen.getByText('messages.toboss.full')).toBeInTheDocument()
  })

  it('selects the conversation a notification open request names', async () => {
    mount({ configure: (fake) => withDirectAndGroup(fake) })
    await openGroup()
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('项目组')
    act(() => requestOpenMessage(DIRECT))
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('周予安'))
    expect(screen.getByText('hello 1', { selector: '[data-ui="messages.bubble"]' })).toBeInTheDocument()
  })
})
