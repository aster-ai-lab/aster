import { getToolName, isToolUIPart } from 'ai'
import { useEffect, useMemo } from 'react'

import { isAskUserQuestionToolName } from '@renderer/components/chat/messages/tools/shared/agentToolTypes'
import type { MessageStreamingLayers, MessageToolApprovalInput } from '@renderer/components/chat/messages/types'
import type { PendingToolApproval } from '@shared/ai/transport'
import type { CherryMessagePart } from '@shared/data/types/message'

import type { ComposerOverride } from './ComposerContext'
import { createAskUserQuestionComposerOverride } from './variants/AskUserQuestionComposer'
import { findLatestPendingAskUserQuestionRequest } from './variants/askUserQuestionComposerRequest'
import { clearAskUserQuestionDraftCache } from './variants/askUserQuestionDraftCache'
import { createPermissionRequestComposerOverride } from './variants/PermissionRequestComposer'
import { findNextPendingPermissionRequest } from './variants/permissionRequestComposerRequest'

type ToolApprovalComposerOverridesOptions = {
  partsByMessageId: Record<string, CherryMessagePart[]>
  /** Database message parts, before live overlays or optimistic projections. */
  persistedPartsByMessageId: Record<string, CherryMessagePart[]>
  streamingLayers?: MessageStreamingLayers
  /** Approvals the route already accepted. The card must not wait on a transcript reload. */
  dismissedApprovalIds?: ReadonlySet<string>
  /** Undefined for legacy transports; an empty native set must hide historical pending parts. */
  pendingApprovals?: readonly PendingToolApproval[]
  onRespond: (input: MessageToolApprovalInput) => void | Promise<void>
}

export function useToolApprovalComposerOverrides({
  partsByMessageId,
  persistedPartsByMessageId,
  streamingLayers,
  dismissedApprovalIds,
  pendingApprovals,
  onRespond
}: ToolApprovalComposerOverridesOptions): readonly ComposerOverride[] {
  useEffect(() => {
    for (const parts of Object.values(persistedPartsByMessageId)) {
      for (const part of parts) {
        if (!isToolUIPart(part) || !isAskUserQuestionToolName(getToolName(part))) continue
        if (
          part.state !== 'approval-responded' &&
          part.state !== 'output-available' &&
          part.state !== 'output-denied'
        ) {
          continue
        }
        if (part.approval?.id) clearAskUserQuestionDraftCache(part.approval.id)
      }
    }
  }, [persistedPartsByMessageId])

  const settledHistoryParts = useMemo<Record<string, CherryMessagePart[]> | null>(() => {
    if (!streamingLayers) return null

    const liveMessageIdSet = new Set(streamingLayers.liveMessageIds)
    const historyParts: Record<string, CherryMessagePart[]> = {}
    for (const [messageId, parts] of Object.entries(streamingLayers.historyPartsByMessageId)) {
      if (!liveMessageIdSet.has(messageId)) historyParts[messageId] = parts
    }
    return historyParts
  }, [streamingLayers])
  const currentParts = useMemo<Record<string, CherryMessagePart[]>>(() => {
    if (!streamingLayers) return partsByMessageId

    const liveParts: Record<string, CherryMessagePart[]> = {}
    for (const messageId of streamingLayers.liveMessageIds) {
      const parts = partsByMessageId[messageId]
      if (parts) liveParts[messageId] = parts
    }
    return liveParts
  }, [streamingLayers, partsByMessageId])
  const nativeParts = useMemo(() => {
    if (!pendingApprovals) return null
    const parts: Record<string, CherryMessagePart[]> = {}
    for (const approval of pendingApprovals) {
      const matches = (part: CherryMessagePart) =>
        isToolUIPart(part) && part.toolCallId === approval.toolCallId && part.approval?.id === approval.approvalId
      // A newer persisted request can be absent from an older execution overlay.
      const part =
        persistedPartsByMessageId[approval.messageId]?.find(matches) ??
        partsByMessageId[approval.messageId]?.find(matches)
      if (part && isToolUIPart(part) && part.state === 'approval-requested') {
        const messageParts = (parts[approval.messageId] ??= [])
        messageParts.push(part)
      }
    }
    return parts
  }, [pendingApprovals, partsByMessageId, persistedPartsByMessageId])
  const historyAskUserQuestionRequest = useMemo(
    () =>
      nativeParts === null && settledHistoryParts ? findLatestPendingAskUserQuestionRequest(settledHistoryParts) : null,
    [nativeParts, settledHistoryParts]
  )
  const currentAskUserQuestionRequest = useMemo(
    // Native questions use the owned form protocol; boolean approval receivers cannot accept edited answers.
    () => (nativeParts === null ? findLatestPendingAskUserQuestionRequest(currentParts) : null),
    [nativeParts, currentParts]
  )
  const askUserQuestionRequestRaw = currentAskUserQuestionRequest ?? historyAskUserQuestionRequest
  const askUserQuestionRequest =
    askUserQuestionRequestRaw && !dismissedApprovalIds?.has(askUserQuestionRequestRaw.approvalId)
      ? askUserQuestionRequestRaw
      : null
  const historyPermissionRequest = useMemo(
    () => (nativeParts === null && settledHistoryParts ? findNextPendingPermissionRequest(settledHistoryParts) : null),
    [nativeParts, settledHistoryParts]
  )
  const currentPermissionRequest = useMemo(
    () => findNextPendingPermissionRequest(nativeParts ?? currentParts),
    [nativeParts, currentParts]
  )
  const permissionRequestRaw = currentPermissionRequest ?? historyPermissionRequest
  const permissionRequest =
    permissionRequestRaw && !dismissedApprovalIds?.has(permissionRequestRaw.approvalId) ? permissionRequestRaw : null

  return useMemo(() => {
    const overrides: ComposerOverride[] = []

    if (askUserQuestionRequest) {
      overrides.push(
        createAskUserQuestionComposerOverride({
          request: askUserQuestionRequest,
          onRespond
        })
      )
    }

    if (permissionRequest) {
      overrides.push(
        createPermissionRequestComposerOverride({
          request: permissionRequest,
          allowRejectionReason: nativeParts === null,
          onRespond
        })
      )
    }

    return overrides
  }, [askUserQuestionRequest, nativeParts, onRespond, permissionRequest])
}
