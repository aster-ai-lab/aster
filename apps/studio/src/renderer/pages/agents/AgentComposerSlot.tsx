import { memo, use, useMemo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { useRightPanelPresentationMaximized } from '@renderer/components/chat/panes/Shell'
import { type ComposerContextValue, selectActiveComposerOverride } from '@renderer/components/composer/ComposerContext'
import ConversationComposerSlot from '@renderer/components/composer/ConversationComposerSlot'
import AgentComposer, {
  type AgentComposerLaunchOptions,
  MissingAgentHomeComposer
} from '@renderer/pages/agents/AgentComposer'
import type { GetAgentResponse } from '@renderer/types/agent'
import type { AgentSessionEntity } from '@shared/data/api/schemas/agentSessions'
import type { Model } from '@shared/data/types/model'

import type { AgentChatRuntimeState } from './useAgentChatRuntimeState'
import { WorkbenchProjectLockedContext } from './workbenchComposerHost'

function workbenchEditDescription(
  t: (key: string) => string,
  workbenchConversation: boolean,
  restoreFiles: boolean | undefined
): string {
  if (!workbenchConversation) return t('agent.edit_resend.warning')
  if (restoreFiles) return t('workbench.message.edit_files_restore')
  return t('workbench.message.edit_files_stay')
}

interface AgentComposerSlotProps {
  agentId?: string
  activeAgent?: GetAgentResponse
  activeModel?: Model
  workspaceWarning?: string
  isMultiSelectMode: boolean
  session: AgentSessionEntity
  sessionId: string
  sendMessage: AgentChatRuntimeState['sendMessage']
  stop: AgentChatRuntimeState['stop']
  planMessageId?: string
  planRevision?: string
  taskProgress?: ReactNode
  isStreaming: boolean
  sendDisabled: boolean
  onAgentChange?: (agentId: string | null) => void | Promise<void>
  agentChanging?: boolean
  onCreateEmptySession?: () => void | Promise<unknown>
  composerContext: ComposerContextValue
  composerLaunchOptions?: AgentComposerLaunchOptions
  editing?: AgentChatRuntimeState['editing']
  editBusy?: boolean
  cancelEditing: () => void
  resendEditedMessage: AgentChatRuntimeState['sendMessage']
  onWorkspaceChange?: (workspaceId: string | null) => void | Promise<void>
  workspaceChanging?: boolean
}

function AgentComposerSlot({
  agentId,
  activeAgent,
  activeModel,
  workspaceWarning,
  isMultiSelectMode,
  session,
  sessionId,
  sendMessage,
  stop,
  planMessageId,
  planRevision,
  taskProgress,
  isStreaming,
  sendDisabled,
  onAgentChange,
  agentChanging,
  onCreateEmptySession,
  composerContext,
  composerLaunchOptions,
  editing,
  editBusy,
  cancelEditing,
  resendEditedMessage,
  onWorkspaceChange,
  workspaceChanging
}: AgentComposerSlotProps) {
  const compactWhenSingleLine = useRightPanelPresentationMaximized()
  const { t } = useTranslation()
  const workbenchConversation = use(WorkbenchProjectLockedContext)
  const editLaunchOptions = useMemo<AgentComposerLaunchOptions | undefined>(
    () =>
      editing
        ? {
            initialDraft: { text: '', tokens: [] },
            initialParts: editing.parts,
            editing: {
              messageId: editing.messageId,
              onCancel: cancelEditing,
              description: workbenchEditDescription(t, workbenchConversation, editing.restoreFiles),
              sendLabel: t('agent.edit_resend.save')
            }
          }
        : undefined,
    [cancelEditing, editing, t, workbenchConversation]
  )
  const renderComposer = (isEditing = false) =>
    agentId ? (
      <AgentComposer
        agentId={agentId}
        sessionId={sessionId}
        sessionOverride={session}
        resolvedAgent={activeAgent}
        resolvedModel={activeModel}
        resolvedWorkspaceWarning={workspaceWarning ?? null}
        externalContextControls
        canChangeModel={!isEditing}
        sendMessage={isEditing ? resendEditedMessage : sendMessage}
        stop={stop}
        planMessageId={isEditing ? undefined : planMessageId}
        planRevision={isEditing ? undefined : planRevision}
        taskProgress={isEditing || hasOverride ? undefined : taskProgress}
        isStreaming={!isEditing && isStreaming}
        sendDisabled={sendDisabled || (isEditing && (isStreaming || editBusy))}
        onCreateEmptySession={isEditing ? undefined : onCreateEmptySession}
        compactWhenSingleLine={compactWhenSingleLine}
        launchOptions={isEditing ? editLaunchOptions : composerLaunchOptions}
        onWorkspaceChange={isEditing ? undefined : onWorkspaceChange}
        workspaceChanging={isEditing ? undefined : workspaceChanging}
      />
    ) : (
      <MissingAgentHomeComposer onAgentChange={onAgentChange} agentChanging={agentChanging} />
    )
  const context: ComposerContextValue = {
    ...composerContext,
    overrides: [
      ...(composerContext.overrides ?? []),
      ...(editing && agentId
        ? [{ id: `edit:${editing.messageId}`, priority: 10, render: () => renderComposer(true) }]
        : [])
    ]
  }

  const hasOverride = Boolean(selectActiveComposerOverride(context.overrides))
  return (
    <>
      {!isMultiSelectMode && !editing && hasOverride ? taskProgress : null}
      <ConversationComposerSlot composerContext={context} fallback={!isMultiSelectMode && renderComposer()} />
    </>
  )
}

export default memo(AgentComposerSlot)
