import { describe, expect, it, vi } from 'vitest'

import { ImApiError } from '../imApi'
import { BUILTIN_TOBOSS_ID, BUILTIN_TOBOT_ID, ImActionError, ImStore } from '../imStore'
import type { StreamHandlers } from '../imStream'
import { DIRECT, entry, fakeApi, GROUP, ME, message, ORG, PEER, THIRD, TRANSFER } from './fakeImApi'

function setup(overrides: { visible?: boolean } = {}) {
  const api = fakeApi()
  let handlers: StreamHandlers | null = null
  const stopStream = vi.fn()
  const store = new ImStore({
    createApi: () => api,
    runtimeAvailable: () => true,
    openStream: (_api, _device, next) => {
      handlers = next
      return stopStream
    },
    deviceId: () => 'studio-test',
    isVisible: () => overrides.visible ?? true,
    createObjectUrl: () => 'blob:local',
    revokeObjectUrl: vi.fn(),
    syncDelayMs: 0
  })
  const ready = async () => {
    store.start(ORG)
    await vi.waitFor(() => expect(store.getSnapshot().status).toBe('ready'))
  }
  const emit = (event: string, data: Record<string, unknown>) =>
    handlers!.onFrame({ event, data: JSON.stringify(data) })
  return { api, store, ready, emit, stopStream, handlers: () => handlers }
}

