import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { HideMessageMenuBarContext } from '@renderer/components/chat/messages/frame/MessageFrame'
import { HideMessageIdentityContext } from '@renderer/components/chat/messages/frame/MessageHeader'
import { useTopicStreamStatus } from '@renderer/hooks/useTopicStreamStatus'
import AgentSessionMessages from '@renderer/pages/agents/components/AgentSessionMessages'
import { useAgentChatRuntimeState } from '@renderer/pages/agents/useAgentChatRuntimeState'
import { buildAgentSessionTopicId } from '@renderer/utils/agentSession'

/** The live a3s code turn for one knowledge compile. The dialog does not compile by itself. */
export function KnowledgeCompileAgentRun({
  sessionId,
  agentId,
  onSettled,
  onFailed
}: {
  sessionId: string
  agentId: string
  onSettled: () => void
  onFailed: () => void
}) {
  const { t } = useTranslation()
  const runtime = useAgentChatRuntimeState({
    sessionId,
    sessionMessagesEnabled: true,
    reservedMessages: []
  })
  const topicId = buildAgentSessionTopicId(sessionId)
  const { status, isFulfilled, markSeen } = useTopicStreamStatus(topicId)
  const finished = useRef(false)
  const onSettledRef = useRef(onSettled)
  const onFailedRef = useRef(onFailed)
  onSettledRef.current = onSettled
  onFailedRef.current = onFailed

  useEffect(() => {
    if (finished.current) return
    if (status === 'error' || status === 'aborted') {
      finished.current = true
      onFailedRef.current()
      return
    }
    if (!isFulfilled) return
    finished.current = true
    markSeen()
    onSettledRef.current()
  }, [isFulfilled, markSeen, status])

  const executionMessages = runtime.uiMessages.filter((message) => message.role !== 'user')

  return (
    <div
      role="log"
      aria-label={t('knowledge.notes.compile_log')}
      data-ui="knowledge.compile.agent"
      className="max-h-80 min-h-40 overflow-y-auto">
      <HideMessageIdentityContext value={true}>
        <HideMessageMenuBarContext value={true}>
          <AgentSessionMessages
            agentId={agentId}
            sessionId={sessionId}
            messages={executionMessages}
            partsByMessageId={runtime.partsByMessageId}
            streamingLayers={runtime.streamingLayers}
            optimisticAskUserQuestionInputsByToolCallId={runtime.optimisticAskUserQuestionInputsByToolCallId}
            isLoading={runtime.isLoading}
            hasOlder={runtime.hasOlder}
            loadOlder={runtime.loadOlder}
            selectAllPagination={runtime.selectAllPagination}
            deleteMessage={runtime.deleteMessage}
            startEditing={runtime.startEditing}
            editBusy={runtime.editBusy}
            respondToolApproval={runtime.respondToolApproval}
          />
        </HideMessageMenuBarContext>
      </HideMessageIdentityContext>
    </div>
  )
}
