/**
 * IM state for the Messages page. REST snapshots ordered by sequence are the truth;
 * stream events only say "something changed here, go fetch it".
 */
import {
  createImApi,
  IM_ATTACHMENT_MAX_BYTES,
  IM_PAGE_LIMIT,
  type ImApi,
  ImApiError,
  type ImAttachment,
  type ImConversationDetail,
  type ImDepartment,
  type ImInboxEntry,
  type ImMe,
  type ImMessage,
  type ImOutgoing,
  type ImPerson,
  imRuntimeAvailable
} from './imApi'
import { keepStreamOpen, type SseFrame } from './imStream'
import { decodeBossWire, encodeBossWire, visibleText } from './quote'

export type ImStatus = 'idle' | 'unavailable' | 'signed-out' | 'no-org' | 'loading' | 'ready' | 'error'

export type PendingMessage = {
  clientMsgId: string
  conversationId: string
  kind: 'text' | 'file' | 'image' | 'voice'
  text: string
  name: string
  sizeBytes: number
  /** Object URL of a local image while it uploads. */
  localUrl: string | null
  createdAt: number
  state: 'sending' | 'failed'
}

export type ThreadState = {
  messages: ImMessage[]
  pending: PendingMessage[]
  loaded: boolean
  hasOlder: boolean
}

export type ImSnapshot = {
  status: ImStatus
  error: string | null
  orgId: string | null
  connected: boolean
  me: ImMe | null
  conversations: ImInboxEntry[]
  threads: Record<string, ThreadState>
  details: Record<string, ImConversationDetail>
  people: Record<string, ImPerson>
  directory: ImPerson[]
  departments: ImDepartment[]
  /** Active typing hints. Ephemeral; a missing entry means nobody else is typing. */
  typing: Record<string, { userId: string; until: number }>
  /** True only after a presence read succeeded. Otherwise the UI hides every dot. */
  presenceKnown: boolean
  online: string[]
}

export class ImActionError extends Error {
  readonly code: 'too_large' | 'empty' | 'request'

  constructor(code: 'too_large' | 'empty' | 'request', message: string = code) {
    super(message)
    this.name = 'ImActionError'
    this.code = code
  }
}

const EMPTY_THREAD: ThreadState = { messages: [], pending: [], loaded: false, hasOlder: false }

const INITIAL: ImSnapshot = {
  status: 'idle',
  error: null,
  orgId: null,
  connected: false,
  me: null,
  conversations: [],
  threads: {},
  details: {},
  people: {},
  directory: [],
  departments: [],
  typing: {},
  presenceKnown: false,
  online: []
}

export type ImStoreDeps = {
  createApi?: (orgId: string) => ImApi
  runtimeAvailable?: () => boolean
  openStream?: typeof keepStreamOpen
  deviceId?: () => string
  isVisible?: () => boolean
  createObjectUrl?: (blob: Blob) => string
  revokeObjectUrl?: (url: string) => void
  /** Delay before batched event syncs run. */
  syncDelayMs?: number
}

const PERSONAL_CENTER = 'personal'

/** Inbox id used until `PUT /file-transfer` returns the server conversation. */
export const BUILTIN_TOBOT_ID = 'tobot'
/** Client-only row for the team creator. The server never stores this id. */
export const BUILTIN_TOBOSS_ID = 'toboss'

export class ImStore {
  private snapshot: ImSnapshot = INITIAL
  private readonly listeners = new Set<() => void>()
  private api: ImApi | null = null
  private stopStream: (() => void) | null = null
  private generation = 0
  private activeId: string | null = null
  private syncQueue = new Set<string>()
  private syncTimer: ReturnType<typeof setTimeout> | null = null
  private listRefresh: Promise<void> | null = null
  private fileTransferId: string | null = null
  private attachmentUrls = new Map<string, Promise<string>>()
  /** In-flight quiet loads. Opening the chat does not wait on this map. */
  private preloads = new Map<string, Promise<void>>()
  private uploads = new Map<string, ImAttachment>()
  private files = new Map<string, File>()
  private typingTimers = new Map<string, ReturnType<typeof setTimeout>>()
  private presenceTimer: ReturnType<typeof setInterval> | null = null
  /** Conversations marked unread stay unread until they are opened again. */
  private holdUnread = new Set<string>()
  /** Local ToBot unread. A list refresh keeps it, and it does not rewind the server cursor. */
  private transferAttention = new Set<string>()
  /** True only when the signed-in member created the open organization. */
  private teamCreator = false
  /** Pin order for this member. A higher value was pinned later and leads the pinned rows. */
  private pinnedAt = readPinOrder()
  private pinClock = Math.max(0, ...this.pinnedAt.values())
  /** Messages this member removed from their own view. Other members still have them. */
  private dismissed = readDismissed()
  /** Sends the member deleted before the server copy arrived. */
  private droppedSends = new Set<string>()
  private readonly deps: Required<ImStoreDeps>

