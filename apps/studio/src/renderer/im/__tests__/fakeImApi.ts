import { vi } from 'vitest'

import type { ImApi, ImConversationDetail, ImInboxEntry, ImMessage } from '../imApi'

export const ORG = '11111111-1111-4111-8111-111111111111'
export const ME = 'aaaaaaaa-0000-4000-8000-000000000001'
export const PEER = 'aaaaaaaa-0000-4000-8000-000000000002'
export const THIRD = 'aaaaaaaa-0000-4000-8000-000000000003'
export const DIRECT = 'cccccccc-0000-4000-8000-000000000001'
export const GROUP = 'cccccccc-0000-4000-8000-000000000002'
export const TRANSFER = 'cccccccc-0000-4000-8000-000000000003'

export function message(sequence: number, overrides: Partial<ImMessage> = {}): ImMessage {
  return {
    id: `m-${overrides.conversationId ?? DIRECT}-${sequence}`,
    conversationId: DIRECT,
    sequence,
    sender: { kind: 'member', id: PEER },
    body: { kind: 'text', text: `hello ${sequence}` },
    recalled: false,
    createdAt: Date.parse('2026-10-02T08:00:00Z') + sequence * 1000,
    ...overrides
  }
}

export function entry(overrides: Partial<ImInboxEntry> = {}): ImInboxEntry {
  return {
    id: DIRECT,
    kind: 'direct',
    title: null,
    peerUserId: PEER,
    memberCount: 2,
    lastSequence: 2,
    lastReadSequence: 1,
    unread: 1,
    preview: {
      kind: 'text',
      messageId: `m-${DIRECT}-2`,
      sequence: 2,
      recalled: false,
      createdAt: Date.parse('2026-10-02T08:00:02Z'),
      sender: { kind: 'member', id: PEER },
      text: 'hello 2',
      name: null,
      code: null,
      params: {}
    },
    muted: false,
    pinned: false,
    updatedAt: Date.parse('2026-10-02T08:00:02Z'),
    ...overrides
  }
}

export function detail(overrides: Partial<ImConversationDetail> = {}): ImConversationDetail {
  return {
    id: DIRECT,
    kind: 'direct',
    title: null,
    memberCount: 2,
    members: [
      { userId: ME, role: 'member', left: false, lastReadSequence: 1 },
      { userId: PEER, role: 'member', left: false, lastReadSequence: 2 }
    ],
    ...overrides
  }
}

export type FakeApi = {
  [K in keyof ImApi]: ImApi[K] extends (...args: infer A) => infer R
    ? ReturnType<typeof vi.fn<(...args: A) => R>>
    : ImApi[K]
}

export function fakeApi(): FakeApi {
  return {
    orgId: ORG,
    me: vi.fn(async () => ({ userId: ME, displayName: '林知夏' })),
    directory: vi.fn(async () => [
      { userId: ME, displayName: '林知夏', email: 'me@example.com', departmentId: 'd-1', position: null },
      { userId: PEER, displayName: '周予安', email: 'peer@example.com', departmentId: 'd-2', position: '设计' },
      { userId: THIRD, displayName: '陈可', email: 'third@example.com', departmentId: 'd-2', position: null }
    ]),
    departments: vi.fn(async () => [
      { id: 'd-1', parentId: null, name: '总部', sortOrder: 0 },
      { id: 'd-2', parentId: 'd-1', name: '设计部', sortOrder: 0 }
    ]),
    conversations: vi.fn(async () => [entry()]),
    detail: vi.fn(async () => detail()),
    messages: vi.fn(async () => [message(2), message(1)]),
    send: vi.fn(async (conversationId, _clientMsgId, body) =>
      message(3, {
        conversationId,
        sender: { kind: 'member', id: ME },
        // A message the member just sent is inside the 5-minute recall window.
        createdAt: Date.now(),
        body:
          body.kind === 'text'
            ? body
            : body.kind === 'voice'
              ? { kind: 'voice', attachment: body.attachment, durationSecs: 0 }
              : { kind: body.kind, attachment: body.attachment }
      })
    ),
    recall: vi.fn(async (messageId) => ({
      ...message(3, { sender: { kind: 'member', id: ME } }),
      id: messageId,
      recalled: true,
      body: { kind: 'recalled', original: 'text' }
    })),
    markRead: vi.fn(async () => 2),
    markUnread: vi.fn(async () => 1),
    setMuted: vi.fn(async () => undefined),
    setPinned: vi.fn(async () => undefined),
    startDirect: vi.fn(async () => DIRECT),
    fileTransfer: vi.fn(async () => TRANSFER),
    createGroup: vi.fn(async () => GROUP),
    addMembers: vi.fn(async () => undefined),
    removeMember: vi.fn(async () => undefined),
    leave: vi.fn(async () => undefined),
    dissolve: vi.fn(async () => undefined),
    rename: vi.fn(async () => undefined),
    setRemark: vi.fn(async () => undefined),
    setNickname: vi.fn(async () => undefined),
    setMuteAll: vi.fn(async () => undefined),
    setMemberSpeech: vi.fn(async () => undefined),
    transferOwner: vi.fn(async () => undefined),
    setMemberRole: vi.fn(async () => undefined),
    typing: vi.fn(async () => undefined),
    renewPresence: vi.fn(async () => true),
    presence: vi.fn(async () => []),
    sync: vi.fn(async () => []),
    upload: vi.fn(async (file: Blob, name: string) => ({
      name,
      sizeBytes: file.size,
      objectKey: `im/${ORG}/${ME}/abc`,
      mime: file.type || null
    })),
    download: vi.fn(async () => new Blob(['x'])),
    stream: vi.fn(async () => new Response(''))
  } as unknown as FakeApi
}
