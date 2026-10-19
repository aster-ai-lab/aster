import { useEffect, useRef, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { dataApiService } from '@data/DataApiService'
import { sharedImStore } from '@renderer/im/imStore'
import { conversationTitle, previewText } from '@renderer/im/imView'
import {
  attentionStep,
  notificationBody,
  tobotDeltaAction,
  tobotSettleAction,
  type AttentionRow
} from '@renderer/im/messageAttention'
import { tobotDisplayName, useToBotProfile } from '@renderer/im/tobotProfile'
import { withHarnessComponents } from '@renderer/services/harnessAssemble'
import {
  bindTobotSession,
  currentTobotSession,
  sessionIdFromEnsure,
  subscribeTobotSession,
  subscribeTobotStream,
  tobotTextDelta,
  tobotTopic
} from '@renderer/services/tobotStream'

import {
  listenMessageAttention,
  messageFocus,
  openMessageFromAttention,
  publishMessageNotification,
  setMessageAttention,
  subscribeMessageFocus,
  withdrawMessageNotification
} from './messageAttentionHost'

/** Dock count, menu-bar count, and one banner per new message. Mounted for the life of the shell. */
export function MessageAttentionRuntime() {
  const { t } = useTranslation()
  const store = sharedImStore()
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot)
  const focus = useSyncExternalStore(subscribeMessageFocus, messageFocus, messageFocus)
  const primed = useRef(false)
  const previousRows = useRef<AttentionRow[]>([])
  const clearing = useRef<string | null>(null)
  const botLabel = tobotDisplayName(useToBotProfile(), t('messages.file_transfer'))
  const copy = useRef({ title: '', fallback: '' })
  copy.current = { title: botLabel, fallback: t('messages.attention.tobot_reply', { name: botLabel }) }

  useEffect(() => listenMessageAttention(openMessageFromAttention), [])

  useEffect(() => {
    if (snapshot.status !== 'ready' && snapshot.status !== 'no-org') {
      primed.current = false
      setMessageAttention(0)
      return
    }
    const rows = snapshot.conversations.map((entry) => ({
      id: entry.id,
      kind: entry.kind,
      unread: entry.unread,
      muted: entry.muted,
      title: entry.kind === 'transfer' && !entry.title ? botLabel : conversationTitle(entry, snapshot, t),
      body: notificationBody(previewText(entry, snapshot, t), t('messages.attention.new_message'))
    }))
    const previous = primed.current ? previousRows.current : null
    primed.current = true
    previousRows.current = rows
    for (const action of attentionStep(previous, rows, messageFocus().lookedAtId)) {
      if (action.type === 'badge') setMessageAttention(action.count)
      else if (action.type === 'notify') publishMessageNotification(action.id, action.title, action.body)
      else withdrawMessageNotification(action.id)
    }
  }, [botLabel, snapshot, t])

  useEffect(() => {
    const id = focus.lookedAtId
    if (!id || clearing.current === id) return
    const entry = snapshot.conversations.find((item) => item.id === id)
    if (!entry || entry.kind !== 'transfer' || entry.unread <= 0) return
    clearing.current = id
    withdrawMessageNotification(id)
    void store.open(id).finally(() => {
      if (clearing.current === id) clearing.current = null
    })
  }, [focus.lookedAtId, snapshot, store])

  const tobotSessionId = useSyncExternalStore(subscribeTobotSession, currentTobotSession, currentTobotSession)

  useEffect(() => {
    let stopped = false
    const delays = [0, 500, 1500]
    const timers: ReturnType<typeof globalThis.setTimeout>[] = []
    const attempt = (index: number) => {
      if (stopped || index >= delays.length) return
      timers.push(
        globalThis.setTimeout(() => {
          if (stopped) return
          void withHarnessComponents({})
            .then((body) => (stopped ? null : dataApiService.post('/agent-sessions/tobot/ensure', { body })))
            .then(
              (ready) => {
                const id = sessionIdFromEnsure(ready)
                if (id) bindTobotSession(id)
              },
              () => attempt(index + 1)
            )
        }, delays[index] ?? 0)
      )
    }
    attempt(0)
    return () => {
      stopped = true
      for (const timer of timers) globalThis.clearTimeout(timer)
    }
  }, [])

  useEffect(() => {
    let text = ''
    let noted = false
    const topic = tobotTopic(tobotSessionId)
    return subscribeTobotStream(
      (payload) => {
        const delta = tobotTextDelta(payload, topic)
        if (!delta) return
        text += delta
        const transfer = store.getSnapshot().conversations.find((entry) => entry.kind === 'transfer')
        const action = tobotDeltaAction({
          noted,
          looking: messageFocus().lookedAtId != null && messageFocus().lookedAtId === transfer?.id,
          unread: transfer?.unread ?? 0,
          id: transfer?.id ?? null
        })
        if (action.type !== 'note') return
        noted = true
        store.noteTransferUnread(action.id)
      },
      () => {
        const transfer = store.getSnapshot().conversations.find((entry) => entry.kind === 'transfer')
        const action = tobotSettleAction({
          text,
          looking: Boolean(transfer && messageFocus().lookedAtId === transfer.id),
          id: transfer?.id ?? null,
          fallback: copy.current.fallback
        })
        text = ''
        noted = false
        if (action.type === 'publish') publishMessageNotification(action.id, copy.current.title, action.body)
        else if (action.type === 'withdraw') withdrawMessageNotification(action.id)
      },
      { replay: false, sessionId: tobotSessionId }
    )
  }, [store, tobotSessionId])

  return null
}