  constructor(deps: ImStoreDeps = {}) {
    this.deps = {
      createApi: deps.createApi ?? ((orgId) => createImApi(orgId)),
      runtimeAvailable: deps.runtimeAvailable ?? imRuntimeAvailable,
      openStream: deps.openStream ?? keepStreamOpen,
      deviceId: deps.deviceId ?? studioDeviceId,
      isVisible: deps.isVisible ?? (() => typeof document === 'undefined' || document.visibilityState !== 'hidden'),
      createObjectUrl: deps.createObjectUrl ?? ((blob) => URL.createObjectURL(blob)),
      revokeObjectUrl: deps.revokeObjectUrl ?? ((url) => URL.revokeObjectURL(url)),
      syncDelayMs: deps.syncDelayMs ?? 60
    }
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  getSnapshot = () => this.snapshot

  // --- lifecycle ---------------------------------------------------------

  /** Points the store at an org. The same org is a no-op unless the last load failed. */
  start(orgId: string | null): void {
    const target = orgId && orgId !== PERSONAL_CENTER ? orgId : null
    if (this.snapshot.orgId === target && this.snapshot.status !== 'error' && this.snapshot.status !== 'idle') return
    this.teamCreator = false
    this.teardown()
    const generation = ++this.generation
    if (!this.deps.runtimeAvailable()) {
      this.set({ ...INITIAL, status: 'unavailable' })
      return
    }
    if (!target) {
      this.set({
        ...INITIAL,
        status: 'no-org',
        conversations: [emptyFileTransfer(BUILTIN_TOBOT_ID)]
      })
      return
    }
    const api = this.deps.createApi(target)
    this.api = api
    this.set({ ...INITIAL, status: 'loading', orgId: target })
    void (async () => {
      try {
        const [me, conversations, directory, departments] = await Promise.all([
          api.me(),
          api.conversations(),
          api.directory(),
          api.departments()
        ])
        if (generation !== this.generation) return
        this.publishConversations(conversations, {
          status: 'ready',
          me,
          directory,
          departments,
          people: Object.fromEntries(directory.map((person) => [person.userId, person]))
        })
        void this.refreshPresence()
        this.presenceTimer = setInterval(() => void this.refreshPresence(), 30_000)
        this.stopStream = this.deps.openStream(api, this.deps.deviceId(), {
          onOpen: () => {
            if (generation !== this.generation) return
            this.set({ ...this.snapshot, connected: true })
            void this.resyncAll()
          },
          onFrame: (frame) => {
            if (generation === this.generation) this.onFrame(frame)
          },
          onClose: () => {
            if (generation === this.generation) this.set({ ...this.snapshot, connected: false })
          }
        })
      } catch (error) {
        if (generation !== this.generation) return
        if (error instanceof ImApiError && error.status === 401) {
          this.set({ ...INITIAL, status: 'signed-out', orgId: target })
          return
        }
        this.set({ ...INITIAL, status: 'error', orgId: target, error: errorMessage(error) })
      }
    })()
  }

  retry(): void {
    const orgId = this.snapshot.orgId
    this.snapshot = { ...this.snapshot, status: 'error' }
    this.start(orgId)
  }

  /** Reloads colleagues and departments. The inbox and open threads stay. */
  refreshDirectory(): void {
    const api = this.api
    const generation = this.generation
    if (!api || this.snapshot.status !== 'ready') return
    void (async () => {
      try {
        const [me, directory, departments] = await Promise.all([api.me(), api.directory(), api.departments()])
        if (generation !== this.generation) return
        this.set({
          ...this.snapshot,
          me,
          directory,
          departments,
          people: Object.fromEntries(directory.map((person) => [person.userId, person]))
        })
      } catch {
        // The picker keeps the last directory when the refresh fails.
      }
    })()
  }

  stop(): void {
    this.teardown()
    this.generation += 1
    this.set(INITIAL)
  }

  private teardown(): void {
    this.stopStream?.()
    this.stopStream = null
    this.api = null
    this.activeId = null
    this.holdUnread.clear()
    this.transferAttention.clear()
    this.syncQueue.clear()
    if (this.syncTimer) clearTimeout(this.syncTimer)
    this.syncTimer = null
    this.listRefresh = null
    this.fileTransferId = null
    for (const url of this.attachmentUrls.values()) void url.then(this.deps.revokeObjectUrl, () => undefined)
    this.attachmentUrls.clear()
    this.preloads.clear()
    for (const thread of Object.values(this.snapshot.threads)) {
      for (const pending of thread.pending) if (pending.localUrl) this.deps.revokeObjectUrl(pending.localUrl)
    }
    this.uploads.clear()
    this.files.clear()
    for (const timer of this.typingTimers.values()) clearTimeout(timer)
    this.typingTimers.clear()
    if (this.presenceTimer) clearInterval(this.presenceTimer)
    this.presenceTimer = null
  }

  /**
   * Shows or hides the team ToBot (formerly ToBoss). An org switch clears this
   * before the member list returns, so the previous team's row does not stay.
   */
  setTeamCreator(on: boolean): void {
    if (this.teamCreator === on) return
    this.teamCreator = on
    if (!this.snapshot.orgId) return
    if (this.snapshot.status !== 'ready') return
    this.publishConversations(this.snapshot.conversations)
  }

  // --- reading -----------------------------------------------------------

  /**
   * Opens a conversation: first page, members with read cursors, and clears unread.
   * The personal ToBot id is local. A team inbox does not open it.
   */
  async open(conversationId: string | null): Promise<string | null> {
    if (conversationId) {
      this.holdUnread.delete(conversationId)
      this.clearTransferAttention(conversationId)
    }
    if (conversationId === BUILTIN_TOBOT_ID) {
      if (this.snapshot.orgId) return null
      const generation = this.generation
      const resolved = await this.ensureFileTransfer()
      if (generation !== this.generation) return null
      if (resolved) {
        const listed = this.snapshot.conversations.filter((entry) => entry.id !== BUILTIN_TOBOT_ID)
        this.publishConversations(listed)
        return this.open(resolved)
      }
      this.activeId = BUILTIN_TOBOT_ID
      this.patchThread(BUILTIN_TOBOT_ID, (current) => ({ ...current, loaded: true, hasOlder: false }))
      this.patchEntry(BUILTIN_TOBOT_ID, (entry) => (entry.unread === 0 ? entry : { ...entry, unread: 0 }))
      return BUILTIN_TOBOT_ID
    }
    if (conversationId === BUILTIN_TOBOSS_ID) {
      this.activeId = BUILTIN_TOBOSS_ID
      this.patchThread(BUILTIN_TOBOSS_ID, (current) => ({ ...current, loaded: true, hasOlder: false }))
      this.patchEntry(BUILTIN_TOBOSS_ID, (entry) => (entry.unread === 0 ? entry : { ...entry, unread: 0 }))
      return BUILTIN_TOBOSS_ID
    }
    this.activeId = conversationId
    if (!conversationId) return null
    const api = this.requireApi()
    const generation = this.generation
    const thread = this.thread(conversationId)
    const [messages, detail] = await Promise.all([
      thread.loaded ? Promise.resolve(null) : api.messages(conversationId),
      api.detail(conversationId)
    ])
    if (generation !== this.generation) return null
    if (messages) {
      this.patchThread(conversationId, (current) => ({
        ...current,
        messages: this.mergeKept(current.messages, messages),
        loaded: true,
        hasOlder: messages.length >= IM_PAGE_LIMIT
      }))
    }
    this.set({ ...this.snapshot, details: { ...this.snapshot.details, [conversationId]: detail } })
    if (messages) this.patchEntry(conversationId, (entry) => this.previewAfterDismiss(entry))
    await this.readIfActive(conversationId)
    return conversationId
  }

  /**
   * First page of a conversation, without opening it or moving the read cursor.
   * The group lane uses this so an @ is visible while another chat stays on screen.
   */
  preload(conversationId: string): Promise<void> {
    if (!conversationId || conversationId === BUILTIN_TOBOT_ID || conversationId === BUILTIN_TOBOSS_ID) {
      return Promise.resolve()
    }
    if (this.thread(conversationId).loaded) return Promise.resolve()
    const existing = this.preloads.get(conversationId)
    if (existing) return existing
    const job = this.loadQuiet(conversationId).finally(() => {
      if (this.preloads.get(conversationId) === job) this.preloads.delete(conversationId)
    })
    this.preloads.set(conversationId, job)
    return job
  }

  private async loadQuiet(conversationId: string): Promise<void> {
    const api = this.api
    if (!api) return
    const generation = this.generation
    let messages: ImMessage[]
    try {
      messages = await api.messages(conversationId)
    } catch {
      return
    }
    if (generation !== this.generation) return
    if (this.thread(conversationId).loaded) return
    this.patchThread(conversationId, (thread) => ({
      ...thread,
      messages: this.mergeKept(thread.messages, messages),
      loaded: true,
      hasOlder: messages.length >= IM_PAGE_LIMIT
    }))
    this.patchEntry(conversationId, (entry) => this.previewAfterDismiss(entry))
  }

  async loadOlder(conversationId: string): Promise<number> {
    const thread = this.thread(conversationId)
    const oldest = thread.messages[0]?.sequence
    if (!thread.loaded || !thread.hasOlder || !oldest || oldest <= 1) return 0
    const generation = this.generation
    const older = await this.requireApi().messages(conversationId, oldest)
    if (generation !== this.generation) return 0
    this.patchThread(conversationId, (current) => ({
      ...current,
      messages: this.mergeKept(current.messages, older),
      hasOlder: older.length >= IM_PAGE_LIMIT
    }))
    return older.length
  }

  attachmentUrl(messageId: string): Promise<string> {
    let url = this.attachmentUrls.get(messageId)
    if (!url) {
      url = this.requireApi()
        .download(messageId)
        .then((blob) => this.deps.createObjectUrl(blob))
      url.catch(() => this.attachmentUrls.delete(messageId))
      this.attachmentUrls.set(messageId, url)
    }
    return url
  }

  /** The local file for a message that has not finished sending. */
  stagedFile(clientMsgId: string): File | null {
    return this.files.get(clientMsgId) ?? null
  }

  /** Bytes of a stored attachment, for forwarding into another conversation. */
  async attachmentFile(messageId: string, name: string, mime: string | null): Promise<File> {
    const blob = await this.requireApi().download(messageId)
    return new File([blob], name || 'file', { type: mime || blob.type || 'application/octet-stream' })
  }

  // --- sending -----------------------------------------------------------

  sendText(conversationId: string, text: string): Promise<void> {
    const trimmed = text.trim()
    if (!trimmed) return Promise.reject(new ImActionError('empty'))
    const pending: PendingMessage = {
      clientMsgId: newClientMsgId(),
      conversationId,
      kind: 'text',
      text: trimmed,
      name: '',
      sizeBytes: 0,
      localUrl: null,
      createdAt: Date.now(),
      state: 'sending'
    }
    this.patchThread(conversationId, (thread) => ({ ...thread, pending: [...thread.pending, pending] }))
    return this.deliver(pending)
  }

  sendFile(conversationId: string, file: File): Promise<void> {
    if (file.size === 0) return Promise.reject(new ImActionError('empty'))
    if (file.size > IM_ATTACHMENT_MAX_BYTES) return Promise.reject(new ImActionError('too_large'))
    const image = file.type.startsWith('image/')
    const voice = file.type.startsWith('audio/')
    const pending: PendingMessage = {
      clientMsgId: newClientMsgId(),
      conversationId,
      kind: image ? 'image' : voice ? 'voice' : 'file',
      text: '',
      name: file.name || (image ? 'image.png' : 'file'),
      sizeBytes: file.size,
      localUrl: image ? this.deps.createObjectUrl(file) : null,
      createdAt: Date.now(),
      state: 'sending'
    }
    this.files.set(pending.clientMsgId, file)
    this.patchThread(conversationId, (thread) => ({ ...thread, pending: [...thread.pending, pending] }))
    return this.deliver(pending)
  }

  /** Resends with the same client id; the server returns the original message if it already landed. */
  retrySend(conversationId: string, clientMsgId: string): Promise<void> {
    const pending = this.thread(conversationId).pending.find((item) => item.clientMsgId === clientMsgId)
    if (!pending) return Promise.resolve()
    const next = { ...pending, state: 'sending' as const }
    this.replacePending(next)
    return this.deliver(next)
  }

  /** Removes one message from this member's thread. It does not recall the message. */
  dismiss(conversationId: string, messageId: string): void {
    if (!messageId) return
    this.rememberDismissed(messageId)
    this.patchThread(conversationId, (thread) => ({
      ...thread,
      messages: thread.messages.filter((message) => message.id !== messageId)
    }))
    this.patchEntry(conversationId, (entry) => this.previewAfterDismiss(entry))
  }

  discardPending(conversationId: string, clientMsgId: string): void {
    const pending = this.thread(conversationId).pending.find((item) => item.clientMsgId === clientMsgId)
    this.droppedSends.add(clientMsgId)
    if (pending?.localUrl) this.deps.revokeObjectUrl(pending.localUrl)
    this.files.delete(clientMsgId)
    this.uploads.delete(clientMsgId)
    this.patchThread(conversationId, (thread) => ({
      ...thread,
      pending: thread.pending.filter((item) => item.clientMsgId !== clientMsgId)
    }))
  }

  private async deliver(pending: PendingMessage): Promise<void> {
    const api = this.requireApi()
    const generation = this.generation
    try {
      pending = await this.bindBuiltinConversation(pending)
      if (generation !== this.generation) return
      let body: ImOutgoing
      if (pending.kind === 'text') {
        body = { kind: 'text', text: pending.text }
      } else {
        let attachment = this.uploads.get(pending.clientMsgId)
        if (!attachment) {
          const file = this.files.get(pending.clientMsgId)
          if (!file) throw new ImActionError('request')
          attachment = await api.upload(file, pending.name)
          this.uploads.set(pending.clientMsgId, attachment)
        }
        body = { kind: pending.kind, attachment }
      }
      const message = await api.send(pending.conversationId, pending.clientMsgId, body)
      if (generation !== this.generation) return
      if (pending.localUrl) this.deps.revokeObjectUrl(pending.localUrl)
      this.files.delete(pending.clientMsgId)
      this.uploads.delete(pending.clientMsgId)
      if (this.droppedSends.delete(pending.clientMsgId)) {
        this.rememberDismissed(message.id)
        this.patchThread(pending.conversationId, (thread) => ({
          ...thread,
          pending: thread.pending.filter((item) => item.clientMsgId !== pending.clientMsgId)
        }))
        this.patchEntry(pending.conversationId, (entry) => this.previewAfterDismiss(entry))
        return
      }
      this.patchThread(pending.conversationId, (thread) => ({
        ...thread,
        messages: thread.loaded ? this.mergeKept(thread.messages, [message]) : thread.messages,
        pending: thread.pending.filter((item) => item.clientMsgId !== pending.clientMsgId)
      }))
      this.patchEntry(pending.conversationId, (entry) => ({
        ...entry,
        lastSequence: Math.max(entry.lastSequence, message.sequence),
        lastReadSequence: Math.max(entry.lastReadSequence, message.sequence),
        unread: Math.max(0, entry.lastSequence - Math.max(entry.lastReadSequence, message.sequence)),
        preview: previewOf(message),
        updatedAt: Math.max(entry.updatedAt, message.createdAt)
      }))
    } catch (error) {
      if (generation !== this.generation) return
      this.replacePending({ ...pending, state: 'failed' })
      throw error instanceof ImActionError ? error : new ImActionError('request', errorMessage(error))
    }
  }

  async recall(conversationId: string, messageId: string): Promise<void> {
    const message = await this.requireApi().recall(messageId)
    this.patchThread(conversationId, (thread) => ({
      ...thread,
      messages: this.mergeKept(thread.messages, [message])
    }))
    this.patchEntry(conversationId, (entry) =>
      entry.preview?.messageId === messageId
        ? { ...entry, preview: { ...entry.preview, recalled: true, text: null, name: null, code: null, params: {} } }
        : entry
    )
  }

  // --- conversation settings ---------------------------------------------

  /** One unread on the ToBot row. A later list refresh keeps it until the row is opened. */
  noteTransferUnread(conversationId: string): void {
    const entry = this.snapshot.conversations.find((item) => item.id === conversationId)
    if (!entry || entry.kind !== 'transfer' || entry.unread > 0 || this.transferAttention.has(conversationId)) return
    this.transferAttention.add(conversationId)
    this.patchEntry(conversationId, (current) => (current.unread > 0 ? current : { ...current, unread: 1 }))
  }

  async markUnread(conversationId: string): Promise<void> {
    this.holdUnread.add(conversationId)
    if (this.activeId === conversationId) this.activeId = null
    if (this.settingsLocally(conversationId)) {
      this.patchEntry(conversationId, (entry) => ({
        ...entry,
        unread: Math.max(entry.unread, 1),
        lastReadSequence: entry.lastSequence > 0 ? entry.lastSequence - 1 : 0
      }))
      return
    }
    try {
      const lastRead = await this.requireApi().markUnread(conversationId)
      this.patchEntry(conversationId, (entry) => ({
        ...entry,
        lastReadSequence: lastRead,
        unread: Math.max(entry.unread, Math.max(0, entry.lastSequence - lastRead), 1)
      }))
    } catch (error) {
      this.holdUnread.delete(conversationId)
      throw error
    }
  }

  async setMuted(conversationId: string, muted: boolean): Promise<void> {
    if (!this.settingsLocally(conversationId)) await this.requireApi().setMuted(conversationId, muted)
    this.patchEntry(conversationId, (entry) => ({ ...entry, muted }))
  }

  async setPinned(conversationId: string, pinned: boolean): Promise<void> {
    if (!this.settingsLocally(conversationId)) await this.requireApi().setPinned(conversationId, pinned)
    this.rememberPin(conversationId, pinned)
    this.patchEntry(conversationId, (entry) => ({ ...entry, pinned }))
  }

  async startDirect(peerUserId: string): Promise<string> {
    const id = await this.requireApi().startDirect(peerUserId)
    await this.refreshList()
    return id
  }

  async createGroup(title: string, memberUserIds: string[]): Promise<string> {
    const id = await this.requireApi().createGroup(title, memberUserIds)
    await this.refreshList()
    return id
  }

  async addMembers(conversationId: string, userIds: string[]): Promise<void> {
    await this.requireApi().addMembers(conversationId, userIds)
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async removeMember(conversationId: string, userId: string): Promise<void> {
    await this.requireApi().removeMember(conversationId, userId)
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async rename(conversationId: string, title: string): Promise<void> {
    await this.requireApi().rename(conversationId, title)
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async setRemark(conversationId: string, remark: string): Promise<void> {
    const trimmed = remark.trim()
    if (!this.settingsLocally(conversationId)) await this.requireApi().setRemark(conversationId, trimmed)
    this.patchEntry(conversationId, (entry) => ({ ...entry, remark: trimmed || null }))
  }

  async setNickname(conversationId: string, nickname: string): Promise<void> {
    const trimmed = nickname.trim()
    await this.requireApi().setNickname(conversationId, trimmed)
    this.patchDetail(conversationId, (detail) => ({
      ...detail,
      members: detail.members.map((member) =>
        member.userId === this.snapshot.me?.userId ? { ...member, nickname: trimmed || null } : member
      )
    }))
  }

  async setMuteAll(conversationId: string, muteAll: boolean): Promise<void> {
    await this.requireApi().setMuteAll(conversationId, muteAll)
    this.patchDetail(conversationId, (detail) => ({ ...detail, muteAll }))
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async setMemberSpeech(conversationId: string, userId: string, banned: boolean): Promise<void> {
    await this.requireApi().setMemberSpeech(conversationId, userId, banned)
    this.patchDetail(conversationId, (detail) => ({
      ...detail,
      members: detail.members.map((member) => (member.userId === userId ? { ...member, speechBanned: banned } : member))
    }))
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async transferOwner(conversationId: string, userId: string): Promise<void> {
    await this.requireApi().transferOwner(conversationId, userId)
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  async setMemberRole(conversationId: string, userId: string, role: 'admin' | 'member'): Promise<void> {
    await this.requireApi().setMemberRole(conversationId, userId, role)
    await Promise.all([this.refreshDetail(conversationId), this.refreshList()])
  }

  /** Best effort. A dropped typing hint must not surface as a send failure. */
  async notifyTyping(conversationId: string): Promise<void> {
    try {
      await this.requireApi().typing(conversationId)
    } catch {
      // The next keystroke can try again.
    }
  }

  async leave(conversationId: string): Promise<void> {
    const entry = this.snapshot.conversations.find((item) => item.id === conversationId)
    if (
      conversationId === BUILTIN_TOBOT_ID ||
      conversationId === BUILTIN_TOBOSS_ID ||
      conversationId === this.fileTransferId ||
      entry?.kind === 'transfer' ||
      entry?.kind === 'boss'
    ) {
      return
    }
    await this.requireApi().leave(conversationId)
    if (this.activeId === conversationId) this.activeId = null
    this.set({
      ...this.snapshot,
      threads: omit(this.snapshot.threads, conversationId),
      details: omit(this.snapshot.details, conversationId),
      conversations: this.snapshot.conversations.filter((entry) => entry.id !== conversationId)
    })
  }

  async dissolve(conversationId: string): Promise<void> {
    const entry = this.snapshot.conversations.find((item) => item.id === conversationId)
    if (!entry || entry.kind !== 'group') return
    await this.requireApi().dissolve(conversationId)
    if (this.activeId === conversationId) this.activeId = null
    this.set({
      ...this.snapshot,
      threads: omit(this.snapshot.threads, conversationId),
      details: omit(this.snapshot.details, conversationId),
      conversations: this.snapshot.conversations.filter((item) => item.id !== conversationId)
    })
  }

  private async refreshPresence(): Promise<void> {
    const api = this.api
    if (!api) return
    const generation = this.generation
    try {
      const renewed = await api.renewPresence()
      if (generation !== this.generation) return
      if (!renewed) {
        this.set({ ...this.snapshot, presenceKnown: false, online: [] })
        return
      }
      const ids = this.snapshot.directory.map((person) => person.userId).slice(0, 100)
      const online = await api.presence(ids)
      if (generation !== this.generation) return
      this.set({ ...this.snapshot, presenceKnown: true, online })
    } catch {
      if (generation !== this.generation) return
      this.set({ ...this.snapshot, presenceKnown: false, online: [] })
    }
  }

  private noteTyping(conversationId: string, userId: string, ttlSecs: number): void {
    const ttl = Math.max(1, ttlSecs)
    const until = Date.now() + ttl * 1000
    const existing = this.typingTimers.get(conversationId)
    if (existing) clearTimeout(existing)
    this.set({
      ...this.snapshot,
      typing: { ...this.snapshot.typing, [conversationId]: { userId, until } }
    })
    this.typingTimers.set(
      conversationId,
      setTimeout(() => {
        this.typingTimers.delete(conversationId)
        const current = this.snapshot.typing[conversationId]
        if (!current || current.until > Date.now()) return
        this.set({ ...this.snapshot, typing: omit(this.snapshot.typing, conversationId) })
      }, ttl * 1000)
    )
  }

  // --- events ------------------------------------------------------------

  private onFrame(frame: SseFrame): void {
    let data: Record<string, unknown>
    try {
      const parsed = JSON.parse(frame.data) as unknown
      if (!parsed || typeof parsed !== 'object') return
      data = parsed as Record<string, unknown>
    } catch {
      return
    }
    const conversationId = typeof data.conversation_id === 'string' ? data.conversation_id : ''
    if (!conversationId) return
    const known = this.snapshot.conversations.some((entry) => entry.id === conversationId)
    switch (frame.event) {
      case 'im.facts.message.created':
        if (known) this.queueSync(conversationId)
        else void this.refreshList()
        break
      case 'im.facts.message.recalled': {
        const messageId = typeof data.message_id === 'string' ? data.message_id : ''
        this.patchThread(conversationId, (thread) => ({
          ...thread,
          messages: thread.messages.map((message) =>
            message.id === messageId && !message.recalled
              ? { ...message, recalled: true, body: { kind: 'recalled', original: originalKind(message) } }
              : message
          )
        }))
        void this.refreshList()
        break
      }
      case 'im.ephemeral.typing': {
        const userId = typeof data.user_id === 'string' ? data.user_id : ''
        const ttlSecs = typeof data.ttl_secs === 'number' ? data.ttl_secs : 5
        if (!known || !userId || userId === this.snapshot.me?.userId) break
        this.noteTyping(conversationId, userId, ttlSecs)
        break
      }
      case 'im.facts.conversation.updated':
        void this.refreshList()
        if (this.snapshot.details[conversationId]) void this.refreshDetail(conversationId)
        if (this.thread(conversationId).loaded) this.queueSync(conversationId)
        break
      case 'im.facts.cursor.changed': {
        const userId = typeof data.user_id === 'string' ? data.user_id : ''
        const sequence = typeof data.sequence === 'number' ? data.sequence : 0
        const detail = this.snapshot.details[conversationId]
        if (detail) {
          this.set({
            ...this.snapshot,
            details: {
              ...this.snapshot.details,
              [conversationId]: {
                ...detail,
                members: detail.members.map((member) =>
                  member.userId === userId ? { ...member, lastReadSequence: sequence } : member
                )
              }
            }
          })
        }
        if (userId && userId === this.snapshot.me?.userId) {
          this.patchEntry(conversationId, (entry) => ({
            ...entry,
            lastReadSequence: sequence,
            unread: Math.max(0, entry.lastSequence - sequence)
          }))
        }
        break
      }
      default:
        break
    }
  }

  private queueSync(conversationId: string): void {
    this.syncQueue.add(conversationId)
    if (this.syncTimer) return
    this.syncTimer = setTimeout(() => {
      this.syncTimer = null
      const ids = [...this.syncQueue]
      this.syncQueue.clear()
      void this.syncConversations(ids).catch(() => undefined)
    }, this.deps.syncDelayMs)
  }

  private async syncConversations(ids: string[]): Promise<void> {
    if (ids.length === 0) return
    const api = this.requireApi()
    const generation = this.generation
    const cursors = ids.map((conversationId) => {
      const thread = this.thread(conversationId)
      const entry = this.snapshot.conversations.find((item) => item.id === conversationId)
      const sequence = thread.loaded ? (thread.messages.at(-1)?.sequence ?? 0) : (entry?.lastSequence ?? 0)
      return { conversationId, sequence }
    })
    const results = await api.sync(cursors)
    if (generation !== this.generation) return
    const reload: string[] = []
    for (const result of results) {
      if (result.entry) this.upsertEntry(result.entry)
      const thread = this.thread(result.conversationId)
      if (!thread.loaded) continue
      if (result.messages.length >= 100) {
        reload.push(result.conversationId)
        continue
      }
      this.patchThread(result.conversationId, (current) => ({
        ...current,
        messages: this.mergeKept(current.messages, result.messages)
      }))
      this.patchEntry(result.conversationId, (entry) => this.previewAfterDismiss(entry))
    }
    for (const conversationId of reload) {
      const latest = await api.messages(conversationId)
      if (generation !== this.generation) return
      this.patchThread(conversationId, (current) => ({
        ...current,
        messages: this.mergeKept([], latest),
        hasOlder: latest.length >= IM_PAGE_LIMIT
      }))
      this.patchEntry(conversationId, (entry) => this.previewAfterDismiss(entry))
    }
    for (const conversationId of ids) await this.readIfActive(conversationId)
  }

  private async resyncAll(): Promise<void> {
    try {
      await this.refreshList()
      const loaded = Object.entries(this.snapshot.threads)
        .filter(([, thread]) => thread.loaded)
        .map(([id]) => id)
      await this.syncConversations(loaded)
      await Promise.all(Object.keys(this.snapshot.details).map((id) => this.refreshDetail(id)))
    } catch {
      // The next event or reconnect retries.
    }
  }

  private refreshList(): Promise<void> {
    if (this.listRefresh) return this.listRefresh
    const api = this.api
    if (!api) return Promise.resolve()
    const generation = this.generation
    this.listRefresh = api
      .conversations()
      .then((entries) => {
        if (generation === this.generation) this.publishConversations(entries)
      })
      .finally(() => {
        if (generation === this.generation) this.listRefresh = null
      })
    return this.listRefresh
  }

  /** Publishes a team inbox: the creator's team ToBot may stay, the personal ToBot does not. */
  private publishConversations(entries: ImInboxEntry[], patch: Partial<ImSnapshot> = {}): void {
    const listed = this.retainToBoss(this.withoutPersonalBot(entries)).map((entry) => this.previewAfterDismiss(entry))
    this.adoptServerPins(listed)
    const conversations = sortEntries(listed, this.pinnedAt)
    if (this.fileTransferId) this.moveBuiltinThread(this.fileTransferId)
    this.set({ ...this.snapshot, ...patch, conversations })
    this.warmGroupFaces()
  }

  /** A team inbox never lists the personal ToBot, including a server transfer row. */
  private withoutPersonalBot(entries: ImInboxEntry[]): ImInboxEntry[] {
    return entries.filter((entry) => entry.kind !== 'transfer' && entry.id !== BUILTIN_TOBOT_ID)
  }

  /** The creator's team ToBot is local. A list refresh keeps its pin, mute, and unread. */
  private retainToBoss(entries: ImInboxEntry[]): ImInboxEntry[] {
    const listed = entries.filter((entry) => entry.kind !== 'boss' && entry.id !== BUILTIN_TOBOSS_ID)
    if (!this.teamCreator || !this.snapshot.orgId) return listed
    const kept = this.snapshot.conversations.find((entry) => entry.id === BUILTIN_TOBOSS_ID)
    return [...listed, kept ?? emptyBoss()]
  }

  /** Personal mode may bind the built-in row to a server id. A team inbox never asks. */
  private async ensureFileTransfer(): Promise<string | null> {
    if (this.snapshot.orgId) return null
    if (this.fileTransferId) return this.fileTransferId
    const api = this.api
    if (!api) return null
    try {
      const id = await api.fileTransfer()
      if (!id || id === BUILTIN_TOBOT_ID) return null
      this.fileTransferId = id
      return id
    } catch {
      return null
    }
  }

  /** Moves the local ToBot thread onto the server conversation before the next snapshot publish. */
  private moveBuiltinThread(serverId: string): void {
    if (serverId === BUILTIN_TOBOT_ID) return
    const hasBuiltin =
      this.activeId === BUILTIN_TOBOT_ID ||
      this.snapshot.threads[BUILTIN_TOBOT_ID] !== undefined ||
      this.snapshot.details[BUILTIN_TOBOT_ID] !== undefined
    if (!hasBuiltin) return
    const threads = { ...this.snapshot.threads }
    const moved = threads[BUILTIN_TOBOT_ID]
    delete threads[BUILTIN_TOBOT_ID]
    if (moved) {
      const remap = (item: PendingMessage) =>
        item.conversationId === BUILTIN_TOBOT_ID ? { ...item, conversationId: serverId } : item
      const existing = threads[serverId]
      threads[serverId] = existing
        ? { ...existing, pending: [...existing.pending, ...moved.pending.map(remap)] }
        : { ...moved, pending: moved.pending.map(remap), loaded: false }
    }
    const details = { ...this.snapshot.details }
    const builtinDetail = details[BUILTIN_TOBOT_ID]
    if (builtinDetail && !details[serverId]) details[serverId] = { ...builtinDetail, id: serverId }
    delete details[BUILTIN_TOBOT_ID]
    if (this.activeId === BUILTIN_TOBOT_ID) this.activeId = serverId
    this.snapshot = { ...this.snapshot, threads, details }
  }

  private async bindBuiltinConversation(pending: PendingMessage): Promise<PendingMessage> {
    if (pending.conversationId !== BUILTIN_TOBOT_ID) return pending
    const resolved = await this.ensureFileTransfer()
    if (!resolved) throw new ImActionError('request', 'not connected')
    const listed = this.snapshot.conversations.filter((entry) => entry.id !== BUILTIN_TOBOT_ID)
    this.publishConversations(listed)
    return { ...pending, conversationId: resolved }
  }

  private async refreshDetail(conversationId: string): Promise<void> {
    const api = this.api
    if (!api) return
    const generation = this.generation
    try {
      const detail = await api.detail(conversationId)
      if (generation !== this.generation) return
      this.set({ ...this.snapshot, details: { ...this.snapshot.details, [conversationId]: detail } })
    } catch {
      // Left or removed: drop the stale roster.
      if (generation !== this.generation) return
      this.set({ ...this.snapshot, details: omit(this.snapshot.details, conversationId) })
    }
  }

  /** The inbox row has no member ids. Group portraits need the detail roster. */
  private warmGroupFaces(): void {
    for (const entry of this.snapshot.conversations) {
      if (entry.kind !== 'group' || this.snapshot.details[entry.id]) continue
      void this.refreshDetail(entry.id)
    }
  }

  /** Personal ToBot and the placeholder id have no server conversation to update. */
  private settingsLocally(conversationId: string): boolean {
    return this.api == null || conversationId === BUILTIN_TOBOT_ID || conversationId === BUILTIN_TOBOSS_ID
  }

  private async readIfActive(conversationId: string): Promise<void> {
    if (this.holdUnread.has(conversationId)) return
    if (this.activeId !== conversationId || !this.deps.isVisible()) return
    const entry = this.snapshot.conversations.find((item) => item.id === conversationId)
    if (!entry || entry.lastReadSequence >= entry.lastSequence) return
    const generation = this.generation
    const lastRead = await this.requireApi().markRead(conversationId)
    if (generation !== this.generation) return
    this.patchEntry(conversationId, (current) => ({
      ...current,
      lastReadSequence: Math.max(current.lastReadSequence, lastRead),
      unread: Math.max(0, current.lastSequence - Math.max(current.lastReadSequence, lastRead))
    }))
  }

  /** Window regained focus: read whatever arrived in the open conversation meanwhile. */
  onVisible(): void {
    if (this.activeId) void this.readIfActive(this.activeId).catch(() => undefined)
  }

  // --- state helpers -----------------------------------------------------

  private requireApi(): ImApi {
    if (!this.api) throw new ImActionError('request', 'not connected')
    return this.api
  }

  private thread(conversationId: string): ThreadState {
    return this.snapshot.threads[conversationId] ?? EMPTY_THREAD
  }

  private patchThread(conversationId: string, recipe: (thread: ThreadState) => ThreadState): void {
    this.set({
      ...this.snapshot,
      threads: { ...this.snapshot.threads, [conversationId]: recipe(this.thread(conversationId)) }
    })
  }

  private replacePending(next: PendingMessage): void {
    this.patchThread(next.conversationId, (thread) => ({
      ...thread,
      pending: thread.pending.map((item) => (item.clientMsgId === next.clientMsgId ? next : item))
    }))
  }

  private clearTransferAttention(conversationId: string): void {
    if (!this.transferAttention.delete(conversationId)) return
    this.patchEntry(conversationId, (entry) => {
      const unread = Math.max(0, entry.lastSequence - entry.lastReadSequence)
      return unread === entry.unread ? entry : { ...entry, unread }
    })
  }

  /** Puts the local ToBot unread back after a server inbox row arrives with zero. */
  private withTransferAttention(conversations: ImInboxEntry[]): ImInboxEntry[] {
    if (this.transferAttention.size === 0) return conversations
    let changed = false
    const next = conversations.map((entry) => {
      if (!this.transferAttention.has(entry.id) || entry.kind !== 'transfer' || entry.unread > 0) return entry
      changed = true
      return { ...entry, unread: 1 }
    })
    return changed ? next : conversations
  }

  private patchDetail(conversationId: string, recipe: (detail: ImConversationDetail) => ImConversationDetail): void {
    const detail = this.snapshot.details[conversationId]
    if (!detail) return
    this.set({
      ...this.snapshot,
      details: { ...this.snapshot.details, [conversationId]: recipe(detail) }
    })
  }

  private patchEntry(conversationId: string, recipe: (entry: ImInboxEntry) => ImInboxEntry): void {
    const index = this.snapshot.conversations.findIndex((entry) => entry.id === conversationId)
    if (index < 0) return
    const conversations = this.snapshot.conversations.slice()
    conversations[index] = recipe(conversations[index])
    this.set({ ...this.snapshot, conversations: sortEntries(conversations, this.pinnedAt) })
  }

  private upsertEntry(next: ImInboxEntry): void {
    const scrubbed = this.previewAfterDismiss(next)
    const rest = this.snapshot.conversations.filter((entry) => entry.id !== scrubbed.id)
    this.set({ ...this.snapshot, conversations: sortEntries([...rest, scrubbed], this.pinnedAt) })
  }

  private mergeKept(current: readonly ImMessage[], incoming: readonly ImMessage[]): ImMessage[] {
    const merged = mergeMessages(current, incoming)
    if (this.dismissed.size === 0) return merged
    return merged.filter((message) => !this.dismissed.has(message.id))
  }

  private rememberDismissed(messageId: string): void {
    this.dismissed.add(messageId)
    while (this.dismissed.size > DISMISSED_CAP) {
      const oldest = this.dismissed.values().next().value
      if (!oldest) break
      this.dismissed.delete(oldest)
    }
    writeDismissed(this.dismissed)
  }

  /** A removed latest message yields the previous message this member can still see. */
  private previewAfterDismiss(entry: ImInboxEntry): ImInboxEntry {
    const thread = this.thread(entry.id)
    const last = thread.messages.at(-1)
    const hidden = Boolean(entry.preview && this.dismissed.has(entry.preview.messageId))
    if (entry.preview && !hidden) return entry
    if (!last) return hidden ? { ...entry, preview: null } : entry
    if (hidden || (thread.loaded && this.dismissed.size > 0)) return { ...entry, preview: previewOf(last) }
    return entry
  }

  /** The latest pin leads. A later message does not pass it. */
  private rememberPin(conversationId: string, pinned: boolean): void {
    if (pinned) this.pinnedAt.set(conversationId, ++this.pinClock)
    else this.pinnedAt.delete(conversationId)
    writePinOrder(this.pinnedAt)
  }

  /** A list refresh drops a pin the server no longer has. */
  private adoptServerPins(entries: readonly ImInboxEntry[]): void {
    let changed = false
    for (const entry of entries) {
      if (!entry.pinned && this.pinnedAt.delete(entry.id)) changed = true
    }
    if (changed) writePinOrder(this.pinnedAt)
  }

  private set(next: ImSnapshot): void {
    const conversations = this.withTransferAttention(next.conversations)
    this.snapshot = conversations === next.conversations ? next : { ...next, conversations }
    for (const listener of this.listeners) listener()
  }
}

function omit<T>(map: Record<string, T>, key: string): Record<string, T> {
  const next = { ...map }
  delete next[key]
  return next
}

export function mergeMessages(current: readonly ImMessage[], incoming: readonly ImMessage[]): ImMessage[] {
  const byId = new Map(current.map((message) => [message.id, message]))
  for (const message of incoming) byId.set(message.id, message)
  return [...byId.values()].sort((left, right) => left.sequence - right.sequence)
}

function emptyFileTransfer(id: string): ImInboxEntry {
  return emptyBuiltin(id, 'transfer')
}

function emptyBoss(): ImInboxEntry {
  return emptyBuiltin(BUILTIN_TOBOSS_ID, 'boss')
}

function emptyBuiltin(id: string, kind: 'transfer' | 'boss'): ImInboxEntry {
  return {
    id,
    kind,
    title: null,
    peerUserId: null,
    memberCount: 1,
    lastSequence: 0,
    lastReadSequence: 0,
    unread: 0,
    preview: null,
    muted: false,
    pinned: false,
    updatedAt: 0
  }
}

const PIN_ORDER_KEY = 'messages.pin-order.v1'
const DISMISSED_KEY = 'messages.dismissed.v1'
const DISMISSED_CAP = 2000

function readDismissed(): Set<string> {
  try {
    const raw = globalThis.localStorage?.getItem(DISMISSED_KEY)
    if (!raw) return new Set()
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return new Set()
    return new Set(parsed.filter((item): item is string => typeof item === 'string' && item.length > 0))
  } catch {
    return new Set()
  }
}

function writeDismissed(ids: ReadonlySet<string>): void {
  try {
    globalThis.localStorage?.setItem(DISMISSED_KEY, JSON.stringify([...ids]))
  } catch {
    // A full disk keeps the in-memory set for this process.
  }
}

function readPinOrder(): Map<string, number> {
  try {
    const raw = globalThis.localStorage?.getItem(PIN_ORDER_KEY)
    if (!raw) return new Map()
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object') return new Map()
    return new Map(
      Object.entries(parsed).filter((entry): entry is [string, number] => typeof entry[1] === 'number' && entry[1] > 0)
    )
  } catch {
    return new Map()
  }
}

function writePinOrder(order: ReadonlyMap<string, number>): void {
  try {
    const body: Record<string, number> = {}
    for (const [id, value] of order) body[id] = value
    globalThis.localStorage?.setItem(PIN_ORDER_KEY, JSON.stringify(body))
  } catch {
    // The in-memory order still leads this session.
  }
}

/** Among pinned rows, the latest pin leads. Message time orders every other tie. */
function sortEntries(entries: ImInboxEntry[], pinnedAt: ReadonlyMap<string, number>): ImInboxEntry[] {
  return entries.slice().sort((left, right) => {
    if (left.pinned && right.pinned) {
      const delta = (pinnedAt.get(right.id) ?? 0) - (pinnedAt.get(left.id) ?? 0)
      if (delta !== 0) return delta
    }
    return right.updatedAt - left.updatedAt || (left.id < right.id ? 1 : -1)
  })
}

function originalKind(message: ImMessage) {
  return message.body.kind === 'recalled' ? message.body.original : message.body.kind
}

function previewBodyText(raw: string): string {
  const wire = decodeBossWire(raw)
  if (!wire) return visibleText(raw).slice(0, 120)
  return encodeBossWire([...wire.text].slice(0, 80).join(''), wire.kind)
}

function previewOf(message: ImMessage): ImInboxEntry['preview'] {
  const body = message.body
  return {
    kind: originalKind(message),
    messageId: message.id,
    sequence: message.sequence,
    recalled: message.recalled,
    createdAt: message.createdAt,
    sender: message.sender,
    text: body.kind === 'text' ? previewBodyText(body.text) : null,
    name: body.kind === 'file' || body.kind === 'image' || body.kind === 'voice' ? body.attachment.name : null,
    code: body.kind === 'system' ? body.code : null,
    params: body.kind === 'system' ? body.params : {}
  }
}

function newClientMsgId(): string {
  const random =
    globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
  return `studio-${random}`
}

function errorMessage(error: unknown): string {
  if (error instanceof ImApiError) return error.status ? `${error.status}: ${error.message}` : error.message
  return error instanceof Error ? error.message : String(error)
}

const DEVICE_KEY = 'a3s.im.device'

function studioDeviceId(): string {
  try {
    const existing = localStorage.getItem(DEVICE_KEY)
    if (existing && /^[A-Za-z0-9_-]{1,64}$/.test(existing)) return existing
    const created = `studio-${Math.random().toString(36).slice(2, 12)}`
    localStorage.setItem(DEVICE_KEY, created)
    return created
  } catch {
    return 'studio'
  }
}

let shared: ImStore | null = null

export function sharedImStore(): ImStore {
  shared ??= new ImStore()
  return shared
}

/** Test isolation only. */
export function __resetSharedImStore(next: ImStore | null = null): void {
  shared?.stop()
  shared = next
}