describe('ImStore', () => {
  it('stays empty without a runtime, and keeps ToBot when no team is selected', async () => {
    const offline = new ImStore({ runtimeAvailable: () => false })
    offline.start(ORG)
    expect(offline.getSnapshot()).toMatchObject({ status: 'unavailable', conversations: [] })

    const { store, api } = setup()
    store.start('personal')
    expect(store.getSnapshot()).toMatchObject({ status: 'no-org' })
    expect(store.getSnapshot().conversations).toEqual([
      expect.objectContaining({ id: BUILTIN_TOBOT_ID, kind: 'transfer', pinned: false, updatedAt: 0 })
    ])
    expect(api.conversations).not.toHaveBeenCalled()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(await store.open(BUILTIN_TOBOT_ID)).toBe(BUILTIN_TOBOT_ID)
    expect(store.getSnapshot().threads[BUILTIN_TOBOT_ID]?.loaded).toBe(true)
    await store.leave(BUILTIN_TOBOT_ID)
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([BUILTIN_TOBOT_ID])
  })

  it('loads identity, inbox and directory, then opens the stream', async () => {
    const { store, ready, handlers, api } = setup()
    await ready()
    const snapshot = store.getSnapshot()
    expect(snapshot.me).toEqual({ userId: ME, displayName: '林知夏' })
    expect(snapshot.conversations.map((item) => item.id)).toEqual([DIRECT])
    expect(snapshot.conversations.some((item) => item.kind === 'transfer' || item.id === BUILTIN_TOBOT_ID)).toBe(false)
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(snapshot.people[PEER].displayName).toBe('周予安')
    expect(handlers()).not.toBeNull()
  })

  it('reloads new colleagues without resetting the inbox', async () => {
    const { store, ready, api } = setup()
    await ready()
    const calls = api.directory.mock.calls.length
    api.directory.mockResolvedValueOnce([
      { userId: ME, displayName: '林知夏', email: 'me@example.com', departmentId: 'd-1', position: null },
      { userId: PEER, displayName: '周予安', email: 'peer@example.com', departmentId: 'd-2', position: '设计' },
      { userId: THIRD, displayName: '陈可', email: 'third@example.com', departmentId: 'd-2', position: null },
      { userId: 'new-member', displayName: '新同事', email: '', departmentId: 'd-1', position: null }
    ])
    store.refreshDirectory()
    await vi.waitFor(() => expect(store.getSnapshot().people['new-member']?.displayName).toBe('新同事'))
    expect(api.directory.mock.calls.length).toBe(calls + 1)
    expect(store.getSnapshot().status).toBe('ready')
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([DIRECT])
  })

  it('reports load failures and retries', async () => {
    const { store, api } = setup()
    api.me.mockRejectedValueOnce(new ImApiError('not a member', 403))
    store.start(ORG)
    await vi.waitFor(() => expect(store.getSnapshot().status).toBe('error'))
    expect(store.getSnapshot().error).toBe('403: not a member')
    store.retry()
    await vi.waitFor(() => expect(store.getSnapshot().status).toBe('ready'))
  })

  it('asks to sign in again when the member session is denied', async () => {
    const { store, api } = setup()
    api.me.mockRejectedValueOnce(new ImApiError('member session denied', 401))
    store.start(ORG)
    await vi.waitFor(() => expect(store.getSnapshot().status).toBe('signed-out'))
    expect(store.getSnapshot()).toMatchObject({ error: null, conversations: [] })
  })

  it('loads a conversation without opening it or moving the read cursor', async () => {
    const { store, ready, api } = setup()
    await ready()
    const unread = store.getSnapshot().conversations[0]?.unread
    await store.preload(DIRECT)
    expect(store.getSnapshot().threads[DIRECT]?.loaded).toBe(true)
    expect(store.getSnapshot().threads[DIRECT]?.messages.map((item) => item.sequence)).toEqual([1, 2])
    expect(api.markRead).not.toHaveBeenCalled()
    expect(api.detail).not.toHaveBeenCalled()
    expect(store.getSnapshot().conversations[0]?.unread).toBe(unread)
    const calls = api.messages.mock.calls.length
    await store.preload(DIRECT)
    expect(api.messages.mock.calls.length).toBe(calls)
  })

  it('opens a conversation in sequence order and reads it', async () => {
    const { store, ready, api } = setup()
    await ready()
    await store.open(DIRECT)
    const snapshot = store.getSnapshot()
    expect(snapshot.threads[DIRECT].messages.map((item) => item.sequence)).toEqual([1, 2])
    expect(snapshot.details[DIRECT].members).toHaveLength(2)
    expect(api.markRead).toHaveBeenCalledWith(DIRECT)
    expect(snapshot.conversations[0]).toMatchObject({ lastReadSequence: 2, unread: 0 })
  })

  it('does not mark read while the window is hidden', async () => {
    const { store, ready, api } = setup({ visible: false })
    await ready()
    await store.open(DIRECT)
    expect(api.markRead).not.toHaveBeenCalled()
  })

  it('sends optimistically and retries with the same client id', async () => {
    const { store, ready, api } = setup()
    await ready()
    await store.open(DIRECT)
    api.send.mockRejectedValueOnce(new ImApiError('network', 0))
    await expect(store.sendText(DIRECT, '  hi  ')).rejects.toBeInstanceOf(ImActionError)
    const [failed] = store.getSnapshot().threads[DIRECT].pending
    expect(failed).toMatchObject({ text: 'hi', state: 'failed' })

    await store.retrySend(DIRECT, failed.clientMsgId)
    expect(api.send.mock.calls[0][1]).toBe(api.send.mock.calls[1][1])
    const thread = store.getSnapshot().threads[DIRECT]
    expect(thread.pending).toEqual([])
    expect(thread.messages.at(-1)).toMatchObject({ sequence: 3, sender: { id: ME } })
    expect(store.getSnapshot().conversations[0]).toMatchObject({ lastSequence: 3, lastReadSequence: 3 })
  })

  it('uploads a file before sending it and rejects oversized ones', async () => {
    const { store, ready, api } = setup()
    await ready()
    await store.open(DIRECT)
    const file = new File(['abc'], 'a.png', { type: 'image/png' })
    await store.sendFile(DIRECT, file)
    expect(api.upload).toHaveBeenCalledWith(file, 'a.png')
    expect(api.send.mock.calls[0][2]).toEqual({
      kind: 'image',
      attachment: { name: 'a.png', sizeBytes: 3, objectKey: `im/${ORG}/${ME}/abc`, mime: 'image/png' }
    })

    const huge = new File(['x'], 'big.bin')
    Object.defineProperty(huge, 'size', { value: 21 * 1024 * 1024 })
    await expect(store.sendFile(DIRECT, huge)).rejects.toMatchObject({ code: 'too_large' })
    expect(api.upload).toHaveBeenCalledTimes(1)

    const voice = new File(['abc'], 'note.m4a', { type: 'audio/mp4' })
    await store.sendFile(DIRECT, voice)
    expect(api.send.mock.calls.at(-1)?.[2]).toMatchObject({ kind: 'voice' })
  })

  it('shows another member typing without writing a message, and hides presence when the cache is down', async () => {
    const { store, ready, api, emit } = setup()
    api.renewPresence.mockResolvedValueOnce(false)
    await ready()
    await vi.waitFor(() => expect(api.renewPresence).toHaveBeenCalled())
    expect(store.getSnapshot().presenceKnown).toBe(false)
    expect(store.getSnapshot().online).toEqual([])
    expect(api.sync).not.toHaveBeenCalled()

    emit('im.ephemeral.typing', { conversation_id: DIRECT, user_id: PEER, ttl_secs: 5 })
    expect(store.getSnapshot().typing[DIRECT]).toMatchObject({ userId: PEER })
    expect(api.sync).not.toHaveBeenCalled()
    emit('im.ephemeral.typing', { conversation_id: DIRECT, user_id: ME, ttl_secs: 5 })
    expect(store.getSnapshot().typing[DIRECT]?.userId).toBe(PEER)
  })

  it('pulls new messages when the stream says a conversation changed', async () => {
    const { store, ready, api, emit } = setup()
    await ready()
    await store.open(DIRECT)
    api.sync.mockResolvedValueOnce([
      {
        conversationId: DIRECT,
        entry: entry({ lastSequence: 3, lastReadSequence: 2, unread: 1 }),
        messages: [message(3)]
      }
    ])
    api.markRead.mockResolvedValueOnce(3)
    emit('im.facts.message.created', { conversation_id: DIRECT, message_id: 'm-3', sequence: 3 })
    await vi.waitFor(() => expect(store.getSnapshot().threads[DIRECT].messages).toHaveLength(3))
    expect(api.sync).toHaveBeenCalledWith([{ conversationId: DIRECT, sequence: 2 }])
    await vi.waitFor(() => expect(store.getSnapshot().conversations[0].unread).toBe(0))
  })

  it('refreshes the list for a conversation it has not seen', async () => {
    const { ready, api, emit } = setup()
    await ready()
    emit('im.facts.message.created', { conversation_id: 'new-conversation', sequence: 1 })
    await vi.waitFor(() => expect(api.conversations).toHaveBeenCalledTimes(2))
  })

  it('applies read receipts and recalls from the stream', async () => {
    const { store, ready, emit } = setup()
    await ready()
    await store.open(DIRECT)
    emit('im.facts.cursor.changed', { conversation_id: DIRECT, user_id: PEER, sequence: 9 })
    expect(store.getSnapshot().details[DIRECT].members.find((member) => member.userId === PEER)?.lastReadSequence).toBe(
      9
    )
    emit('im.facts.message.recalled', { conversation_id: DIRECT, message_id: `m-${DIRECT}-1` })
    expect(store.getSnapshot().threads[DIRECT].messages[0]).toMatchObject({
      recalled: true,
      body: { kind: 'recalled', original: 'text' }
    })
  })

  it('keeps a dismissed message out of this member view', async () => {
    localStorage.removeItem('messages.dismissed.v1')
    try {
      const { store, ready, api, emit } = setup()
      await ready()
      await store.open(DIRECT)
      store.dismiss(DIRECT, `m-${DIRECT}-2`)
      expect(store.getSnapshot().threads[DIRECT].messages.map((item) => item.id)).toEqual([`m-${DIRECT}-1`])
      expect(store.getSnapshot().conversations.find((item) => item.id === DIRECT)?.preview?.text).toBe('hello 1')
      expect(api.recall).not.toHaveBeenCalled()

      api.sync.mockResolvedValueOnce([
        {
          conversationId: DIRECT,
          entry: entry(),
          messages: [message(2)]
        }
      ])
      emit('im.facts.message.created', { conversation_id: DIRECT, message_id: `m-${DIRECT}-2`, sequence: 2 })
      await vi.waitFor(() =>
        expect(store.getSnapshot().conversations.find((item) => item.id === DIRECT)?.preview?.text).toBe('hello 1')
      )
      expect(store.getSnapshot().threads[DIRECT].messages.map((item) => item.id)).toEqual([`m-${DIRECT}-1`])

      const next = setup()
      await next.ready()
      await next.store.open(DIRECT)
      expect(next.store.getSnapshot().threads[DIRECT].messages.map((item) => item.id)).toEqual([`m-${DIRECT}-1`])
      expect(next.store.getSnapshot().conversations.find((item) => item.id === DIRECT)?.preview?.text).toBe('hello 1')
    } finally {
      localStorage.removeItem('messages.dismissed.v1')
    }
  })

  it('loads older pages before the oldest sequence', async () => {
    const { store, ready, api } = setup()
    api.messages.mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(100 - index)))
    await ready()
    await store.open(DIRECT)
    api.messages.mockResolvedValueOnce([message(50), message(49)])
    expect(await store.loadOlder(DIRECT)).toBe(2)
    expect(api.messages).toHaveBeenLastCalledWith(DIRECT, 51)
    const thread = store.getSnapshot().threads[DIRECT]
    expect(thread.messages[0].sequence).toBe(49)
    expect(thread.hasOlder).toBe(false)
  })

  it('resets everything when the team changes', async () => {
    const { store, ready, stopStream } = setup()
    await ready()
    await store.open(DIRECT)
    store.start('22222222-2222-4222-8222-222222222222')
    expect(stopStream).toHaveBeenCalled()
    expect(store.getSnapshot()).toMatchObject({ status: 'loading', conversations: [], threads: {} })
  })

  it('puts the conversation pinned last at the front of the pinned rows', async () => {
    localStorage.removeItem('messages.pin-order.v1')
    const { store, ready, api } = setup()
    const quiet = entry({ id: DIRECT, updatedAt: 1_000, pinned: false })
    const active = entry({
      id: GROUP,
      kind: 'group',
      title: '近期群',
      peerUserId: null,
      memberCount: 3,
      preview: null,
      updatedAt: 9_000,
      pinned: true
    })
    api.conversations.mockResolvedValue([active, quiet])
    await ready()
    await store.setPinned(DIRECT, true)
    expect(
      store
        .getSnapshot()
        .conversations.filter((item) => item.pinned)
        .map((item) => item.id)[0]
    ).toBe(DIRECT)

    api.conversations.mockResolvedValue([
      entry({ ...active, updatedAt: 50_000 }),
      entry({ ...quiet, pinned: true, updatedAt: 1_000 })
    ])
    await store.startDirect(PEER)
    expect(
      store
        .getSnapshot()
        .conversations.filter((item) => item.pinned)
        .map((item) => item.id)[0]
    ).toBe(DIRECT)
  })

  it('updates settings after the server accepts them', async () => {
    const { store, ready, api } = setup()
    await ready()
    await store.setPinned(DIRECT, true)
    await store.setMuted(DIRECT, true)
    await store.markUnread(DIRECT)
    expect(api.setPinned).toHaveBeenCalledWith(DIRECT, true)
    expect(store.getSnapshot().conversations[0]).toMatchObject({
      pinned: true,
      muted: true,
      lastReadSequence: 1,
      unread: 1
    })
    await store.leave(DIRECT)
    expect(api.leave).toHaveBeenCalledTimes(1)
    expect(store.getSnapshot().conversations).toEqual([])
  })

  it('drops a server file-transfer row from the team inbox', async () => {
    const { store, ready, api } = setup()
    await ready()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    api.conversations.mockResolvedValue([
      entry(),
      entry({
        id: TRANSFER,
        kind: 'transfer',
        peerUserId: null,
        unread: 2,
        lastSequence: 2,
        updatedAt: 9_000
      })
    ])
    await store.startDirect(PEER)
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([DIRECT])
    expect(
      store.getSnapshot().conversations.some((item) => item.kind === 'transfer' || item.id === BUILTIN_TOBOT_ID)
    ).toBe(false)
  })

  it('does not put the personal ToBot back when a transfer reply is noted', async () => {
    const { store, ready, api } = setup()
    await ready()
    store.noteTransferUnread(TRANSFER)
    store.noteTransferUnread(DIRECT)
    expect(api.markUnread).not.toHaveBeenCalled()
    expect(store.getSnapshot().conversations.some((item) => item.id === TRANSFER)).toBe(false)
    expect(store.getSnapshot().conversations.find((item) => item.id === DIRECT)?.unread).toBe(1)

    api.conversations.mockResolvedValue([
      entry(),
      entry({
        id: TRANSFER,
        kind: 'transfer',
        peerUserId: null,
        memberCount: 1,
        unread: 1,
        lastSequence: 4,
        lastReadSequence: 4,
        preview: null
      })
    ])
    await store.startDirect(PEER)
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([DIRECT])
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(await store.open(BUILTIN_TOBOT_ID)).toBeNull()
    expect(api.messages).not.toHaveBeenCalled()
  })

  it('does not open a personal ToBot inside a team', async () => {
    const { store, ready, api } = setup()
    api.fileTransfer.mockRejectedValue(new ImApiError('down', 503))
    await ready()
    expect(store.getSnapshot()).toMatchObject({ status: 'ready' })
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([DIRECT])
    expect(await store.open(BUILTIN_TOBOT_ID)).toBeNull()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(api.messages).not.toHaveBeenCalled()

    api.fileTransfer.mockResolvedValue(TRANSFER)
    expect(await store.open(BUILTIN_TOBOT_ID)).toBeNull()
    expect(api.fileTransfer).not.toHaveBeenCalled()
    expect(store.getSnapshot().conversations.some((item) => item.id === BUILTIN_TOBOT_ID || item.id === TRANSFER)).toBe(
      false
    )
  })

  it('shows ToBoss only for the team creator and keeps its pin and mute', async () => {
    const { store, api } = setup()
    store.start(ORG)
    expect(store.getSnapshot().status).toBe('loading')
    store.setTeamCreator(true)
    expect(store.getSnapshot().conversations.some((item) => item.id === BUILTIN_TOBOSS_ID)).toBe(false)
    await vi.waitFor(() => expect(store.getSnapshot().status).toBe('ready'))
    expect(store.getSnapshot().conversations.find((item) => item.id === BUILTIN_TOBOSS_ID)).toMatchObject({
      kind: 'boss'
    })
    expect(
      store.getSnapshot().conversations.some((item) => item.kind === 'transfer' || item.id === BUILTIN_TOBOT_ID)
    ).toBe(false)
    expect(api.fileTransfer).not.toHaveBeenCalled()

    await store.setPinned(BUILTIN_TOBOSS_ID, true)
    await store.setMuted(BUILTIN_TOBOSS_ID, true)
    expect(api.setPinned).not.toHaveBeenCalled()
    expect(api.setMuted).not.toHaveBeenCalled()
    api.conversations.mockResolvedValue([entry()])
    await store.startDirect(PEER)
    expect(store.getSnapshot().conversations.find((item) => item.id === BUILTIN_TOBOSS_ID)).toMatchObject({
      kind: 'boss',
      pinned: true,
      muted: true
    })

    const messageCalls = api.messages.mock.calls.length
    expect(await store.open(BUILTIN_TOBOSS_ID)).toBe(BUILTIN_TOBOSS_ID)
    expect(api.messages.mock.calls.length).toBe(messageCalls)
    expect(store.getSnapshot().threads[BUILTIN_TOBOSS_ID]?.loaded).toBe(true)
    await store.leave(BUILTIN_TOBOSS_ID)
    expect(api.leave).not.toHaveBeenCalled()
    expect(store.getSnapshot().conversations.some((item) => item.id === BUILTIN_TOBOSS_ID)).toBe(true)

    store.setTeamCreator(false)
    expect(store.getSnapshot().conversations.some((item) => item.id === BUILTIN_TOBOSS_ID)).toBe(false)

    store.start('personal')
    store.setTeamCreator(true)
    expect(store.getSnapshot().conversations.map((item) => item.id)).toEqual([BUILTIN_TOBOT_ID])
  })
})
