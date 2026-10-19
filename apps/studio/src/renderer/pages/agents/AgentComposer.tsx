import { Tooltip } from '@a3s-lab/ui'
import { Pointer, Settings2, Terminal, ToolCase } from 'lucide-react'
import React, { use, useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { AgentContextUsageSummary } from '@renderer/components/chat/agent/AgentContextUsageSummary'
import { ContextUsageMeter } from '@renderer/components/chat/contextUsage'
import { useChatLayoutMode } from '@renderer/components/chat/layout/ChatLayoutModeContext'
import {
  ConversationTopBarPortal,
  useConversationTopBarPortalLayout
} from '@renderer/components/chat/shell/ConversationTopBarPortal'
import { agentParameterTurn } from '@renderer/components/composer/agentParameterDraft'
import { useComposerLayerActive } from '@renderer/components/composer/ComposerContext'
import { excludeComposerDraftTokens } from '@renderer/components/composer/composerDraft'
import {
  type ComposerSurfaceActions,
  type ComposerSurfaceEditingState,
  type ComposerSurfaceProps
} from '@renderer/components/composer/ComposerSurface'
import {
  ComposerPinnedToolsProvider,
  ComposerToolDerivedStateProvider,
  ComposerToolRuntimeHost,
  ComposerToolRuntimeProvider,
  useComposerTokenReconcile,
  useComposerToolDispatch,
  useComposerToolLauncherActions,
  useComposerToolLauncherVersion,
  useComposerToolState
} from '@renderer/components/composer/ComposerToolRuntime'
import { HomeComposerInput } from '@renderer/components/composer/HomeComposerInput'
import type { InputHistoryDirection } from '@renderer/components/composer/inputHistoryNavigation'
import { QueuedFollowupsDock } from '@renderer/components/composer/QueuedFollowupsDock'
import {
  ComposerPanelSymbol,
  getQuickPanelSearchAliases,
  prepareComposerQuickPanelSearch
} from '@renderer/components/composer/quickPanel'
import type {
  ComposerDraftToken,
  ComposerSerializedDraft,
  ComposerSerializedToken
} from '@renderer/components/composer/tokens'
import type { ComposerToolFooterAction, ComposerToolLauncher } from '@renderer/components/composer/toolLauncher'
import { getComposerToolConfig } from '@renderer/components/composer/tools/registry'
import type { ToolContext } from '@renderer/components/composer/tools/types'
import { type FollowupQueueItem, useFollowupQueue } from '@renderer/components/composer/useFollowupQueue'
import { useInputHistory } from '@renderer/components/composer/useInputHistory'
import { useQueuedWorkbenchFiles } from '@renderer/components/composer/useQueuedWorkbenchFiles'
import { isPathWithinAccessiblePath } from '@renderer/components/composer/variants/agent/accessiblePath'
import {
  AgentConversationControls,
  type AgentConversationControlsProps,
  type AgentConversationWorkspace
} from '@renderer/components/composer/variants/agent/AgentConversationControls'
import {
  type AgentComposerDraftCache,
  type AgentComposerDraftCacheKey,
  getAgentDraftCacheKey,
  getAgentDraftTokens,
  getCachedSkillTokens,
  getSkillFromCachedToken,
  readAgentDraftCache,
  type RestoredAgentComposerDraftCache,
  writeAgentDraftCache
} from '@renderer/components/composer/variants/agent/agentDraftCache'
import { useAgentResourceMentionSource } from '@renderer/components/composer/variants/agent/useAgentResourceMentionSource'
import {
  agentComposerTokenId,
  agentFileToComposerToken,
  agentKnowledgeBaseToComposerToken,
  agentSkillToComposerToken,
  getAgentComposerTokenIds
} from '@renderer/components/composer/variants/agentComposerTokens'
import { createEditableMessageDraft } from '@renderer/components/composer/variants/chat/messageEditingDraft'
import {
  COMPOSER_TOOLBAR_CLASS,
  ComposerBelowControls,
  ComposerToolbarControls,
  ComposerToolMenuControls
} from '@renderer/components/composer/variants/shared/ComposerControlScaffolding'
import {
  emptyActions,
  type ProviderActionHandlers
} from '@renderer/components/composer/variants/shared/composerProviderActions'
import {
  buildComposerQueuedPayload,
  getComposerHistoryText
} from '@renderer/components/composer/variants/shared/composerQueuedPayload'
import { useComposerQuoteInsertion } from '@renderer/components/composer/variants/shared/composerQuote'
import { retainPromptFileTokens } from '@renderer/components/composer/variants/shared/composerTokens'
import {
  type ComposerToolbarCustomTool,
  ComposerToolbarShortcuts
} from '@renderer/components/composer/variants/shared/ComposerToolbarShortcuts'
import {
  agentFileExtensions,
  useComposerFileCapabilities
} from '@renderer/components/composer/variants/shared/useComposerFileCapabilities'
import { useComposerKnowledgeBaseScope } from '@renderer/components/composer/variants/shared/useComposerKnowledgeBaseScope'
import { useComposerSelectionReferenceInsertion } from '@renderer/components/composer/variants/shared/useComposerSelectionReferenceInsertion'
import { useComposerToolbarPinnedTools } from '@renderer/components/composer/variants/shared/useComposerToolbarPinnedTools'
import { useEntityReferenceMentionItems } from '@renderer/components/composer/variants/shared/useEntityReferenceMentionSource'
import { useLatest } from '@renderer/components/composer/variants/shared/useLatest'
import {
  createWorkspaceFileGroupToken,
  workspaceFileGroupLabel
} from '@renderer/components/composer/workspaceFileGroup'
import NewConversationIcon from '@renderer/components/icons/NewConversationIcon'
import { McpLogo } from '@renderer/components/icons/SvgIcon'
import {
  ModelSpeedControl,
  resolveSupportedReasoningEffort,
  resolveSupportedServiceTier
} from '@renderer/components/ModelSpeedControl'
import {
  type QuickPanelInputAdapter,
  type QuickPanelListItem,
  useOptionalQuickPanel
} from '@renderer/components/QuickPanel'
import {
  openResourceEditDialog,
  ResourceEditDialogEventHost
} from '@renderer/components/resourceCatalog/dialogs/ResourceEditDialogEventHost'
import { usePreference } from '@renderer/data/hooks/usePreference'
import { useUpdateAgent } from '@renderer/hooks/agent/useAgent'
import { useAgentModelDisabled, useAgentModelFilter } from '@renderer/hooks/agent/useAgentModelFilter'
import { useAgentSessionCompaction } from '@renderer/hooks/agent/useAgentSessionCompaction'
import { useAgentSessionContextUsage } from '@renderer/hooks/agent/useAgentSessionContextUsage'
import { useAgentSessionSlashCommands } from '@renderer/hooks/agent/useAgentSessionSlashCommands'
import { useUpdateSession } from '@renderer/hooks/agent/useSession'
import { useCommandHandler } from '@renderer/hooks/command'
import { useIsActiveTab } from '@renderer/hooks/tab'
import { useAlignedSessionPreface } from '@renderer/hooks/useAlignedSessionPreface'
import { useKnowledgeBases } from '@renderer/hooks/useKnowledgeBase'
import { useModelMutations, useModels } from '@renderer/hooks/useModel'
import { useAvailableSkills } from '@renderer/hooks/useSkills'
import { useTimer } from '@renderer/hooks/useTimer'
import { useTopicStreamStatus } from '@renderer/hooks/useTopicStreamStatus'
import { resolveAppLanguage } from '@renderer/i18n/languages'
import { ipcApi } from '@renderer/ipc'
import { agentIdFromMentionToken, agentWikiGuide } from '@renderer/pages/agents/agentWiki'
import { aixDeclarationFromUnknown, cloudBindingFromUnknown } from '@renderer/pages/agents/aixAgentRuntime'
import { HOME_COMPOSER_TEXT_HIGHLIGHTS } from '@renderer/pages/agents/homeSlashCatalog'
import {
  ensureReviewDocumentLine,
  reviewDraftNeedsSource,
  reviewFillFromText
} from '@renderer/pages/agents/reviewHandoff'
import {
  resolvePublishedAutoModel,
  useWorkbenchAgentRuntimeUnavailable,
  useWorkbenchModel
} from '@renderer/pages/agents/useWorkbenchDefaultAgent'
import {
  type WorkbenchComposerFill,
  registerWorkbenchComposer,
  sessionDraftWriteAllowed,
  settleWorkbenchComposer
} from '@renderer/pages/agents/workbenchComposerBridge'
import {
  beginHomeDraftHandoff,
  consumePendingHomeSubmit,
  finishHomeDraftHandoff,
  homeDraftHandoffActive,
  markPendingHomeSubmit,
  peekPendingHomeSubmit,
  WORKBENCH_COMPOSER_EDITOR_MIN_HEIGHT,
  WorkbenchComposerHostContext,
  WorkbenchHomeActiveContext,
  WorkbenchHomeTaskContext,
  WorkbenchProjectLockedContext,
  WorkbenchSessionHasMessagesContext
} from '@renderer/pages/agents/workbenchComposerHost'
import { homeTurnContext, researchBlocksSend } from '@renderer/pages/agents/workbenchHomeCapabilities'
import { displayedTaskPermissionMode } from '@renderer/pages/agents/workbenchPermissionPostures'
import { useWorkbenchHomeComposer } from '@renderer/pages/agents/WorkbenchHomeComposerExtras'
import { WorkbenchWorktreeSlot } from '@renderer/pages/agents/WorkbenchWorktreeSlot'
import {
  readPendingProject,
  selectWorkbenchProject,
  writePendingProject
} from '@renderer/pages/agents/WorkbenchProjectTray'
import {
  readPendingReasoningEffort,
  writePendingReasoningEffort
} from '@renderer/pages/agents/workbenchReasoningEffort'
import { selectedWorkspacePaths, type WorkspaceFileChoice } from '@renderer/pages/agents/WorkbenchWorkspaceFiles'
import { fileMentionRoot, useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings'
import { EVENT_NAMES, EventEmitter } from '@renderer/services/EventService'
import { toast } from '@renderer/services/toast'
import type { PermissionMode } from '@renderer/types/agent'
import type { ThinkingOption } from '@renderer/types/reasoning'
import { TopicType } from '@renderer/types/topic'
import { buildAgentFileWorkspaceKey, buildAgentSessionTopicId } from '@renderer/utils/agentSession'
import { agentHostContext } from '@renderer/utils/alignedSessionPreface'
import { buildFilePartsForAttachments, withComposerFilePartMeta } from '@renderer/utils/file/buildFileParts'
import {
  getComposerShortcutLabel,
  resolveNewlineShortcut,
  resolveSendShortcut,
  resolveSteerShortcut
} from '@renderer/utils/input'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { resolveReasoningEffortForModel } from '@renderer/utils/model'
import type { ComposerQueuedMessagePayload } from '@shared/ai/transport'
import type { AgentEntity } from '@shared/data/types/agent'
import type { KnowledgeBase } from '@shared/data/types/knowledge'
import type { CherryMessagePart, FileUIPart } from '@shared/data/types/message'
import type { Model, ServiceTierSelection } from '@shared/data/types/model'
import { getKnowledgeBaseIdsFromParts, withKnowledgeScopePart } from '@shared/data/types/uiParts'
import type { OutputFor } from '@shared/ipc/types'
import { type AbsoluteFilePath, AbsoluteFilePathSchema } from '@shared/types/file'
import { reviewTextSourceSchema } from '@shared/types/reviewSource'
import type { LocalSkill } from '@shared/types/skill'
import { type CanonicalFilePath, canonicalizeFilePath, createFilePathHandle, toFileUrl } from '@shared/utils/file'

import { writeCodeSession } from './codeMode'
import {
  type HomeComposerCodePlan,
  type HomeComposerDeck,
  noteHomeComposerTopic,
  requestHomeComposerSend,
  useHomeComposerModeState
} from './homeComposerMode'
import { openPaperloom } from './paperloom/paperloomApi'
import { paperloomDraftQuestion, samePaperloomDraft } from './paperloom/paperloomHandoff'
import { codeWorkspaceErrorText, placeCodeSend } from './prepareCodeWorkspace'
import { armPresentationDeck } from './presentationDeck'

const logger = loggerService.withContext('AgentComposer')

const AGENT_MANAGED_TOKEN_KINDS = [
  'file',
  'knowledge',
  'skill'
] as const satisfies readonly ComposerDraftToken['kind'][]
const AGENT_MANAGED_TOKEN_KINDS_BEFORE_KNOWLEDGE_RESTORE = [
  'file',
  'skill'
] as const satisfies readonly ComposerDraftToken['kind'][]
const AGENT_SKILLS_LAUNCHER_ID = 'agent-skills'
const AGENT_NEW_SESSION_TOOL_ID = 'composer:new-session'
const EMPTY_ACCESSIBLE_PATHS: readonly AbsoluteFilePath[] = []
const FILE_IPC_BATCH_SIZE = 500

type AccessibleAttachment = {
  attachment: ComposerAttachment
  filePath: CanonicalFilePath
  index: number
}

const requestAccessiblePathMetadata = async (
  attachments: readonly AccessibleAttachment[]
): Promise<OutputFor<'file.batch_get_metadata'>> => {
  if (attachments.length === 0) return {}

  const chunks: AccessibleAttachment[][] = []
  for (let i = 0; i < attachments.length; i += FILE_IPC_BATCH_SIZE) {
    chunks.push(attachments.slice(i, i + FILE_IPC_BATCH_SIZE))
  }

  const results = await Promise.all(
    chunks.map((chunk) =>
      ipcApi.request('file.batch_get_metadata', {
        items: chunk.map(({ filePath }) => ({
          key: filePath,
          handle: createFilePathHandle(filePath)
        }))
      })
    )
  )

  return Object.assign({}, ...results)
}

const buildAccessiblePathFilePart = (
  attachment: ComposerAttachment,
  filePath: CanonicalFilePath,
  metadataByPath: OutputFor<'file.batch_get_metadata'>
): FileUIPart => {
  const metadata = metadataByPath[filePath]
  if (!metadata || metadata.kind !== 'file') throw new Error('Workspace file is no longer available')

  return withComposerFilePartMeta(
    {
      type: 'file',
      url: toFileUrl(filePath),
      mediaType: metadata.mime,
      filename: attachment.origin_name || attachment.name
    },
    attachment
  )
}

/**
 * `AgentWorkspacePathSchema` only guarantees a non-empty string, so the stored
 * workspace path is asserted to be an absolute filesystem path here, before it
 * reaches the path helpers. A malformed one yields no accessible paths, which
 * degrades to inlining attachments instead of referencing them — still correct,
 * just less efficient.
 */
const toAccessiblePaths = (workspacePath: string | undefined): AbsoluteFilePath[] => {
  if (!workspacePath) return []
  const parsed = AbsoluteFilePathSchema.safeParse(workspacePath)
  if (!parsed.success) {
    logger.warn('Ignoring agent workspace path that is not an absolute filesystem path', { path: workspacePath })
    return []
  }
  return [parsed.data]
}

const buildAgentFilePartsForAttachments = async (
  attachments: ComposerAttachment[],
  accessiblePaths: readonly AbsoluteFilePath[]
): Promise<FileUIPart[]> => {
  const accessibleAttachments: AccessibleAttachment[] = []
  const internalizedAttachments: ComposerAttachment[] = []
  const internalizedIndexes: number[] = []

  attachments.forEach((attachment, index) => {
    // A path-less attachment (message-editing round-trip) cannot be matched
    // against the workspace, so it takes the internalized branch — the same
    // outcome it already had when its non-path value failed the match.
    if (attachment.path && isPathWithinAccessiblePath(attachment.path, accessiblePaths)) {
      accessibleAttachments.push({
        attachment,
        filePath: canonicalizeFilePath(attachment.path),
        index
      })
      return
    }

    internalizedAttachments.push(attachment)
    internalizedIndexes.push(index)
  })

  const [metadataByPath, internalizedFileParts] = await Promise.all([
    requestAccessiblePathMetadata(accessibleAttachments),
    buildFilePartsForAttachments(internalizedAttachments)
  ])

  const fileParts = new Array<FileUIPart>(attachments.length)

  accessibleAttachments.forEach(({ attachment, filePath, index }) => {
    fileParts[index] = buildAccessiblePathFilePart(attachment, filePath, metadataByPath)
  })

  internalizedFileParts.forEach((filePart, offset) => {
    const originalIndex = internalizedIndexes[offset]
    if (originalIndex === undefined || !filePart) {
      throw new Error(`Failed to build file part for attachment: ${internalizedAttachments[offset]?.path ?? ''}`)
    }
    fileParts[originalIndex] = filePart
  })

  return fileParts
}

const createSkillQuickPanelItems = (
  skills: readonly LocalSkill[],
  options: {
    skillLabel: string
    onInsertSkill: (skill: LocalSkill, inputAdapter?: QuickPanelInputAdapter) => void
  }
): QuickPanelListItem[] => {
  return skills.map((skill) => ({
    id: agentComposerTokenId.skill(skill),
    label: skill.name,
    description: skill.description ?? undefined,
    icon: <ToolCase size={16} />,
    // Skills still exclude descriptions from root-panel search; the category alias powers the persistent shortcut.
    filterText: skill.name,
    searchAliases: [options.skillLabel],
    action: ({ inputAdapter }) => {
      options.onInsertSkill(skill, inputAdapter)
    }
  }))
}

type AgentComposerSessionSnapshot = {
  workspace?: AgentConversationWorkspace | null
  workspaceId?: string | null
}

export interface AgentComposerSendBody {
  agentId: string
  sessionId: string
  userMessageParts: ComposerQueuedMessagePayload['userMessageParts']
  reasoningEffort?: ThinkingOption
  serviceTier?: ServiceTierSelection
  fastMode?: boolean
  hostContext?: ComposerQueuedMessagePayload['hostContext']
  turnContext?: ComposerQueuedMessagePayload['turnContext']
  /** Host-produced AIX text. The composer does not set this. */
  aixOutput?: string
  permissionMode?: 'default' | 'plan' | 'auto' | 'bypassPermissions'
}

export type AgentComposerSendOptions = { body?: AgentComposerSendBody }

export interface AgentComposerLaunchOptions {
  initialDraft: Pick<AgentComposerDraftCache, 'text' | 'tokens'>
  initialParts?: CherryMessagePart[]
  editing?: ComposerSurfaceEditingState
  onSent?: () => void
}

type Props = {
  agentId: string
  sessionId: string
  sessionOverride: AgentComposerSessionSnapshot
  resolvedAgent: AgentEntity | undefined
  resolvedModel: Model | undefined
  resolvedWorkspaceWarning: string | null
  externalContextControls?: boolean
  sendMessage: (message?: { text: string }, options?: AgentComposerSendOptions) => Promise<boolean | void>
  stop: () => Promise<void>
  onCreateEmptySession?: () => void | Promise<unknown>
  onAgentChange?: (agentId: string | null) => void | Promise<void>
  agentChanging?: boolean
  canChangeAgent?: boolean
  workspaceId?: string | null
  onWorkspaceChange?: (workspaceId: string | null) => void | Promise<void>
  workspaceChanging?: boolean
  canChangeModel?: boolean
  isStreaming: boolean
  sendDisabled?: boolean
  compactWhenSingleLine?: boolean
  launchOptions?: AgentComposerLaunchOptions
}

type AgentComposerRootProps = Props & {
  renderControls: AgentComposerControlsRenderer
  forceNarrowLayout?: boolean
  deferQuickPanel?: boolean
}

function withReviewDocumentLine(
  payload: ComposerQueuedMessagePayload,
  mentionId: string | undefined,
  tokens: readonly ComposerSerializedToken[]
): ComposerQueuedMessagePayload {
  if (payload.turnContext?.reviewSource) return payload
  const paths = (payload.attachments ?? []).flatMap((file) => (typeof file.path === 'string' ? [file.path] : []))
  const text = ensureReviewDocumentLine(payload.text, mentionId, paths, tokens)
  if (text === payload.text) return payload
  return {
    ...payload,
    text,
    userMessageParts: payload.userMessageParts.map((part) =>
      part.type === 'text' && part.text === payload.text ? { ...part, text } : part
    )
  }
}

const AgentComposerRoot = ({
  agentId,
  sessionId,
  sessionOverride,
  resolvedAgent,
  resolvedModel,
  resolvedWorkspaceWarning,
  sendMessage,
  stop,
  onCreateEmptySession,
  onAgentChange,
  agentChanging,
  canChangeAgent = false,
  workspaceId,
  onWorkspaceChange,
  workspaceChanging,
  canChangeModel = true,
  isStreaming,
  sendDisabled = false,
  compactWhenSingleLine = false,
  launchOptions,
  renderControls,
  forceNarrowLayout = false,
  deferQuickPanel = false
}: AgentComposerRootProps) => {
  const session = sessionOverride
  const agent = resolvedAgent
  const sessionModel = resolvedModel
  const actionsRef = useRef<ProviderActionHandlers>({ ...emptyActions })
  // The seed sticks to its session so clearing `launchOptions` on send does not re-key the
  // composer mid-send, and it is consumed once: a later scope change (switching the session
  // workspace) must not re-inject the template over what the user has already written.
  const launchIdentityRef = useRef({ sessionId, initialDraft: launchOptions?.initialDraft, consumed: false })
  if (
    launchIdentityRef.current.sessionId !== sessionId ||
    (launchOptions && launchOptions.initialDraft !== launchIdentityRef.current.initialDraft)
  ) {
    launchIdentityRef.current = { sessionId, initialDraft: launchOptions?.initialDraft, consumed: false }
  }
  const launchInitialDraft = launchIdentityRef.current.initialDraft
  // Persistence follows the live launch options, not the sticky seed: once the launch message is
  // sent the session owns its draft again, so follow-ups are cached like any other session's.
  const draftPersistenceEnabled = launchOptions === undefined
  const composerInstanceKey = `${sessionId}:${launchInitialDraft === undefined ? 'default' : 'launch'}`
  const resolvedWorkspaceId = workspaceId ?? session?.workspaceId ?? null
  const workspaceKey = buildAgentFileWorkspaceKey(resolvedWorkspaceId, session?.workspace?.path)
  const draftCacheKey = getAgentDraftCacheKey(sessionId)
  const initialDraftRef = useRef<{
    instanceKey: string
    draft: RestoredAgentComposerDraftCache
  } | null>(null)
  const scopedComposerInstanceKey = `${composerInstanceKey}:${workspaceKey}`
  if (initialDraftRef.current?.instanceKey !== scopedComposerInstanceKey) {
    let draft: RestoredAgentComposerDraftCache
    if (launchInitialDraft === undefined) {
      draft = readAgentDraftCache(draftCacheKey, { workspaceKey, agentId })
    } else {
      // A launch draft is never cached, so a scope change (switching the session workspace)
      // has to carry the live edits over itself — re-seeding the template would silently
      // discard whatever the user has written into it. Files stay behind: they belong to the
      // workspace being left.
      const seed = launchIdentityRef.current.consumed ? actionsRef.current.getDraft() : launchInitialDraft
      const edited =
        launchOptions?.initialParts && launchOptions.editing
          ? createEditableMessageDraft(launchOptions.initialParts, launchOptions.editing.messageId)
          : undefined
      launchIdentityRef.current.consumed = true
      draft = {
        text: edited?.text ?? seed.text,
        tokens: edited?.draftTokens ?? [...seed.tokens],
        files: edited?.files ?? [],
        knowledgeBaseIds: getKnowledgeBaseIdsFromParts(launchOptions?.initialParts ?? []) ?? [],
        workspaceKey,
        agentId,
        shouldValidateSkills: false
      }
    }
    initialDraftRef.current = { instanceKey: scopedComposerInstanceKey, draft }
  }
  const { draft: initialDraft } = initialDraftRef.current
  const handleNewSessionShortcut = useCallback(() => {
    void onCreateEmptySession?.()
  }, [onCreateEmptySession])
  const hasNewSessionShortcutAction = Boolean(onCreateEmptySession)

  const isActiveTab = useIsActiveTab()
  useCommandHandler('topic.create', handleNewSessionShortcut, {
    enabled: isActiveTab && Boolean(session && agent && hasNewSessionShortcutAction)
  })

  const sessionSlashCommands = useAgentSessionSlashCommands(sessionId)
  const sessionData = useMemo(() => {
    if (!session || !agent) return undefined
    const accessiblePaths = toAccessiblePaths(session.workspace?.type === 'user' ? session.workspace.path : undefined)
    return {
      agentId,
      sessionId,
      agentType: agent.type,
      accessiblePaths,
      slashCommands: sessionSlashCommands,
      knowledgeBaseIds: agent.knowledgeBaseIds ?? []
    }
  }, [session, agent, agentId, sessionId, sessionSlashCommands])

  const initialState = useMemo(
    () => ({
      mentionedModels: [],
      selectedKnowledgeBases: [],
      files: initialDraft.files,
      isExpanded: false,
      couldAddImageFile: false,
      extensions: [] as string[]
    }),
    [initialDraft]
  )

  return (
    <ComposerToolRuntimeProvider
      key={scopedComposerInstanceKey}
      initialState={initialState}
      actions={{
        onTextChange: (updater) => actionsRef.current.onTextChange(updater),
        addNewTopic: () => {
          void onCreateEmptySession?.()
        }
      }}>
      <AgentComposerInner
        key={scopedComposerInstanceKey}
        agent={agent}
        model={sessionModel}
        modelPending={!resolvedModel && sendDisabled}
        agentId={agentId}
        sessionId={sessionId}
        initialDraft={initialDraft}
        draftCacheKey={draftCacheKey}
        draftPersistenceEnabled={draftPersistenceEnabled}
        workspaceKey={workspaceKey}
        sessionData={sessionData}
        workspace={session?.workspace ?? null}
        workspaceId={resolvedWorkspaceId}
        actionsRef={actionsRef}
        chatSendMessage={sendMessage}
        chatStop={stop}
        onCreateEmptySession={onCreateEmptySession}
        onAgentChange={onAgentChange}
        agentChanging={agentChanging}
        canChangeAgent={canChangeAgent}
        onWorkspaceChange={onWorkspaceChange}
        workspaceChanging={workspaceChanging}
        canChangeModel={canChangeModel}
        isStreaming={isStreaming}
        sendDisabled={sendDisabled}
        compactWhenSingleLine={compactWhenSingleLine}
        launchOptions={launchOptions}
        renderControls={renderControls}
        forceNarrowLayout={forceNarrowLayout}
        deferQuickPanel={deferQuickPanel}
        resolvedWorkspaceWarning={resolvedWorkspaceWarning}
      />
    </ComposerToolRuntimeProvider>
  )
}

/** Managed picks parked while an input-history entry is previewed, restored when the preview exits. */
interface InputHistoryToolSnapshot {
  files: ComposerAttachment[]
  selectedKnowledgeBases: KnowledgeBase[]
}

interface InnerProps {
  agent?: AgentEntity
  model?: Model
  modelPending?: boolean
  agentId: string
  sessionId: string
  initialDraft: RestoredAgentComposerDraftCache
  draftCacheKey: AgentComposerDraftCacheKey
  draftPersistenceEnabled: boolean
  workspaceKey: string
  sessionData?: ToolContext['session']
  workspace?: AgentConversationWorkspace | null
  workspaceId?: string | null
  actionsRef: React.MutableRefObject<ProviderActionHandlers>
  chatSendMessage: Props['sendMessage']
  chatStop: Props['stop']
  onCreateEmptySession?: Props['onCreateEmptySession']
  onAgentChange?: Props['onAgentChange']
  agentChanging?: boolean
  canChangeAgent: boolean
  onWorkspaceChange?: Props['onWorkspaceChange']
  workspaceChanging?: boolean
  canChangeModel: boolean
  isStreaming: boolean
  sendDisabled: boolean
  compactWhenSingleLine: boolean
  launchOptions?: AgentComposerLaunchOptions
  renderControls: AgentComposerControlsRenderer
  forceNarrowLayout?: boolean
  deferQuickPanel?: boolean
  resolvedWorkspaceWarning: string | null
}

export function AgentComposerContextUsage({ model, sessionId }: { model?: Model; sessionId: string }) {
  const { t } = useTranslation()
  const { percentage, usage, maxTokens } = useAgentSessionContextUsage(sessionId, model)
  const compaction = useAgentSessionCompaction(sessionId)
  if (percentage === null || !usage) return null

  const isCompacting = compaction.status === 'compacting'
  const label = t('agent.right_pane.info.context_usage')

  return (
    <Tooltip
      placement="top"
      sideOffset={8}
      showArrow={false}
      classNames={{
        placeholder: 'inline-grid',
        content:
          'w-64 max-w-64 rounded-md border border-border bg-card p-3 text-card-foreground shadow-md dark:bg-card dark:text-card-foreground'
      }}
      content={
        <AgentContextUsageSummary
          usage={usage}
          percentage={percentage}
          maxTokens={maxTokens}
          isCompacting={isCompacting}
          modelName={model?.name}
          showCategories={false}
        />
      }>
      <ContextUsageMeter
        label={label}
        percentage={percentage}
        isBusy={isCompacting}
        // The cached reading is only refreshed when a turn settles, so it goes stale mid-turn. Main
        // throttles this and answers on the shared-cache key; a session with no live connection
        // keeps showing its last reading.
        onPointerEnter={() => {
          void ipcApi.request('ai.agent.session.refresh_context_usage', { sessionId })
        }}
      />
    </Tooltip>
  )
}

type AgentComposerControlProps = Omit<
  AgentConversationControlsProps,
  'side' | 'iconOnly' | 'onAgentDialogCloseAutoFocus'
> & {
  topBarPortalAvailable: boolean
  topBarPortalIconOnly: boolean
  leadingControl?: React.ReactNode
  trailingControl?: React.ReactNode
  renderQuickPanelShortcuts?: (args: {
    inputAdapter?: AgentComposerInputAdapter
    unifiedPanelControl?: AgentComposerUnifiedPanelControl
  }) => React.ReactNode
}
type AgentComposerControlSlots = Pick<
  ComposerSurfaceProps,
  'renderLeftControls' | 'renderBelowControls' | 'renderCompactControls'
>
type AgentComposerControlsRenderer = (props: AgentComposerControlProps) => AgentComposerControlSlots

type AgentComposerInputAdapter = Parameters<NonNullable<ComposerSurfaceProps['renderLeftControls']>>[0]
type AgentComposerUnifiedPanelControl = Parameters<NonNullable<ComposerSurfaceProps['renderLeftControls']>>[1]

const restoreAgentComposerInputFocus = (inputAdapter: AgentComposerInputAdapter) => {
  window.requestAnimationFrame(() => inputAdapter?.focus())
}

const AgentConversationControlsWithAutoFocus = ({
  inputAdapter,
  ...props
}: AgentConversationControlsProps & { inputAdapter: AgentComposerInputAdapter }) => {
  const onAgentDialogCloseAutoFocus = useCallback(() => restoreAgentComposerInputFocus(inputAdapter), [inputAdapter])

  return <AgentConversationControls {...props} onAgentDialogCloseAutoFocus={onAgentDialogCloseAutoFocus} />
}

const renderAgentComposerContextControls = (
  props: AgentComposerControlProps,
  inputAdapter: AgentComposerInputAdapter,
  { side, iconOnly }: { side: 'top' | 'bottom'; iconOnly: boolean }
) => {
  const resolvedSide = props.topBarPortalAvailable ? 'bottom' : side
  const resolvedIconOnly = props.topBarPortalAvailable ? props.topBarPortalIconOnly : iconOnly
  const controls = (
    <AgentConversationControlsWithAutoFocus
      {...props}
      side={resolvedSide}
      iconOnly={resolvedIconOnly}
      inputAdapter={inputAdapter}
    />
  )

  return props.topBarPortalAvailable ? <ConversationTopBarPortal>{controls}</ConversationTopBarPortal> : controls
}

const renderAgentToolbarControls: AgentComposerControlsRenderer = (props) => {
  return {
    renderLeftControls: (inputAdapter, unifiedPanelControl) => {
      const quickPanelShortcuts = props.renderQuickPanelShortcuts?.({ inputAdapter, unifiedPanelControl })

      return (
        <ComposerToolbarControls
          inputAdapter={inputAdapter}
          leading={
            <>
              {props.leadingControl}
              {quickPanelShortcuts}
              {props.trailingControl}
            </>
          }
          unifiedPanelControl={unifiedPanelControl}
          renderContextControls={({ side, iconOnly }) =>
            renderAgentComposerContextControls(props, inputAdapter, { side, iconOnly })
          }
        />
      )
    },
    renderCompactControls: (inputAdapter, unifiedPanelControl) => (
      <>
        {props.topBarPortalAvailable
          ? renderAgentComposerContextControls(props, inputAdapter, {
              side: 'bottom',
              iconOnly: props.topBarPortalIconOnly
            })
          : null}
        {props.renderQuickPanelShortcuts?.({ inputAdapter, unifiedPanelControl })}
        {props.trailingControl}
      </>
    )
  }
}

const renderAgentInputControls: AgentComposerControlsRenderer = (props) => {
  return {
    renderLeftControls: (inputAdapter, unifiedPanelControl) => (
      <ComposerToolbarControls
        inputAdapter={inputAdapter}
        leading={
          <>
            {props.leadingControl}
            {props.renderQuickPanelShortcuts?.({ inputAdapter, unifiedPanelControl })}
            {props.trailingControl}
          </>
        }
        unifiedPanelControl={unifiedPanelControl}
        renderContextControls={() => null}
      />
    ),
    renderCompactControls: (inputAdapter, unifiedPanelControl) => (
      <>
        {props.renderQuickPanelShortcuts?.({ inputAdapter, unifiedPanelControl })}
        {props.trailingControl}
      </>
    )
  }
}

const renderAgentHomeControls: AgentComposerControlsRenderer = (props) => {
  return {
    renderLeftControls: (inputAdapter, unifiedPanelControl) => {
      const quickPanelShortcuts = props.renderQuickPanelShortcuts?.({ inputAdapter, unifiedPanelControl })

      return (
        <>
          {props.topBarPortalAvailable
            ? renderAgentComposerContextControls(props, inputAdapter, {
                side: 'bottom',
                iconOnly: false
              })
            : null}
          <div className={COMPOSER_TOOLBAR_CLASS}>
            {props.leadingControl}
            {quickPanelShortcuts}
            <ComposerToolMenuControls inputAdapter={inputAdapter} unifiedPanelControl={unifiedPanelControl} />
          </div>
        </>
      )
    },
    renderBelowControls: props.topBarPortalAvailable
      ? undefined
      : (inputAdapter) => (
          <ComposerBelowControls
            renderContextControls={({ side, iconOnly }) =>
              renderAgentComposerContextControls(props, inputAdapter, { side, iconOnly })
            }
          />
        )
  }
}

const AgentComposerInner = ({
  agent,
  model,
  modelPending,
  agentId,
  sessionId,
  initialDraft,
  draftCacheKey,
  draftPersistenceEnabled,
  workspaceKey,
  sessionData,
  workspace,
  workspaceId,
  actionsRef,
  chatSendMessage,
  chatStop,
  onCreateEmptySession,
  onAgentChange,
  agentChanging,
  canChangeAgent,
  onWorkspaceChange,
  workspaceChanging,
  canChangeModel,
  isStreaming,
  sendDisabled,
  compactWhenSingleLine,
  launchOptions,
  renderControls,
  forceNarrowLayout = false,
  deferQuickPanel = false,
  resolvedWorkspaceWarning
}: InnerProps) => {
  const workbenchComposerHost = use(WorkbenchComposerHostContext)
  const homeActive = use(WorkbenchHomeActiveContext)
  const homeActiveRef = useRef(homeActive)
  homeActiveRef.current = homeActive
  const composerMode = useHomeComposerModeState()
  const projectLocked = use(WorkbenchProjectLockedContext)
  const sessionHasMessages = use(WorkbenchSessionHasMessagesContext)
  const homeTask = use(WorkbenchHomeTaskContext)
  const homeDraftActive = homeActive && !launchOptions?.editing && !homeDraftHandoffActive(sessionId)
  const composerStreaming = isStreaming && !homeDraftActive
  const workbenchSurface = Boolean(workbenchComposerHost) || projectLocked
  const conversationFrame = projectLocked && !workbenchComposerHost
  const [conversationScope, setConversationScope] = useState<HTMLElement | null>(null)
  const { updateAgent, updateModel } = useUpdateAgent()
  const [defaultModelId] = usePreference('chat.default_model_id')
  const autoModel = workbenchSurface && !defaultModelId
  const { models: catalogModels } = useModels(undefined, { fetchEnabled: autoModel })
  const { createModel } = useModelMutations()
  const { updateSession } = useUpdateSession()
  const scope = TopicType.Session
  const config = getComposerToolConfig(scope)
  const { files, isExpanded, selectedKnowledgeBases } = useComposerToolState()
  const { setFiles, setIsExpanded, setSelectedKnowledgeBases, toolsRegistry } = useComposerToolDispatch()
  const { getLaunchers, dispatchLauncher } = useComposerToolLauncherActions()
  const toolLaunchersVersion = useComposerToolLauncherVersion()
  const [enableSpellCheck] = usePreference('app.spell_check.enabled')
  const [fontSize] = usePreference('chat.message.font_size')
  const [narrowMode] = usePreference('chat.narrow_mode')
  const appWorkspacePath = useDisplayedWorkspacePath()
  const sessionPreface = useAlignedSessionPreface()
  // Yield the same rail gutter as the message column so the composer stays aligned.
  const { railGutterPx } = useChatLayoutMode()
  const [storedSendShortcut] = usePreference('chat.input.send_message_shortcut')
  const sendMessageShortcut = resolveSendShortcut(storedSendShortcut)
  const [steerShortcut] = usePreference('chat.input.steer_shortcut')
  const [newlineShortcut] = usePreference('chat.input.newline_shortcut')
  const { available: topBarPortalAvailable, iconOnly: topBarPortalIconOnly } = useConversationTopBarPortalLayout()
  const {
    pinnedIds: pinnedToolIds,
    setPinnedIds: setPinnedToolIds,
    resetPinnedIds: resetPinnedToolIds,
    isDefault: pinnedToolsAtDefault,
    customizeOpen: customizeToolbarOpen,
    setCustomizeOpen: setCustomizeToolbarOpen
  } = useComposerToolbarPinnedTools('agent.input.toolbar.pinned_tools')
  const { t, i18n } = useTranslation()
  const agentModelFilter = useAgentModelFilter(agent?.type)
  const isModelDisabled = useAgentModelDisabled()
  const isModelUnavailable = Boolean(agent) && !model && !modelPending
  const missingModelMessage = isModelUnavailable ? t('code.model_required') : undefined
  const { setTimeoutTimer, clearTimeoutTimer } = useTimer()
  const pinnedLauncherIds = useMemo(
    () => pinnedToolIds.map((id) => (id === 'skills' ? AGENT_SKILLS_LAUNCHER_ID : id)),
    [pinnedToolIds]
  )
  const configuredReasoningEffort = agent?.configuration?.reasoning_effort ?? 'default'
  const canonicalReasoningEffort = model
    ? (resolveReasoningEffortForModel(model, configuredReasoningEffort) ?? 'default')
    : configuredReasoningEffort
  const [reasoningOverride, setReasoningOverride] = useState<{
    agentId: string
    value: ThinkingOption
    version: number
    canonicalAtMutationStart?: ThinkingOption
  } | null>(null)
  const reasoningMutationVersionRef = useRef(0)
  const pendingReasoningEditRef = useRef<{
    agentId: string
    version: number
    effort: ThinkingOption
  } | null>(null)
  const activeReasoningOverride =
    reasoningOverride &&
    reasoningOverride.agentId === agent?.id &&
    (reasoningOverride.canonicalAtMutationStart === undefined ||
      reasoningOverride.canonicalAtMutationStart === canonicalReasoningEffort)
      ? reasoningOverride
      : null
  useEffect(() => {
    if (
      !reasoningOverride ||
      reasoningOverride.agentId !== agent?.id ||
      reasoningOverride.canonicalAtMutationStart === undefined ||
      reasoningOverride.canonicalAtMutationStart === canonicalReasoningEffort
    ) {
      return
    }
    setReasoningOverride((current) => (current === reasoningOverride ? null : current))
  }, [agent?.id, canonicalReasoningEffort, reasoningOverride])
  const memberId = useMemberProfile()?.id ?? null
  const [composerEffort, setComposerEffort] = useState<ThinkingOption | null>(() => readPendingReasoningEffort())
  useEffect(() => {
    if (!workbenchSurface) return
    setComposerEffort(readPendingReasoningEffort(undefined, memberId))
  }, [memberId, workbenchSurface])
  const reasoningEffort =
    activeReasoningOverride?.value ?? (workbenchSurface && composerEffort ? composerEffort : canonicalReasoningEffort)
  const { model: rememberedModel } = useWorkbenchModel({ enabled: workbenchSurface })
  const syncedModelId = useRef<string | null>(null)
  const canonicalServiceTier = agent?.configuration?.service_tier ?? 'standard'
  const [serviceTierOverride, setServiceTierOverride] = useState<{
    agentId: string
    value: ServiceTierSelection
    version: number
  } | null>(null)
  const serviceTierMutationVersionRef = useRef(0)
  const activeServiceTierOverride = serviceTierOverride?.agentId === agent?.id ? serviceTierOverride : null
  const serviceTier = activeServiceTierOverride?.value ?? canonicalServiceTier
  const [fastMode, setFastMode] = useState(false)
  const [selectedSkills, setSelectedSkills] = useState<LocalSkill[]>(() =>
    getCachedSkillTokens(initialDraft.tokens).map(getSkillFromCachedToken)
  )
  const [shouldValidateSkills, setShouldValidateSkills] = useState(initialDraft.shouldValidateSkills)
  const [text, setTextState] = useState(() => initialDraft.text)
  const [draftTokens, setDraftTokens] = useState<ComposerSerializedToken[]>(() => initialDraft.tokens)
  const [isDirectSending, setIsDirectSending] = useState(false)
  const directSendInFlightRef = useRef(false)
  const draftTokensRef = useRef(draftTokens)
  const knowledgeBaseIdsRef = useRef([...initialDraft.knowledgeBaseIds])
  const observedKnowledgeBaseSelectionKeyRef = useRef<string | null>(
    initialDraft.knowledgeBaseIds.length === 0 ? JSON.stringify([]) : null
  )
  const [isKnowledgeBaseDraftHydrated, setIsKnowledgeBaseDraftHydrated] = useState(
    initialDraft.knowledgeBaseIds.length === 0
  )
  const sessionTopicId = buildAgentSessionTopicId(sessionId)
  const layerActive = useComposerLayerActive()
  const accessiblePaths = sessionData?.accessiblePaths ?? EMPTY_ACCESSIBLE_PATHS
  const enableResourceMention = accessiblePaths.length > 0
  const userWorkspacePath = workspace?.type === 'user' ? workspace.path : undefined
  const workspaceWarning = resolvedWorkspaceWarning ?? undefined
  const quickPanel = useOptionalQuickPanel()
  const rootPanelVisible =
    layerActive && Boolean(quickPanel?.isVisible && quickPanel.symbol === ComposerPanelSymbol.Root)
  const skillsPanelVisible =
    layerActive && Boolean(quickPanel?.isVisible && quickPanel.symbol === AGENT_SKILLS_LAUNCHER_ID)
  const knowledgeBasePanelVisible = Boolean(
    layerActive && quickPanel?.isVisible && quickPanel.symbol === ComposerPanelSymbol.KnowledgeBase
  )
  const skillsDataEnabled =
    workbenchSurface ||
    selectedSkills.length > 0 ||
    getAgentComposerTokenIds(draftTokens, 'skill').size > 0 ||
    rootPanelVisible ||
    skillsPanelVisible
  const knowledgeBasesDataEnabled =
    selectedKnowledgeBases.length > 0 ||
    getAgentComposerTokenIds(draftTokens, 'knowledge').size > 0 ||
    rootPanelVisible ||
    knowledgeBasePanelVisible
  const {
    skills: availableSkills,
    loading: isAvailableSkillsLoading,
    error: availableSkillsError,
    refresh: refreshAvailableSkills
  } = useAvailableSkills(agentId, userWorkspacePath, { enabled: skillsDataEnabled })
  const skillByFilename = useMemo(
    () => new Map(availableSkills.map((skill) => [skill.filename, skill])),
    [availableSkills]
  )
  const { bases: allKnowledgeBases, isLoading: isKnowledgeBasesLoading } = useKnowledgeBases({
    enabled: knowledgeBasesDataEnabled
  })

  const { canAddImageFile, supportedExts: modelSupportedExts } = useComposerFileCapabilities(model)
  // A missing model blocks send. It must not block staging a file the user already dropped.
  const supportedExts = workbenchSurface && modelSupportedExts.length === 0 ? agentFileExtensions() : modelSupportedExts
  useQueuedWorkbenchFiles({
    enabled: Boolean(workbenchComposerHost),
    supportedExts,
    setFiles
  })

  useEffect(() => {
    if (model?.supportsFastMode !== true) setFastMode(false)
  }, [model?.supportsFastMode])

  const setText = useCallback(
    (nextText: string) => {
      clearTimeoutTimer('agentComposerSendMessage')
      setTextState(nextText)
    },
    [clearTimeoutTimer]
  )
  useEffect(() => {
    if (!homeActive) return
    noteHomeComposerTopic(text)
  }, [homeActive, text])

  useEffect(() => {
    if (!shouldValidateSkills || isAvailableSkillsLoading || availableSkillsError) return

    const draft = { text, tokens: draftTokens }
    const validatedDraft = excludeComposerDraftTokens(draft, (token) => {
      if (token.kind !== 'skill') return false
      return !skillByFilename.has(getSkillFromCachedToken(token).filename)
    })
    if (validatedDraft !== draft) {
      actionsRef.current.replaceDraft(validatedDraft)
      setText(validatedDraft.text)
      setDraftTokens(validatedDraft.tokens)
      draftTokensRef.current = validatedDraft.tokens
    }
    setSelectedSkills(
      getCachedSkillTokens(validatedDraft.tokens).flatMap((token) => {
        const skill = skillByFilename.get(getSkillFromCachedToken(token).filename)
        return skill ? [skill] : []
      })
    )
    setShouldValidateSkills(false)
  }, [
    actionsRef,
    availableSkillsError,
    draftTokens,
    isAvailableSkillsLoading,
    setText,
    shouldValidateSkills,
    skillByFilename,
    text
  ])
  const filesRef = useLatest(files)
  const selectedKnowledgeBasesRef = useLatest(selectedKnowledgeBases)
  const inputHistoryToolsRef = useRef<InputHistoryToolSnapshot | null>(null)
  const applyHistoryDraft = useCallback(
    (historyDraft: ComposerSerializedDraft, options: { source: 'history' | 'draft' }) => {
      const nextDraftTokens = getAgentDraftTokens(historyDraft.tokens)
      actionsRef.current.replaceDraft(historyDraft)
      setText(historyDraft.text)
      setDraftTokens(nextDraftTokens)
      draftTokensRef.current = nextDraftTokens
      setSelectedSkills(getCachedSkillTokens(nextDraftTokens).map(getSkillFromCachedToken))

      if (options.source === 'history') {
        // A recalled entry is plain text with no tokens, so every managed pick steps aside — skills
        // and files already do. A live knowledge selection would re-insert its chip on top of the
        // sentence the entry already carries, sending the same attachment claim twice.
        inputHistoryToolsRef.current ??= {
          files: filesRef.current,
          selectedKnowledgeBases: selectedKnowledgeBasesRef.current
        }
        setFiles([])
        setSelectedKnowledgeBases([])
        return
      }

      const savedTools = inputHistoryToolsRef.current
      inputHistoryToolsRef.current = null
      if (!savedTools) return
      setFiles(savedTools.files)
      setSelectedKnowledgeBases(savedTools.selectedKnowledgeBases)
    },
    [actionsRef, filesRef, selectedKnowledgeBasesRef, setFiles, setSelectedKnowledgeBases, setText]
  )
  const { isInputHistoryActive, navigateHistory, resetHistoryIndex, saveHistory } = useInputHistory({
    applyDraft: applyHistoryDraft
  })
  const handleTextChange = useCallback(
    (nextText: string) => {
      resetHistoryIndex()
      inputHistoryToolsRef.current = null
      setText(nextText)
    },
    [resetHistoryIndex, setText]
  )
  const fillWorkbenchDraft = useCallback(
    (next: WorkbenchComposerFill) => {
      resetHistoryIndex()
      inputHistoryToolsRef.current = null
      const resolved = typeof next === 'string' ? (reviewFillFromText(next) ?? next) : next
      if (typeof resolved === 'string') {
        setDraftTokens([])
        draftTokensRef.current = []
        actionsRef.current.onTextChange(resolved)
        return
      }
      const tokens = [...resolved.tokens]
      setDraftTokens(tokens)
      draftTokensRef.current = tokens
      setText(resolved.text)
      actionsRef.current.replaceDraft({ text: resolved.text, tokens })
    },
    [actionsRef, resetHistoryIndex, setText]
  )
  useEffect(() => {
    return registerWorkbenchComposer({
      fill: fillWorkbenchDraft,
      focus: () => actionsRef.current.focus('end')
    })
  }, [actionsRef, fillWorkbenchDraft])
  // The catalog closes before this host node exists. Apply a queued shortcut only
  // once the home composer is the one on screen, so an earlier instance cannot consume it.
  useEffect(() => {
    if (!workbenchComposerHost) return
    settleWorkbenchComposer()
  }, [workbenchComposerHost])
  const handleInputHistoryNavigate = useCallback(
    (direction: InputHistoryDirection) => navigateHistory(direction, actionsRef.current.getDraft()),
    [actionsRef, navigateHistory]
  )

  useEffect(() => {
    draftTokensRef.current = draftTokens
  }, [draftTokens])

  const selectedKnowledgeBasesScopeKey = `${sessionTopicId}:${agentId}`
  const {
    selectableKnowledgeBases,
    selectedKnowledgeBasesInScope,
    resolveKnowledgeBaseMarker,
    restoreKnowledgeBaseSelection
  } = useComposerKnowledgeBaseScope({
    configuredKnowledgeBaseIds: agent?.knowledgeBaseIds,
    allKnowledgeBases,
    isKnowledgeBasesLoading,
    scopeKey: selectedKnowledgeBasesScopeKey,
    selectedKnowledgeBases,
    setSelectedKnowledgeBases,
    // The tool provider above is keyed by agent + session + workspace, so this hook remounts with the scope.
    remountsOnScopeChange: true
  })

  useEffect(() => {
    if (isKnowledgeBaseDraftHydrated || isKnowledgeBasesLoading) return

    const wantedIds = new Set(initialDraft.knowledgeBaseIds)
    const restoredIds = selectableKnowledgeBases.filter((base) => wantedIds.has(base.id)).map((base) => base.id)
    observedKnowledgeBaseSelectionKeyRef.current = JSON.stringify(restoredIds)
    restoreKnowledgeBaseSelection(initialDraft.knowledgeBaseIds)
    setIsKnowledgeBaseDraftHydrated(true)
  }, [
    initialDraft.knowledgeBaseIds,
    isKnowledgeBaseDraftHydrated,
    isKnowledgeBasesLoading,
    restoreKnowledgeBaseSelection,
    selectableKnowledgeBases
  ])

  // A home send clears the draft, then the composer remounts into the task
  // conversation. The unmount snapshot still sees the editor document, so without
  // this flag that snapshot writes the sent text back and the next mount restores it.
  const sentDraftRef = useRef(false)
  const persistedOnceRef = useRef(false)
  useEffect(() => {
    if (!draftPersistenceEnabled || isInputHistoryActive || !isKnowledgeBaseDraftHydrated) return
    if (homeDraftHandoffActive(sessionId)) return
    if (sentDraftRef.current) {
      if (text.trim().length > 0 || files.length > 0 || draftTokens.length > 0) {
        sentDraftRef.current = false
      } else {
        return
      }
    }
    if (!persistedOnceRef.current) {
      persistedOnceRef.current = true
      return
    }

    const selectedKnowledgeBaseIds = selectedKnowledgeBasesInScope.map((base) => base.id)
    const selectedKnowledgeBaseIdsKey = JSON.stringify(selectedKnowledgeBaseIds)
    if (observedKnowledgeBaseSelectionKeyRef.current === null) {
      observedKnowledgeBaseSelectionKeyRef.current = selectedKnowledgeBaseIdsKey
    } else if (observedKnowledgeBaseSelectionKeyRef.current !== selectedKnowledgeBaseIdsKey) {
      observedKnowledgeBaseSelectionKeyRef.current = selectedKnowledgeBaseIdsKey
      const selectableKnowledgeBaseIdSet = new Set(selectableKnowledgeBases.map((base) => base.id))
      const unresolvedKnowledgeBaseIds = knowledgeBaseIdsRef.current.filter(
        (id) => !selectableKnowledgeBaseIdSet.has(id)
      )
      knowledgeBaseIdsRef.current = [...new Set([...unresolvedKnowledgeBaseIds, ...selectedKnowledgeBaseIds])]
    }
    if (!sessionDraftWriteAllowed(homeActiveRef.current)) return
    const draft = actionsRef.current.getDraft()
    writeAgentDraftCache(draftCacheKey, {
      text: draft.text,
      tokens: draft.tokens,
      files,
      knowledgeBaseIds: knowledgeBaseIdsRef.current,
      workspaceKey,
      agentId,
      shouldValidateSkills
    })
  }, [
    actionsRef,
    agentId,
    draftCacheKey,
    draftPersistenceEnabled,
    draftTokens,
    files,
    isInputHistoryActive,
    isKnowledgeBaseDraftHydrated,
    selectableKnowledgeBases,
    selectedKnowledgeBasesInScope,
    sessionId,
    text,
    workspaceKey,
    shouldValidateSkills
  ])

  const persistFinalDraft = useEffectEvent(() => {
    if (!draftPersistenceEnabled || isInputHistoryActive) return
    if (homeDraftHandoffActive(sessionId) || sentDraftRef.current) {
      writeAgentDraftCache(draftCacheKey, {
        text: '',
        tokens: [],
        files: [],
        knowledgeBaseIds: [],
        workspaceKey,
        agentId
      })
      return
    }
    if (!sessionDraftWriteAllowed(homeActiveRef.current)) return
    // Managed-sync token changes never reach `draftTokens` (isSyncingTokensRef suppresses
    // onTokensChange), so the cache must come from one surface serialization, not the shadow state.
    const draft = actionsRef.current.getDraft()
    writeAgentDraftCache(draftCacheKey, {
      text: draft.text,
      tokens: draft.tokens,
      files: filesRef.current,
      knowledgeBaseIds: knowledgeBaseIdsRef.current,
      workspaceKey,
      agentId,
      shouldValidateSkills
    })
  })
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `useEffectEvent` reads the latest draft; cleanup is keyed only by persistence/session/workspace.
  useEffect(() => () => persistFinalDraft(), [draftCacheKey, draftPersistenceEnabled, workspaceKey])

  const tokens = useMemo(
    () =>
      retainPromptFileTokens(
        [
          ...files.map(agentFileToComposerToken),
          ...selectedKnowledgeBasesInScope.map(agentKnowledgeBaseToComposerToken),
          ...selectedSkills.map(agentSkillToComposerToken)
        ],
        draftTokens
      ),
    [draftTokens, files, selectedKnowledgeBasesInScope, selectedSkills]
  )
  const resolveSkillMarker = useCallback(
    (marker: string): ComposerDraftToken | null => {
      const skill = skillByFilename.get(marker)
      return skill ? agentSkillToComposerToken(skill) : null
    },
    [skillByFilename]
  )

  const handleSurfaceActionsChange = useCallback(
    (actions: ComposerSurfaceActions) => {
      Object.assign(actionsRef.current, actions)
    },
    [actionsRef]
  )

  useEffect(() => {
    if (!layerActive) return
    return EventEmitter.on(EVENT_NAMES.FOCUS_CHAT_COMPOSER, (payload) => {
      const topicId = typeof payload === 'object' && payload ? (payload as { topicId?: string }).topicId : undefined
      if (topicId !== sessionTopicId) return
      actionsRef.current.focus('end')
    })
  }, [actionsRef, layerActive, sessionTopicId])

  useEffect(() => {
    if (!layerActive) return
    return EventEmitter.on(EVENT_NAMES.INSERT_AGENT_COMPOSER_TOKEN, (payload) => {
      const data =
        typeof payload === 'object' && payload
          ? (payload as { topicId?: string; token?: ComposerDraftToken; updateOnly?: boolean })
          : null
      if (!data?.token || data.topicId !== sessionTopicId) return
      data.updateOnly ? actionsRef.current.insertToken(data.token, true) : actionsRef.current.insertToken(data.token)
    })
  }, [actionsRef, layerActive, sessionTopicId])

  useEffect(() => {
    if (!launchOptions?.initialDraft) return
    const frameId = window.requestAnimationFrame(() => actionsRef.current.focus('end'))
    return () => window.cancelAnimationFrame(frameId)
  }, [actionsRef, launchOptions?.initialDraft])

  const insertSkillToken = useCallback(
    (skill: LocalSkill, inputAdapter?: QuickPanelInputAdapter) => {
      if (!inputAdapter?.insertToken) return

      const token = agentSkillToComposerToken(skill)
      const exists = selectedSkills.some((selectedSkill) => agentComposerTokenId.skill(selectedSkill) === token.id)
      if (!exists) {
        inputAdapter.insertToken(token)
        setSelectedSkills((prev) =>
          prev.some((selectedSkill) => agentComposerTokenId.skill(selectedSkill) === token.id) ? prev : [...prev, skill]
        )
      }
      inputAdapter.focus()
    },
    [selectedSkills]
  )

  const skillManageFooterAction = useMemo<ComposerToolFooterAction>(() => {
    const label = t('plugins.manage_skills')
    return {
      id: 'agent-skills:manage',
      panelSymbol: AGENT_SKILLS_LAUNCHER_ID,
      order: 10,
      label,
      ariaLabel: label,
      tooltip: label,
      icon: <Settings2 size={16} />,
      action: () => openResourceEditDialog({ kind: 'agent', id: agentId, initialTab: 'tools.skills' })
    }
  }, [agentId, t])

  const skillLabel = t('plugins.skills')
  const skillItems = useMemo<QuickPanelListItem[]>(
    () =>
      createSkillQuickPanelItems(availableSkills, {
        skillLabel,
        onInsertSkill: insertSkillToken
      }),
    [availableSkills, insertSkillToken, skillLabel]
  )
  const skillsLauncher = useMemo<ComposerToolLauncher>(() => {
    return {
      id: AGENT_SKILLS_LAUNCHER_ID,
      kind: 'panel',
      sources: ['root-panel'],
      order: 40,
      label: skillLabel,
      icon: <ToolCase />,
      searchAliases: [skillLabel],
      panelSymbol: AGENT_SKILLS_LAUNCHER_ID,
      rootSearchItems: skillItems.map((item) => ({ ...item, suffix: skillLabel })),
      action: ({ inputAdapter, parentPanel, queryAnchor, quickPanel, triggerInfo }) => {
        void refreshAvailableSkills().catch((error) => {
          logger.warn('Failed to refresh available skills when opening the skills panel', { error })
        })
        quickPanel.open({
          title: skillLabel,
          list: skillItems,
          symbol: AGENT_SKILLS_LAUNCHER_ID,
          parentPanel,
          ...prepareComposerQuickPanelSearch({ inputAdapter, queryAnchor, triggerInfo })
        })
      }
    }
  }, [refreshAvailableSkills, skillItems, skillLabel])

  useEffect(
    () => toolsRegistry.registerLaunchers(AGENT_SKILLS_LAUNCHER_ID, [skillsLauncher], [skillManageFooterAction]),
    [skillManageFooterAction, skillsLauncher, toolsRegistry]
  )

  // Keep an already-open skills submenu in sync once a refresh resolves — the launcher action opens
  // it with the current (possibly stale) closure, so an externally installed/removed skill would
  // otherwise only appear on the next open (mirrors the MCP status panel).
  const updateQuickPanelList = quickPanel?.updateList
  useEffect(() => {
    if (!skillsPanelVisible || !updateQuickPanelList) return
    updateQuickPanelList(skillItems)
  }, [skillsPanelVisible, skillItems, updateQuickPanelList])

  const handleRootPanelOpen = useCallback(() => {
    void refreshAvailableSkills().catch((error) => {
      logger.warn('Failed to refresh available skills when opening root panel', { error })
    })
  }, [refreshAvailableSkills])

  useComposerQuoteInsertion(actionsRef)
  useComposerSelectionReferenceInsertion(actionsRef, sessionTopicId)

  const abortAgentSession = useCallback(async () => {
    if (homeDraftActive) return
    logger.info('Aborting agent session', { sessionTopicId })
    try {
      await chatStop()
    } catch (error) {
      logger.error('Failed to abort agent session', { sessionTopicId, error })
    }
  }, [chatStop, homeDraftActive, sessionTopicId])

  const handleAgentChange = useCallback(
    async (nextAgentId: string | null) => {
      if (!nextAgentId || nextAgentId === agentId) return
      if (onAgentChange) {
        await onAgentChange(nextAgentId)
        return
      }
      await updateSession({ id: sessionId, agentId: nextAgentId }, { showSuccessToast: false })
    },
    [agentId, onAgentChange, sessionId, updateSession]
  )

  const handleModelSelect = useCallback(
    async (nextModel?: Model) => {
      if (!agent || !canChangeModel || !nextModel || nextModel.id === model?.id) return

      const nextReasoningEffort = resolveReasoningEffortForModel(nextModel, reasoningEffort) ?? 'default'
      const pendingReasoningEdit =
        pendingReasoningEditRef.current?.agentId === agent.id ? pendingReasoningEditRef.current : null
      const pendingReasoningEffort = pendingReasoningEdit ? { reasoningEffort: pendingReasoningEdit.effort } : {}
      const previousReasoningOverride = activeReasoningOverride
      const version = ++reasoningMutationVersionRef.current
      setReasoningOverride({
        agentId: agent.id,
        value: nextReasoningEffort,
        version
      })

      const updatedAgent = await updateModel(
        { agentId: agent.id, modelId: nextModel.id, ...pendingReasoningEffort },
        { showSuccessToast: false }
      )
      if (!updatedAgent) {
        setReasoningOverride((current) => {
          if (current?.agentId !== agent.id || current.version !== version) return current
          if (!previousReasoningOverride) return null

          const previousEditStillPending =
            pendingReasoningEditRef.current?.agentId === previousReasoningOverride.agentId &&
            pendingReasoningEditRef.current.version === previousReasoningOverride.version
          return previousEditStillPending || previousReasoningOverride.canonicalAtMutationStart !== undefined
            ? previousReasoningOverride
            : { ...previousReasoningOverride, canonicalAtMutationStart: canonicalReasoningEffort }
        })
        return
      }
      if (
        pendingReasoningEdit &&
        pendingReasoningEditRef.current?.agentId === pendingReasoningEdit.agentId &&
        pendingReasoningEditRef.current?.version === pendingReasoningEdit.version
      ) {
        pendingReasoningEditRef.current = null
      }
      setReasoningOverride((current) => (current?.agentId === agent.id && current.version === version ? null : current))
    },
    [activeReasoningOverride, agent, canChangeModel, canonicalReasoningEffort, model?.id, reasoningEffort, updateModel]
  )

  useEffect(() => {
    if (!workbenchSurface || !agent || !composerEffort) return
    if ((agent.configuration?.reasoning_effort ?? 'default') === composerEffort) return
    void updateAgent({ id: agent.id, configuration: { reasoning_effort: composerEffort } }, { showSuccessToast: false })
  }, [agent, composerEffort, updateAgent, workbenchSurface])

  useEffect(() => {
    if (!workbenchSurface || !agent || !canChangeModel || !rememberedModel) return
    if (rememberedModel.id === model?.id) {
      syncedModelId.current = rememberedModel.id
      return
    }
    if (syncedModelId.current === rememberedModel.id) return
    syncedModelId.current = rememberedModel.id
    void handleModelSelect(rememberedModel)
  }, [agent, canChangeModel, handleModelSelect, model?.id, rememberedModel, workbenchSurface])

  const handleCreateEmptySession = useCallback(() => {
    void onCreateEmptySession?.()
  }, [onCreateEmptySession])
  const hasNewSessionAction = Boolean(onCreateEmptySession)

  const rootPanelNewSessionItems = useMemo<QuickPanelListItem[]>(() => {
    if (!hasNewSessionAction) return []

    const label = t('agent.session.new')

    return [
      {
        id: AGENT_NEW_SESSION_TOOL_ID,
        label,
        icon: <NewConversationIcon size={16} />,
        filterText: label,
        searchAliases: getQuickPanelSearchAliases(t, 'agent.session.new'),
        action: () => {
          handleCreateEmptySession()
        }
      }
    ]
  }, [handleCreateEmptySession, hasNewSessionAction, t])

  const toolsSession = sessionData
  const handleReasoningEffortChange = useCallback(
    (option: ThinkingOption) => {
      if (!agent) return

      const canonicalAtMutationStart = canonicalReasoningEffort
      const version = ++reasoningMutationVersionRef.current
      pendingReasoningEditRef.current = { agentId: agent.id, version, effort: option }
      if (workbenchSurface) {
        writePendingReasoningEffort(option, undefined, memberId)
        setComposerEffort(option)
      }
      setReasoningOverride({
        agentId: agent.id,
        value: option,
        version
      })

      void updateAgent(
        {
          id: agent.id,
          configuration: { reasoning_effort: option }
        },
        { showSuccessToast: false }
      ).then((updatedAgent) => {
        if (!updatedAgent) return

        if (
          pendingReasoningEditRef.current?.agentId === agent.id &&
          pendingReasoningEditRef.current.version === version
        ) {
          pendingReasoningEditRef.current = null
        }
        setReasoningOverride((current) =>
          current?.agentId === agent.id && current.version === version
            ? {
                ...current,
                value: updatedAgent.configuration?.reasoning_effort ?? 'default',
                canonicalAtMutationStart
              }
            : current
        )
      })
    },
    [agent, canonicalReasoningEffort, memberId, updateAgent, workbenchSurface]
  )
  const handleServiceTierChange = useCallback(
    (tier: ServiceTierSelection) => {
      if (!agent) return
      const version = ++serviceTierMutationVersionRef.current
      setServiceTierOverride({ agentId: agent.id, value: tier, version })
      void updateAgent({ id: agent.id, configuration: { service_tier: tier } }, { showSuccessToast: false }).then(
        () => {
          setServiceTierOverride((current) =>
            current?.agentId === agent.id && current.version === version ? null : current
          )
        }
      )
    },
    [agent, updateAgent]
  )

  // File reconcile (prune + dedup) is owned by attachmentTool via the tools DI seam. Skill
  // reconcile stays here (agent-only, no shared duplication) alongside the editor draft-token
  // cache snapshot, which is variant state.
  const reconcileTokens = useComposerTokenReconcile({ scope, model, session: toolsSession })
  const handleTokensChange = useCallback(
    (draftTokens: readonly ComposerSerializedToken[]) => {
      const nextDraftTokens = getAgentDraftTokens(draftTokens)
      setDraftTokens(nextDraftTokens)
      draftTokensRef.current = nextDraftTokens
      reconcileTokens(draftTokens)

      const skillTokenIds = getAgentComposerTokenIds(draftTokens, 'skill')
      const skillTokens = draftTokens.filter((token) => token.kind === 'skill')
      setSelectedSkills((prev) => {
        const next = prev.filter((skill) => skillTokenIds.has(agentComposerTokenId.skill(skill)))
        const nextIds = new Set(next.map(agentComposerTokenId.skill))
        let changed = next.length !== prev.length

        for (const token of skillTokens) {
          const skill = availableSkills.find((candidate) => {
            const candidateId = agentComposerTokenId.skill(candidate)
            return candidateId === token.id || candidate.name === token.label || candidate.filename === token.label
          })
          if (!skill) continue

          const skillId = agentComposerTokenId.skill(skill)
          if (nextIds.has(skillId)) continue
          next.push(skill)
          nextIds.add(skillId)
          changed = true
        }

        return changed ? next : prev
      })
    },
    [availableSkills, reconcileTokens]
  )

  const resolvedNewlineShortcut = resolveNewlineShortcut(newlineShortcut, sendMessageShortcut)
  const resolvedSteerShortcut = resolveSteerShortcut(steerShortcut, sendMessageShortcut, resolvedNewlineShortcut)
  const commitWorkspaceFiles = useCallback(
    (next: readonly WorkspaceFileChoice[]) => {
      const label = workspaceFileGroupLabel(next, (count) =>
        t('workbench.composer.files.group', '{{count}} 个文件').replace('{{count}}', String(count))
      )
      if (!label) return
      actionsRef.current.insertToken(createWorkspaceFileGroupToken(next, label), false, 'end')
    },
    [actionsRef, t]
  )
  const workspaceFileSelection = useMemo(
    () => ({
      selectedPaths: selectedWorkspacePaths(files, draftTokens),
      onCommit: commitWorkspaceFiles
    }),
    [commitWorkspaceFiles, draftTokens, files]
  )

  const homeComposer = useWorkbenchHomeComposer({
    enabled: workbenchSurface,
    tokens: draftTokens,
    fileCount: files.length,
    model: workbenchSurface ? (rememberedModel ?? model) : model,
    canChangeModel,
    catalogModelPicker: canChangeModel,
    reasoningEffort,
    onReasoningEffortChange: handleReasoningEffortChange,
    onModelSelect: (next) => {
      void handleModelSelect(next)
    },
    modelFilter: agentModelFilter,
    isModelDisabled,
    workspace,
    workspaceId,
    // A home send over an existing task keeps the pick pending so it belongs to the next task.
    onWorkspaceChange: workbenchComposerHost && sessionHasMessages ? undefined : onWorkspaceChange,
    workspaceChanging,
    projectLocked,
    sessionId: homeActive ? null : sessionId,
    scopeElement: conversationFrame ? conversationScope : null,
    onModelUnavailable: () => {
      if (missingModelMessage) toast.error(missingModelMessage)
    },
    text,
    onTextChange: setText,
    appWorkspacePath,
    workspaceFileSelection: workbenchSurface ? workspaceFileSelection : undefined
  })
  const homeResearchBlocked = workbenchSurface && researchBlocksSend(homeComposer.options, files.length)
  const paperloomSend =
    workbenchSurface && !launchOptions?.editing && paperloomDraftQuestion({ text, tokens: draftTokens }) !== null
  const seededProjectSessionRef = useRef<string | null>(null)
  useEffect(() => {
    if (!workbenchComposerHost || !sessionHasMessages) return
    if (seededProjectSessionRef.current === sessionId) return
    seededProjectSessionRef.current = sessionId
    if (readPendingProject()) return
    if (workspace?.type === 'user' && workspaceId) selectWorkbenchProject(workspaceId)
  }, [sessionHasMessages, sessionId, workbenchComposerHost, workspace, workspaceId])

  const placeholderText = useMemo(
    () =>
      homeActive && composerMode.prompt
        ? composerMode.prompt
        : workbenchSurface && !composerStreaming
          ? t('workbench.composer.placeholder', '描述任务，输入 / 使用指令或技能，@ 使用智能体')
          : composerStreaming
            ? t('agent.input.placeholder_streaming', {
                sendKey: getComposerShortcutLabel(sendMessageShortcut),
                steerKey: getComposerShortcutLabel(resolvedSteerShortcut)
              })
            : t('agent.input.placeholder', { key: getComposerShortcutLabel(sendMessageShortcut) }),
    [
      composerMode.prompt,
      composerStreaming,
      homeActive,
      resolvedSteerShortcut,
      sendMessageShortcut,
      t,
      workbenchSurface
    ]
  )
  const buildQueuedPayload = useCallback(
    (draft: ComposerSerializedDraft): ComposerQueuedMessagePayload | null => {
      const mentionId = draft.tokens.map((token) => agentIdFromMentionToken(token.id)).find((id) => id)
      const parameterTurn = agentParameterTurn(
        mentionId && /^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/.test(mentionId) ? mentionId : undefined,
        draft.tokens
      )
      const hostContext = agentHostContext(
        sessionPreface,
        resolveAppLanguage(i18n?.resolvedLanguage ?? i18n?.language),
        appWorkspacePath
      )
      if (mentionId) {
        const guide = agentWikiGuide(mentionId)
        hostContext.guidelines = [hostContext.guidelines, guide]
          .filter((line) => line.length > 0)
          .join('\n\n')
          .slice(0, 8000)
      }
      const homeTurn = workbenchSurface ? homeTurnContext(homeComposer.options) : null
      const agentId = mentionId && /^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/.test(mentionId) ? mentionId : undefined
      const mentionToken = draft.tokens.find((token) => agentIdFromMentionToken(token.id) === agentId)
      const mentionPayload =
        mentionToken && mentionToken.payload && typeof mentionToken.payload === 'object'
          ? (mentionToken.payload as {
              agentRuntime?: unknown
              cloudBinding?: unknown
              conversationId?: unknown
              reviewSource?: unknown
            })
          : undefined
      const agentRuntime = aixDeclarationFromUnknown(mentionPayload?.agentRuntime)
      const storedSource = launchOptions?.editing
        ? launchOptions.initialParts?.find((part) => part.type === 'data-review-source')
        : undefined
      const rawReviewSource =
        mentionPayload?.reviewSource ??
        (storedSource?.type === 'data-review-source' && storedSource.data.kind === 'text'
          ? { kind: storedSource.data.kind, text: storedSource.data.text }
          : undefined)
      const reviewSource = reviewTextSourceSchema.safeParse(rawReviewSource)
      if (rawReviewSource !== undefined && !reviewSource.success) return null
      const cloudBinding = cloudBindingFromUnknown(mentionPayload?.cloudBinding)
      const conversationId =
        typeof mentionPayload?.conversationId === 'string' && mentionPayload.conversationId.length > 0
          ? mentionPayload.conversationId
          : undefined
      const turn =
        homeTurn || agentId
          ? {
              directives: homeTurn?.directives ?? [],
              research: homeTurn?.research ?? false,
              ...(homeTurn ? { workbench: homeTurn.workbench } : {}),
              ...(agentId ? { agentId } : {}),
              ...(reviewSource.success ? { reviewSource: reviewSource.data } : {}),
              ...(parameterTurn ? { parametersSchema: parameterTurn.schema } : {}),
              ...(agentRuntime ? { agentRuntime } : {}),
              ...(cloudBinding ? { cloudBinding } : {}),
              ...(conversationId ? { conversationId } : {})
            }
          : null
      const payload = buildComposerQueuedPayload(draft, {
        files,
        fileTokenId: agentComposerTokenId.file,
        extra: () => ({
          reasoningEffort: model ? resolveSupportedReasoningEffort(model, reasoningEffort) : reasoningEffort,
          serviceTier: model ? resolveSupportedServiceTier(model, serviceTier) : serviceTier,
          ...(fastMode && model?.supportsFastMode === true ? { fastMode: true } : {}),
          hostContext,
          ...(turn &&
          (turn.directives.length > 0 ||
            turn.workbench ||
            turn.research ||
            turn.agentId ||
            turn.parametersSchema ||
            turn.agentRuntime ||
            turn.conversationId)
            ? { turnContext: turn }
            : {})
        })
      })
      if (!payload) return null
      const reviewed = withReviewDocumentLine(payload, agentId, draft.tokens)

      const tokenIds = getAgentComposerTokenIds(draft.tokens)
      const knowledgeBaseIds = selectedKnowledgeBasesInScope
        .filter((base) => tokenIds.has(agentComposerTokenId.knowledge(base)))
        .map((base) => base.id)
      return {
        ...reviewed,
        userMessageParts: withKnowledgeScopePart(reviewed.userMessageParts, knowledgeBaseIds)
      }
    },
    [
      appWorkspacePath,
      fastMode,
      files,
      homeComposer.options,
      i18n,
      launchOptions?.editing,
      launchOptions?.initialParts,
      model,
      reasoningEffort,
      selectedKnowledgeBasesInScope,
      serviceTier,
      sessionPreface,
      workbenchSurface
    ]
  )

  const sendQueuedPayload = useCallback(
    async (payload: ComposerQueuedMessagePayload) => {
      if (autoModel && agent) {
        try {
          const resolved = await resolvePublishedAutoModel(catalogModels, createModel)
          if (!resolved) {
            toast.error(t('workbench.model_picker.auto_unavailable'))
            return false
          }
          if (resolved.id !== model?.id) {
            const updated = await updateModel({ agentId: agent.id, modelId: resolved.id }, { showSuccessToast: false })
            if (!updated) return false
          }
        } catch (error: unknown) {
          logger.warn('Failed to resolve the automatic model', error as Error)
          toast.error(t('workbench.model_picker.auto_unavailable'))
          return false
        }
      }
      try {
        const attachments = (payload.attachments as ComposerAttachment[] | undefined) ?? []
        const originals = launchOptions?.initialParts?.filter((part): part is FileUIPart => part.type === 'file') ?? []
        const retainedParts = attachments.map((attachment) => {
          const index = initialDraft.files.findIndex((file) => file.fileTokenSourceId === attachment.fileTokenSourceId)
          return index >= 0 ? originals[index] : undefined
        })
        const addedParts = await buildAgentFilePartsForAttachments(
          attachments.filter((_, index) => !retainedParts[index]),
          accessiblePaths
        )
        let addedIndex = 0
        const fileParts = retainedParts.map((part) => part ?? addedParts[addedIndex++])
        const taskMode = workbenchSurface ? displayedTaskPermissionMode(sessionId, memberId) : undefined
        const sent = await chatSendMessage(
          { text: payload.text },
          {
            body: {
              agentId,
              sessionId,
              userMessageParts: [...payload.userMessageParts, ...fileParts],
              reasoningEffort: payload.reasoningEffort,
              serviceTier: payload.serviceTier,
              ...(payload.fastMode ? { fastMode: true } : {}),
              ...(payload.hostContext ? { hostContext: payload.hostContext } : {}),
              ...(payload.turnContext ? { turnContext: payload.turnContext } : {}),
              ...(taskMode === 'default' ||
              taskMode === 'plan' ||
              taskMode === 'auto' ||
              taskMode === 'bypassPermissions'
                ? { permissionMode: taskMode }
                : {})
            }
          }
        )
        if (sent === false) {
          return false
        }
        void EventEmitter.emit(EVENT_NAMES.SEND_MESSAGE, { topicId: sessionTopicId })
        saveHistory(getComposerHistoryText(payload.userMessageParts))
        launchOptions?.onSent?.()
        return true
      } catch (error: unknown) {
        logger.warn('Failed to send message:', error as Error)
        toast.error(t('chat.input.send_failed'))
        return false
      }
    },
    [
      accessiblePaths,
      agent,
      agentId,
      autoModel,
      catalogModels,
      chatSendMessage,
      createModel,
      initialDraft.files,
      launchOptions,
      memberId,
      model?.id,
      saveHistory,
      sessionId,
      sessionTopicId,
      t,
      updateModel,
      workbenchSurface
    ]
  )

  const clearCurrentDraft = useCallback(() => {
    sentDraftRef.current = true
    actionsRef.current.replaceDraft({ text: '', tokens: [] })
    setText('')
    setFiles([])
    setSelectedSkills([])
    setShouldValidateSkills(false)
    setDraftTokens([])
    draftTokensRef.current = []
    if (draftPersistenceEnabled && sessionDraftWriteAllowed(homeActive)) {
      writeAgentDraftCache(draftCacheKey, {
        text: '',
        tokens: [],
        files: [],
        knowledgeBaseIds: knowledgeBaseIdsRef.current,
        workspaceKey,
        agentId
      })
    }
    setTimeoutTimer('agentComposerSendMessage', () => setText(''), 500)
    // Drop the input-history nav state so a recalled draft that gets sent/queued
    // does not leave useInputHistory pointing at it; otherwise the next
    // ArrowDown would restore the already-sent draft and ArrowUp would resume
    // from a stale index.
    resetHistoryIndex()
    inputHistoryToolsRef.current = null
  }, [
    actionsRef,
    agentId,
    draftCacheKey,
    draftPersistenceEnabled,
    homeActive,
    resetHistoryIndex,
    setFiles,
    setText,
    setTimeoutTimer,
    workspaceKey
  ])

  // Queue mode (same as chat): while the session streams, follow-ups queue here and auto-drain on idle.
  const { isFulfilled: sessionFulfilled, markSeen: markSessionSeen } = useTopicStreamStatus(sessionTopicId)
  const {
    items: queuedFollowups,
    enqueue: enqueueFollowup,
    removeId: removeFollowup,
    reorder: reorderFollowups,
    paused: followupPaused,
    setPaused: setFollowupPaused
  } = useFollowupQueue({
    scopeKey: launchOptions?.editing ? `${sessionTopicId}:edit:${launchOptions.editing.messageId}` : sessionTopicId,
    isFulfilled: !launchOptions?.editing && sessionFulfilled,
    markSeen: markSessionSeen,
    onDrain: sendQueuedPayload
  })

  useEffect(() => {
    if (launchOptions?.editing) return
    void ipcApi
      .request('ai.agent.session.set_pending_input_count', { sessionId, count: queuedFollowups.length })
      .catch((error) => logger.warn('Failed to publish pending input count', { error }))
  }, [launchOptions?.editing, queuedFollowups.length, sessionId])

  // Edit a queued item = atomically restore the whole editor draft, then synchronize live token
  // state and the managed file/knowledge/skill selections before dropping it from the queue.
  const restoreFollowupDraft = useCallback(
    (item: FollowupQueueItem) => {
      const nextDraftTokens = getAgentDraftTokens(item.draft.tokens)
      resetHistoryIndex()
      inputHistoryToolsRef.current = null
      actionsRef.current.replaceDraft(item.draft)
      setDraftTokens(nextDraftTokens)
      draftTokensRef.current = nextDraftTokens
      setText(item.draft.text)
      setFiles((item.payload.attachments as ComposerAttachment[] | undefined) ?? [])
      setSelectedSkills(getCachedSkillTokens(nextDraftTokens).map(getSkillFromCachedToken))
      restoreKnowledgeBaseSelection(getKnowledgeBaseIdsFromParts(item.payload.userMessageParts) ?? [])
      handleReasoningEffortChange(item.payload.reasoningEffort ?? 'default')
      handleServiceTierChange(item.payload.serviceTier ?? 'standard')
      setFastMode(item.payload.fastMode === true)
    },
    [
      actionsRef,
      handleReasoningEffortChange,
      handleServiceTierChange,
      resetHistoryIndex,
      restoreKnowledgeBaseSelection,
      setFiles,
      setText
    ]
  )

  const handleSendDraft = useCallback(
    async (draft: ComposerSerializedDraft, options?: { steer?: boolean }) => {
      if (directSendInFlightRef.current) return
      const paperloomQuestion = paperloomDraftQuestion(draft)
      const homeDraft = homeActive && !launchOptions?.editing && !homeDraftHandoffActive(sessionId)
      const taskMode = workbenchSurface
        ? displayedTaskPermissionMode(homeDraft ? 'draft' : sessionId, memberId)
        : (agent?.configuration?.permission_mode ?? 'default')
      if (workbenchSurface && agent && (agent.configuration?.permission_mode ?? 'default') !== taskMode) {
        await updateAgent(
          { id: agent.id, configuration: { permission_mode: taskMode } },
          { showSuccessToast: false }
        )
      }
      if (workbenchSurface && !launchOptions?.editing && paperloomQuestion !== null) {
        if (
          !openPaperloom(
            paperloomQuestion,
            {
              root: fileMentionRoot(homeComposer.projectPath || workspace?.path, appWorkspacePath),
              paths: selectedWorkspacePaths(filesRef.current, draft.tokens)
            },
            () => {
              if (samePaperloomDraft(actionsRef.current.getDraft(), draft)) clearCurrentDraft()
            }
          )
        )
          toast.error(t('paperloom.open_failed'))
        return
      }
      const storedTextReview =
        launchOptions?.editing &&
        launchOptions.initialParts?.some(
          (part) =>
            part.type === 'data-review-source' &&
            part.data.kind === 'text' &&
            reviewTextSourceSchema.safeParse({ kind: part.data.kind, text: part.data.text }).success
        )
      if (
        !storedTextReview &&
        reviewDraftNeedsSource(
          draft,
          files.map((file) => file.path ?? '')
        )
      ) {
        toast.error(t('workbench.review.composer.choose'))
        return
      }
      let outgoing = draft
      let deck: HomeComposerDeck | undefined
      let codePlan: HomeComposerCodePlan | undefined
      if (homeActive) {
        const outcome = await requestHomeComposerSend({
          surface: 'live',
          text: draft.text,
          permissionMode: taskMode
        })
        if (outcome.handled) {
          if (outcome.cleared) clearCurrentDraft()
          return
        }
        if (outcome.text !== undefined) outgoing = { ...draft, text: outcome.text }
        deck = outcome.deck
        codePlan = outcome.code
      }
      if (sendDisabled) return
      if (!model) {
        toast.error(t('code.model_required'))
        return
      }
      // A code send chooses its own directory. The open task's missing folder must not hide that.
      if (workspaceWarning && !codePlan) {
        toast.error(workspaceWarning)
        return
      }
      if (workbenchSurface && researchBlocksSend(homeComposer.options, files.length)) {
        toast.error(t('workbench.composer.research_warning', '深度研究不接受任务资源；移除资源后才能发送。'))
        return
      }
      if (homeDraft && !codePlan && !deck && (!workbenchComposerHost || !homeTask)) {
        toast.error(t('workbench.default_agent.preparing'))
        return
      }
      // A deck starts a task too. The homepage covers the workspace, so arming the
      // open empty session never shows the canvas.
      const creatingTask = Boolean(
        workbenchComposerHost && homeTask && !launchOptions?.editing && (sessionHasMessages || deck || homeDraft)
      )
      let codeRecord: Awaited<ReturnType<typeof placeCodeSend>> | null = null
      if (codePlan) {
        directSendInFlightRef.current = true
        setIsDirectSending(true)
        try {
          codeRecord = await placeCodeSend({
            placement: codePlan.placement,
            projectPath: homeComposer.projectPath || null,
            topic: draft.text.trim(),
            suffix: codePlan.suffix,
            branch: codePlan.branch,
            base: codePlan.base,
            existing: codePlan.existing,
            pending: creatingTask,
            bindCurrent: creatingTask ? undefined : onWorkspaceChange
          })
        } catch (error) {
          toast.error(codeWorkspaceErrorText(error))
          return
        } finally {
          directSendInFlightRef.current = false
          setIsDirectSending(false)
        }
        if (!creatingTask) writeCodeSession(sessionId, codeRecord)
      }
      // The home composer is bound to whatever session is active. Once that session
      // has messages, the draft moves onto a new task and that task sends it.
      if (creatingTask && homeTask) {
        directSendInFlightRef.current = true
        setIsDirectSending(true)
        let moved = false
        try {
          const created = await homeTask.create()
          if (!created) {
            if (codeRecord?.placement === 'worktree') writePendingProject(null)
            return
          }
          if (homeDraft && created.id === sessionId && (sessionHasMessages || isStreaming)) {
            if (codeRecord?.placement === 'worktree') writePendingProject(null)
            toast.error(t('agent.session.create.error.failed'))
            return
          }
          if (codeRecord) writeCodeSession(created.id, codeRecord)
          if (created.id !== sessionId) {
            if (deck) armPresentationDeck(created.id, deck)
            writeAgentDraftCache(getAgentDraftCacheKey(created.id), {
              text: deck ? '' : outgoing.text,
              tokens: deck ? [] : [...outgoing.tokens],
              files: deck ? [] : filesRef.current,
              knowledgeBaseIds: deck ? [] : [...knowledgeBaseIdsRef.current],
              workspaceKey,
              agentId
            })
            if (!deck) markPendingHomeSubmit(created.id)
            homeTask.adopt(created.id)
            clearCurrentDraft()
            moved = true
          }
        } finally {
          directSendInFlightRef.current = false
          setIsDirectSending(false)
        }
        if (moved) return
      }
      if (deck) {
        armPresentationDeck(sessionId, deck)
        clearCurrentDraft()
        return
      }
      const payload = buildQueuedPayload(outgoing)
      if (!payload) return

      // Busy (streaming) → queue the follow-up; the head auto-drains when the session goes idle and
      // the dock lets the user steer/edit/remove items. The steer shortcut opts out of the queue and
      // falls through to the direct send below, mirroring the dock's "insert" action.
      if (isStreaming && !options?.steer) {
        enqueueFollowup(outgoing, payload)
        clearCurrentDraft()
        return
      }

      directSendInFlightRef.current = true
      setIsDirectSending(true)
      try {
        const sent = await sendQueuedPayload(payload)
        if (sent) clearCurrentDraft()
      } finally {
        directSendInFlightRef.current = false
        setIsDirectSending(false)
      }
    },
    [
      agent,
      agentId,
      appWorkspacePath,
      buildQueuedPayload,
      clearCurrentDraft,
      enqueueFollowup,
      homeActive,
      files,
      filesRef,
      homeComposer.options,
      homeComposer.projectPath,
      homeTask,
      isStreaming,
      memberId,
      onWorkspaceChange,
      launchOptions?.editing,
      launchOptions?.initialParts,
      model,
      sendDisabled,
      sendQueuedPayload,
      sessionHasMessages,
      sessionId,
      t,
      updateAgent,
      workbenchComposerHost,
      workbenchSurface,
      workspace?.path,
      workspaceKey,
      workspaceWarning
    ]
  )

  const homeSubmitSent = useRef(false)
  useEffect(() => {
    if (homeSubmitSent.current || launchOptions?.editing || !peekPendingHomeSubmit(sessionId)) return
    if (sendDisabled || !model || !isKnowledgeBaseDraftHydrated) return
    const hasDraft =
      text.trim().length > 0 || files.length > 0 || draftTokensRef.current.some((token) => token.kind === 'skill')
    if (!hasDraft) return
    homeSubmitSent.current = true
    consumePendingHomeSubmit(sessionId)
    // Drop the cached copy before the send awaits. Leaving home remounts this
    // composer, and that mount would otherwise restore the sentence just sent.
    beginHomeDraftHandoff(sessionId)
    if (draftPersistenceEnabled) {
      writeAgentDraftCache(draftCacheKey, {
        text: '',
        tokens: [],
        files: [],
        knowledgeBaseIds: [],
        workspaceKey,
        agentId
      })
    }
    void handleSendDraft({ text, tokens: draftTokensRef.current })
  }, [
    agentId,
    draftCacheKey,
    draftPersistenceEnabled,
    files.length,
    handleSendDraft,
    isKnowledgeBaseDraftHydrated,
    launchOptions?.editing,
    model,
    sendDisabled,
    sessionId,
    text,
    workspaceKey
  ])

  useEffect(() => {
    if (!homeDraftHandoffActive(sessionId)) return
    if (text.trim().length > 0 || files.length > 0) return
    finishHomeDraftHandoff(sessionId)
  }, [files.length, sessionId, text])

  const { getItems: getEntityReferenceItems, hasPendingReference } = useEntityReferenceMentionItems({
    entityType: 'session',
    excludeId: sessionId
  })
  const resourceMentionSources = useAgentResourceMentionSource({
    accessiblePaths,
    files,
    setFiles,
    enabled: !workbenchSurface && enableResourceMention,
    getAdditionalItems: workbenchSurface ? undefined : getEntityReferenceItems
  })

  const toolbarCustomTools = useMemo<ComposerToolbarCustomTool[]>(() => {
    const newSessionLabel = t('agent.session.new')
    const skillLabel = t('plugins.skills')
    const slashCommandsLabel = t('chat.input.slash_commands.title')
    return [
      ...(hasNewSessionAction
        ? [
            {
              id: AGENT_NEW_SESSION_TOOL_ID,
              label: newSessionLabel,
              icon: <NewConversationIcon size={18} aria-hidden />,
              customizePlacement: 'leading' as const,
              requiresPanel: false,
              onSelect: () => handleCreateEmptySession()
            }
          ]
        : []),
      {
        id: 'skills',
        label: skillLabel,
        icon: <ToolCase size={18} aria-hidden />,
        onSelect: ({ unifiedPanelControl }) =>
          unifiedPanelControl?.open({ launcherId: AGENT_SKILLS_LAUNCHER_ID, searchText: skillLabel })
      },
      {
        id: 'slash-commands',
        label: slashCommandsLabel,
        icon: <Terminal size={18} aria-hidden />,
        onSelect: ({ unifiedPanelControl }) => unifiedPanelControl?.open({ searchText: slashCommandsLabel })
      },
      {
        id: ComposerPanelSymbol.McpStatus,
        label: 'MCP',
        icon: <McpLogo width={18} height={18} aria-hidden />,
        onSelect: ({ unifiedPanelControl }) =>
          unifiedPanelControl?.open({ launcherId: ComposerPanelSymbol.McpStatus, searchText: 'MCP' })
      }
    ]
  }, [handleCreateEmptySession, hasNewSessionAction, t])

  const renderQuickPanelShortcuts = useCallback(
    ({
      inputAdapter,
      unifiedPanelControl
    }: {
      inputAdapter?: AgentComposerInputAdapter
      unifiedPanelControl?: AgentComposerUnifiedPanelControl
    }) => (
      <>
        <ComposerToolbarShortcuts
          scope={TopicType.Session}
          pinnedIds={workbenchSurface ? homeComposer.pinnedIds : pinnedToolIds}
          onPinnedIdsChange={workbenchSurface ? homeComposer.setPinnedIds : setPinnedToolIds}
          onResetPinnedIds={workbenchSurface ? homeComposer.resetPinnedIds : resetPinnedToolIds}
          isDefault={workbenchSurface ? homeComposer.pinsAtDefault : pinnedToolsAtDefault}
          customTools={toolbarCustomTools}
          customizeOpen={customizeToolbarOpen}
          onCustomizeOpenChange={setCustomizeToolbarOpen}
          isModelUnavailable={isModelUnavailable}
          inputAdapter={inputAdapter}
          unifiedPanelControl={unifiedPanelControl}
        />
      </>
    ),
    [
      customizeToolbarOpen,
      homeComposer,
      isModelUnavailable,
      pinnedToolIds,
      pinnedToolsAtDefault,
      resetPinnedToolIds,
      setCustomizeToolbarOpen,
      setPinnedToolIds,
      toolbarCustomTools,
      workbenchSurface
    ]
  )

  const controlSlots = renderControls({
    agent,
    model,
    workspace,
    workspaceId,
    workspaceWarning,
    selectAgentLabel: t('chat.alerts.select_agent'),
    selectModelLabel: t('button.select_model'),
    selectWorkspaceLabel: t('agent.session.workspace_selector.placeholder'),
    agentChanging,
    agentTriggerMode: canChangeAgent ? 'selector' : 'edit',
    shouldAutoSelectCreatedAgent: true,
    topBarPortalAvailable,
    topBarPortalIconOnly,
    canChangeModel,
    onModelSelect: handleModelSelect,
    modelFilter: agentModelFilter,
    isModelDisabled,
    renderQuickPanelShortcuts,
    onAgentChange: handleAgentChange,
    onWorkspaceChange,
    workspaceChanging
  })

  const surfaceControlSlots = {
    ...controlSlots,
    renderLeftControls: (inputAdapter, unifiedPanelControl) =>
      workbenchSurface
        ? homeComposer.renderLayout({ inputAdapter, unifiedPanelControl })
        : controlSlots.renderLeftControls?.(inputAdapter, unifiedPanelControl)
  }

  const sendAccessory: ComposerSurfaceProps['sendAccessory'] = (
    <>
      {homeComposer.accessory}
      {!workbenchSurface && model && !launchOptions?.editing ? (
        <ModelSpeedControl
          model={model}
          reasoningEffort={reasoningEffort}
          serviceTier={serviceTier}
          fastMode={fastMode}
          onReasoningEffortChange={handleReasoningEffortChange}
          onServiceTierChange={handleServiceTierChange}
          onFastModeChange={setFastMode}
        />
      ) : null}
      <AgentComposerContextUsage model={model} sessionId={sessionId} />
    </>
  )

  const composerTree = (
    <ComposerToolDerivedStateProvider
      couldAddImageFile={canAddImageFile}
      extensions={supportedExts}
      selectableKnowledgeBases={selectableKnowledgeBases}>
      {model && <ComposerToolRuntimeHost scope={scope} model={model} session={toolsSession} />}
      {homeComposer.chrome}
      <ResourceEditDialogEventHost />
      <ComposerPinnedToolsProvider value={pinnedLauncherIds}>
        <HomeComposerInput
          showAiDisclaimer
          draftScope={
            launchOptions?.editing ? `${sessionTopicId}:edit:${launchOptions.editing.messageId}` : sessionTopicId
          }
          text={text}
          onTextChange={handleTextChange}
          editable={!isDirectSending}
          tokens={tokens}
          draftTokens={draftTokens}
          managedTokenKinds={
            isKnowledgeBaseDraftHydrated
              ? AGENT_MANAGED_TOKEN_KINDS
              : AGENT_MANAGED_TOKEN_KINDS_BEFORE_KNOWLEDGE_RESTORE
          }
          onTokensChange={handleTokensChange}
          resolveKnowledgeBaseMarker={resolveKnowledgeBaseMarker}
          resolveSkillMarker={resolveSkillMarker}
          placeholder={placeholderText}
          sendMessageShortcut={sendMessageShortcut}
          steerShortcut={composerStreaming ? resolvedSteerShortcut : undefined}
          sendDisabled={
            isDirectSending ||
            hasPendingReference ||
            (!paperloomSend &&
              (sendDisabled ||
                modelPending ||
                !!missingModelMessage ||
                homeResearchBlocked ||
                (text.trim().length === 0 && files.length === 0 && selectedSkills.length === 0)))
          }
          sendBlockedReason={
            !paperloomSend && homeResearchBlocked
              ? t('workbench.composer.research_warning', '深度研究不接受任务资源；移除资源后才能发送。')
              : isDirectSending || hasPendingReference || (!paperloomSend && sendDisabled)
                ? t('common.loading')
                : paperloomSend
                  ? undefined
                  : missingModelMessage
          }
          isLoading={composerStreaming}
          onSendDraft={handleSendDraft}
          editingState={
            launchOptions?.editing ? { ...launchOptions.editing, cancelDisabled: isDirectSending } : undefined
          }
          onPause={abortAgentSession}
          queueContent={
            homeDraftActive ? undefined : (
            <>
              {queuedFollowups.length > 0 ? (
                <QueuedFollowupsDock
                  items={queuedFollowups}
                  paused={followupPaused}
                  onTogglePause={() => setFollowupPaused(!followupPaused)}
                  onSteer={async (id) => {
                    const item = queuedFollowups.find((entry) => entry.id === id)
                    if (!item) return
                    // Only drop the item once the send actually succeeds; a failed manual
                    // steer keeps it in the dock + toasts, matching the direct-send/auto-drain paths.
                    const sent = await sendQueuedPayload(item.payload)
                    if (sent) removeFollowup(id)
                  }}
                  onEdit={(id) => {
                    const item = queuedFollowups.find((entry) => entry.id === id)
                    if (!item) return
                    restoreFollowupDraft(item)
                    removeFollowup(id)
                  }}
                  onRemove={removeFollowup}
                  onReorder={reorderFollowups}
                />
              ) : undefined}
            </>
            )
          }
          supportedExts={supportedExts}
          setFiles={setFiles}
          showScreenshot={!homeActive && !workbenchSurface}
          screenshotDisabled={Boolean(launchOptions?.editing)}
          screenshotAcceptQueued={workbenchSurface && !conversationFrame}
          filesCount={files.length}
          isExpanded={isExpanded}
          onExpandedChange={setIsExpanded}
          quickPanelEnabled={config.enableQuickPanel ?? true}
          enableDragDrop={config.enableDragDrop ?? true}
          enableSpellCheck={enableSpellCheck}
          fontSize={fontSize}
          editorMinHeight={workbenchSurface ? WORKBENCH_COMPOSER_EDITOR_MIN_HEIGHT : undefined}
          lockEditorHeight={Boolean(workbenchSurface)}
          narrowMode={forceNarrowLayout || narrowMode}
          railGutterPx={railGutterPx}
          onActionsChange={handleSurfaceActionsChange}
          isInputHistoryActive={isInputHistoryActive}
          onInputHistoryNavigate={handleInputHistoryNavigate}
          getToolLaunchers={() => getLaunchers()}
          toolLaunchersVersion={toolLaunchersVersion}
          suggestionSources={
            homeComposer.suggestionSource
              ? [...resourceMentionSources, homeComposer.suggestionSource]
              : resourceMentionSources
          }
          rootPanelLeadingItems={workbenchSurface ? undefined : rootPanelNewSessionItems}
          rootPanelAdditionalItems={workbenchSurface ? homeComposer.rootPanelItems : undefined}
          rootPanelCatalogOnly={Boolean(workbenchSurface)}
          textHighlights={workbenchSurface ? HOME_COMPOSER_TEXT_HIGHLIGHTS : undefined}
          onRootPanelOpen={handleRootPanelOpen}
          onToolLauncherSelect={(launcher, options) => dispatchLauncher(launcher, options)}
          sendAccessory={sendAccessory}
          compactWhenSingleLine={compactWhenSingleLine}
          deferQuickPanel={deferQuickPanel}
          {...surfaceControlSlots}
        />
      </ComposerPinnedToolsProvider>
    </ComposerToolDerivedStateProvider>
  )

  const worktreeSlot =
    workbenchSurface && !launchOptions?.editing ? (
      <WorkbenchWorktreeSlot
        cwd={userWorkspacePath}
        hasMessages={sessionHasMessages}
        onWorkspaceChange={onWorkspaceChange}
      />
    ) : null

  if (!conversationFrame) {
    return (
      <>
        {composerTree}
        {worktreeSlot}
      </>
    )
  }

  return (
    <div className="pointer-events-auto mx-auto w-full max-w-[760px] px-6 pb-4">
      <div data-ui="workbench.composer" className="relative w-full">
        {composerTree}
        {worktreeSlot}
      </div>
      <div ref={setConversationScope} data-ui="workbench.composer.scope" />
    </div>
  )
}

type MissingAgentHomeComposerProps = {
  onAgentChange?: (agentId: string | null) => void | Promise<void>
  agentChanging?: boolean
  permissionAgentId?: string | null
}

type MissingAgentHomeComposerInnerProps = MissingAgentHomeComposerProps & {
  actionsRef: React.RefObject<ProviderActionHandlers>
}

const MissingAgentHomeComposerInner = ({
  onAgentChange,
  agentChanging,
  permissionAgentId = null,
  actionsRef
}: MissingAgentHomeComposerInnerProps) => {
  const { updateAgent } = useUpdateAgent()
  const workbenchComposerHost = use(WorkbenchComposerHostContext)
  const homeActive = use(WorkbenchHomeActiveContext)
  const composerMode = useHomeComposerModeState()
  const config = getComposerToolConfig(TopicType.Session)
  const { files, isExpanded } = useComposerToolState()
  const { setFiles, setIsExpanded } = useComposerToolDispatch()
  const { getLaunchers, dispatchLauncher } = useComposerToolLauncherActions()
  const toolLaunchersVersion = useComposerToolLauncherVersion()
  const [enableSpellCheck] = usePreference('app.spell_check.enabled')
  const [fontSize] = usePreference('chat.message.font_size')
  const [storedSendShortcut] = usePreference('chat.input.send_message_shortcut')
  const appWorkspacePath = useDisplayedWorkspacePath()
  const sendMessageShortcut = resolveSendShortcut(storedSendShortcut)
  const [narrowMode] = usePreference('chat.narrow_mode')
  // Yield the same rail gutter as the message column so the composer stays aligned.
  const { railGutterPx } = useChatLayoutMode()
  const { available: topBarPortalAvailable, iconOnly: topBarPortalIconOnly } = useConversationTopBarPortalLayout()
  const { t } = useTranslation()
  const [text, setText] = useState('')
  useEffect(() => {
    if (!homeActive) return
    noteHomeComposerTopic(text)
  }, [homeActive, text])
  const [homeDraftTokens, setHomeDraftTokens] = useState<ComposerSerializedToken[]>([])
  const [sendingHome, setSendingHome] = useState(false)
  const sendingHomeRef = useRef(false)
  const homeTask = use(WorkbenchHomeTaskContext)
  const { model: defaultModel } = useWorkbenchModel()
  const { supportedExts: modelSupportedExts } = useComposerFileCapabilities(defaultModel)
  // Ordinary chat needs a model; workspace reading does not.
  const homeSupportedExts =
    workbenchComposerHost && modelSupportedExts.length === 0 ? agentFileExtensions() : modelSupportedExts
  useQueuedWorkbenchFiles({
    enabled: Boolean(workbenchComposerHost),
    supportedExts: homeSupportedExts,
    setFiles
  })
  const agentRuntimeUnavailable = useWorkbenchAgentRuntimeUnavailable()
  // A ready home composer sends. The blocked copy is only a real missing model or runtime.
  const selectAgentMessage = !workbenchComposerHost
    ? t('chat.alerts.select_agent')
    : agentRuntimeUnavailable
      ? t('workbench.default_agent.runtime_unavailable')
      : !defaultModel
        ? t('workbench.default_agent.needs_model')
        : sendingHome
          ? t('common.loading')
          : undefined
  const paperloomSend =
    Boolean(workbenchComposerHost) && paperloomDraftQuestion({ text, tokens: homeDraftTokens }) !== null
  const homeHasDraft =
    paperloomSend ||
    text.trim().length > 0 ||
    files.length > 0 ||
    homeDraftTokens.some((token) => token.kind === 'skill')
  const homeSendReady = Boolean(
    workbenchComposerHost && !sendingHome && (paperloomSend || (defaultModel && !agentRuntimeUnavailable))
  )
  const handleSurfaceActionsChange = useCallback(
    (actions: ComposerSurfaceActions) => {
      Object.assign(actionsRef.current, actions)
    },
    [actionsRef]
  )
  const fillHomeDraft = useCallback(
    (next: WorkbenchComposerFill) => {
      const resolved = typeof next === 'string' ? (reviewFillFromText(next) ?? next) : next
      if (typeof resolved === 'string') {
        setHomeDraftTokens([])
        actionsRef.current.onTextChange(resolved)
        return
      }
      const tokens = [...resolved.tokens]
      setHomeDraftTokens(tokens)
      setText(resolved.text)
      actionsRef.current.replaceDraft({ text: resolved.text, tokens })
    },
    [actionsRef]
  )
  useEffect(() => {
    return registerWorkbenchComposer({
      fill: fillHomeDraft,
      focus: () => actionsRef.current.focus('end')
    })
  }, [actionsRef, fillHomeDraft])
  useEffect(() => {
    if (!workbenchComposerHost) return
    settleWorkbenchComposer()
  }, [workbenchComposerHost])
  const handleAgentChange = useCallback(
    async (nextAgentId: string | null) => {
      if (!nextAgentId) return
      await onAgentChange?.(nextAgentId)
    },
    [onAgentChange]
  )
  const {
    pinnedIds: pinnedToolIds,
    setPinnedIds: setPinnedToolIds,
    resetPinnedIds: resetPinnedToolIds,
    isDefault: pinnedToolsAtDefault,
    customizeOpen: customizeToolbarOpen,
    setCustomizeOpen: setCustomizeToolbarOpen
  } = useComposerToolbarPinnedTools('agent.input.toolbar.pinned_tools')
  const [reasoningEffort, setReasoningEffort] = useState<ThinkingOption>(
    () => readPendingReasoningEffort() ?? 'default'
  )
  const homeMemberId = useMemberProfile()?.id ?? null
  const rememberReasoningEffort = useCallback(
    (effort: ThinkingOption) => {
      writePendingReasoningEffort(effort, undefined, homeMemberId)
      setReasoningEffort(effort)
    },
    [homeMemberId]
  )
  useEffect(() => {
    const stored = readPendingReasoningEffort(undefined, homeMemberId)
    if (stored) setReasoningEffort(stored)
  }, [homeMemberId])
  const commitHomeWorkspaceFiles = useCallback(
    (next: readonly WorkspaceFileChoice[]) => {
      const label = workspaceFileGroupLabel(next, (count) =>
        t('workbench.composer.files.group', '{{count}} 个文件').replace('{{count}}', String(count))
      )
      if (!label) return
      actionsRef.current.insertToken(createWorkspaceFileGroupToken(next, label), false, 'end')
    },
    [actionsRef, t]
  )
  const homeWorkspaceFileSelection = useMemo(
    () => ({
      selectedPaths: selectedWorkspacePaths(files, homeDraftTokens),
      onCommit: commitHomeWorkspaceFiles
    }),
    [commitHomeWorkspaceFiles, files, homeDraftTokens]
  )
  const homeComposer = useWorkbenchHomeComposer({
    enabled: Boolean(workbenchComposerHost),
    tokens: homeDraftTokens,
    fileCount: files.length,
    model: defaultModel,
    canChangeModel: false,
    catalogModelPicker: !agentRuntimeUnavailable,
    reasoningEffort,
    onReasoningEffortChange: rememberReasoningEffort,
    onModelUnavailable: () => {
      if (selectAgentMessage) toast.error(selectAgentMessage)
    },
    text,
    onTextChange: setText,
    appWorkspacePath,
    workspaceFileSelection: workbenchComposerHost ? homeWorkspaceFileSelection : undefined
  })
  const handleHomeSend = useCallback(
    async (draft: ComposerSerializedDraft) => {
      if (sendingHomeRef.current) return
      const question = paperloomDraftQuestion(draft)
      if (workbenchComposerHost && question !== null) {
        if (
          !openPaperloom(
            question,
            {
              root: fileMentionRoot(homeComposer.projectPath, appWorkspacePath),
              paths: selectedWorkspacePaths(files, draft.tokens)
            },
            () => {
              if (!samePaperloomDraft(actionsRef.current.getDraft(), draft)) return
              setHomeDraftTokens([])
              setText('')
              setFiles([])
              actionsRef.current.replaceDraft({ text: '', tokens: [] })
            }
          )
        )
          toast.error(t('paperloom.open_failed'))
        return
      }
      if (
        reviewDraftNeedsSource(
          draft,
          files.map((file) => file.path ?? '')
        )
      ) {
        toast.error(t('workbench.review.composer.choose'))
        return
      }
      let outgoing = draft
      let deck: HomeComposerDeck | undefined
      let codePlan: HomeComposerCodePlan | undefined
      if (homeActive) {
        const mode = displayedTaskPermissionMode('draft', homeMemberId)
        if (permissionAgentId) {
          await updateAgent(
            { id: permissionAgentId, configuration: { permission_mode: mode as PermissionMode } },
            { showSuccessToast: false }
          )
        }
        const outcome = await requestHomeComposerSend({
          surface: 'live',
          text: draft.text,
          permissionMode: mode
        })
        if (outcome.handled) {
          if (outcome.cleared) {
            setHomeDraftTokens([])
            setText('')
            actionsRef.current.replaceDraft({ text: '', tokens: [] })
          }
          return
        }
        if (outcome.text !== undefined) outgoing = { ...draft, text: outcome.text }
        deck = outcome.deck
        codePlan = outcome.code
      }
      if (!workbenchComposerHost) {
        toast.error(t('chat.alerts.select_agent'))
        return
      }
      if (agentRuntimeUnavailable) {
        toast.error(t('workbench.default_agent.runtime_unavailable'))
        return
      }
      if (!defaultModel) {
        toast.error(t('workbench.default_agent.needs_model'))
        return
      }
      if (!homeTask || sendingHomeRef.current) return
      sendingHomeRef.current = true
      setSendingHome(true)
      try {
        let codeRecord: Awaited<ReturnType<typeof placeCodeSend>> | null = null
        if (codePlan) {
          try {
            codeRecord = await placeCodeSend({
              placement: codePlan.placement,
              projectPath: homeComposer.projectPath || null,
              topic: draft.text.trim(),
              suffix: codePlan.suffix,
              branch: codePlan.branch,
              base: codePlan.base,
              existing: codePlan.existing,
              pending: true
            })
          } catch (error) {
            toast.error(codeWorkspaceErrorText(error))
            return
          }
        }
        const created = await homeTask.create()
        if (!created) {
          if (codeRecord?.placement === 'worktree') writePendingProject(null)
          return
        }
        if (codeRecord) writeCodeSession(created.id, codeRecord)
        if (deck) armPresentationDeck(created.id, deck)
        writeAgentDraftCache(getAgentDraftCacheKey(created.id), {
          text: deck ? '' : outgoing.text,
          tokens: deck ? [] : [...outgoing.tokens],
          files: deck ? [] : files,
          knowledgeBaseIds: [],
          workspaceKey: '',
          agentId: ''
        })
        if (!deck) markPendingHomeSubmit(created.id)
        homeTask.adopt(created.id)
        setHomeDraftTokens([])
        setText('')
        actionsRef.current.replaceDraft({ text: '', tokens: [] })
      } finally {
        sendingHomeRef.current = false
        setSendingHome(false)
      }
    },
    [
      actionsRef,
      agentRuntimeUnavailable,
      appWorkspacePath,
      defaultModel,
      files,
      homeActive,
      homeComposer.projectPath,
      homeMemberId,
      homeTask,
      permissionAgentId,
      setFiles,
      t,
      updateAgent,
      workbenchComposerHost
    ]
  )
  const homeFileTokens = useMemo(() => files.map(agentFileToComposerToken), [files])
  const homeTokens = useMemo(
    () => retainPromptFileTokens(homeFileTokens, homeDraftTokens),
    [homeDraftTokens, homeFileTokens]
  )
  const handleHomeTokensChange = useCallback(
    (nextTokens: readonly ComposerSerializedToken[]) => {
      setHomeDraftTokens(getAgentDraftTokens(nextTokens))
      const fileTokenIds = getAgentComposerTokenIds(nextTokens, 'file')
      setFiles((prev) => {
        const next = prev.filter((file) => fileTokenIds.has(agentComposerTokenId.file(file)))
        return next.length === prev.length ? prev : next
      })
    },
    [setFiles]
  )
  const homeSuggestionSources = useMemo(
    () => (homeComposer.suggestionSource ? [homeComposer.suggestionSource] : []),
    [homeComposer.suggestionSource]
  )
  const toolbarCustomTools = useMemo<ComposerToolbarCustomTool[]>(() => {
    const newSessionLabel = t('agent.session.new')
    const skillLabel = t('plugins.skills')
    const slashCommandsLabel = t('chat.input.slash_commands.title')
    const permissionLabel = t('agent.settings.permissionMode.title')
    return [
      {
        id: AGENT_NEW_SESSION_TOOL_ID,
        label: newSessionLabel,
        icon: <NewConversationIcon size={18} aria-hidden />,
        customizePlacement: 'leading',
        requiresPanel: false,
        availableWithoutModel: true,
        onSelect: () => {
          if (selectAgentMessage) toast.error(selectAgentMessage)
        }
      },
      {
        id: 'skills',
        label: skillLabel,
        icon: <ToolCase size={18} aria-hidden />,
        availableWithoutModel: true,
        onSelect: ({ unifiedPanelControl }) =>
          unifiedPanelControl?.open({ launcherId: AGENT_SKILLS_LAUNCHER_ID, searchText: skillLabel })
      },
      {
        id: 'permission-mode',
        label: permissionLabel,
        icon: <Pointer size={18} aria-hidden />,
        availableWithoutModel: true,
        onSelect: ({ unifiedPanelControl }) =>
          unifiedPanelControl?.open({ launcherId: 'permission-mode', searchText: permissionLabel })
      },
      {
        id: 'slash-commands',
        label: slashCommandsLabel,
        icon: <Terminal size={18} aria-hidden />,
        availableWithoutModel: true,
        onSelect: ({ unifiedPanelControl }) => unifiedPanelControl?.open({ searchText: slashCommandsLabel })
      },
      {
        id: ComposerPanelSymbol.McpStatus,
        label: 'MCP',
        icon: <McpLogo width={18} height={18} aria-hidden />,
        availableWithoutModel: true,
        onSelect: ({ unifiedPanelControl }) =>
          unifiedPanelControl?.open({ launcherId: ComposerPanelSymbol.McpStatus, searchText: 'MCP' })
      }
    ]
  }, [selectAgentMessage, t])
  const renderQuickPanelShortcuts = useCallback(
    ({
      inputAdapter,
      unifiedPanelControl
    }: {
      inputAdapter?: AgentComposerInputAdapter
      unifiedPanelControl?: AgentComposerUnifiedPanelControl
    }) => (
      <>
        <ComposerToolbarShortcuts
          scope={TopicType.Session}
          pinnedIds={workbenchComposerHost ? homeComposer.pinnedIds : pinnedToolIds}
          onPinnedIdsChange={workbenchComposerHost ? homeComposer.setPinnedIds : setPinnedToolIds}
          onResetPinnedIds={workbenchComposerHost ? homeComposer.resetPinnedIds : resetPinnedToolIds}
          isDefault={workbenchComposerHost ? homeComposer.pinsAtDefault : pinnedToolsAtDefault}
          customTools={toolbarCustomTools}
          customizeOpen={customizeToolbarOpen}
          onCustomizeOpenChange={setCustomizeToolbarOpen}
          inputAdapter={inputAdapter}
          unifiedPanelControl={unifiedPanelControl}
        />
      </>
    ),
    [
      customizeToolbarOpen,
      homeComposer,
      pinnedToolIds,
      pinnedToolsAtDefault,
      resetPinnedToolIds,
      setCustomizeToolbarOpen,
      setPinnedToolIds,
      toolbarCustomTools,
      workbenchComposerHost
    ]
  )
  const rootPanelLeadingItems = useMemo<QuickPanelListItem[]>(() => {
    const label = t('agent.session.new')
    return [
      {
        id: AGENT_NEW_SESSION_TOOL_ID,
        label,
        icon: <NewConversationIcon size={16} />,
        filterText: label,
        action: () => {
          if (selectAgentMessage) toast.error(selectAgentMessage)
        }
      }
    ]
  }, [selectAgentMessage, t])
  const placeholderText =
    homeActive && composerMode.prompt
      ? composerMode.prompt
      : workbenchComposerHost
        ? t('workbench.composer.placeholder', '描述任务，输入 / 使用指令或技能，@ 使用智能体')
        : t('agent.input.placeholder', {
            key: getComposerShortcutLabel(sendMessageShortcut)
          })
  const renderMissingControls = workbenchComposerHost ? renderAgentInputControls : renderAgentToolbarControls
  const controlSlots = renderMissingControls({
    agent: undefined,
    selectAgentLabel: selectAgentMessage ?? t('chat.alerts.select_agent'),
    model: undefined,
    selectModelLabel: t('button.select_model'),
    selectWorkspaceLabel: t('agent.session.workspace_selector.placeholder'),
    workspace: undefined,
    workspaceId: null,
    agentChanging,
    agentTriggerMode: 'selector',
    shouldAutoSelectCreatedAgent: true,
    topBarPortalAvailable,
    topBarPortalIconOnly,
    canChangeModel: false,
    onAgentChange: handleAgentChange,
    onModelSelect: () => undefined,
    renderQuickPanelShortcuts,
    // The workspace selector stays disabled until an agent creates a real session.
    onWorkspaceChange: undefined
  })
  const surfaceControlSlots = {
    ...controlSlots,
    renderLeftControls: (inputAdapter, unifiedPanelControl) =>
      workbenchComposerHost
        ? homeComposer.renderLayout({ inputAdapter, unifiedPanelControl })
        : controlSlots.renderLeftControls?.(inputAdapter, unifiedPanelControl)
  }

  const sendAccessory: ComposerSurfaceProps['sendAccessory'] = (
    <>
      {homeComposer.accessory}
      {!workbenchComposerHost && defaultModel ? (
        <ModelSpeedControl
          model={defaultModel}
          reasoningEffort={reasoningEffort}
          onReasoningEffortChange={setReasoningEffort}
        />
      ) : null}
    </>
  )

  return (
    <ComposerToolDerivedStateProvider
      couldAddImageFile={Boolean(workbenchComposerHost)}
      extensions={workbenchComposerHost ? homeSupportedExts : []}>
      {homeComposer.chrome}
      <HomeComposerInput
        showAiDisclaimer
        text={text}
        onTextChange={setText}
        tokens={homeTokens}
        draftTokens={homeDraftTokens}
        managedTokenKinds={AGENT_MANAGED_TOKEN_KINDS}
        onTokensChange={handleHomeTokensChange}
        placeholder={placeholderText}
        sendMessageShortcut={sendMessageShortcut}
        sendDisabled={!homeSendReady || !homeHasDraft}
        sendBlockedReason={paperloomSend ? undefined : selectAgentMessage}
        isLoading={false}
        onSendDraft={handleHomeSend}
        onPause={() => undefined}
        supportedExts={homeSupportedExts}
        setFiles={setFiles}
        showScreenshot={!homeActive}
        screenshotAcceptQueued={Boolean(workbenchComposerHost)}
        filesCount={files.length}
        isExpanded={isExpanded}
        onExpandedChange={setIsExpanded}
        quickPanelEnabled={config.enableQuickPanel ?? true}
        enableDragDrop={Boolean(workbenchComposerHost)}
        enableSpellCheck={enableSpellCheck}
        fontSize={fontSize}
        editorMinHeight={workbenchComposerHost ? WORKBENCH_COMPOSER_EDITOR_MIN_HEIGHT : undefined}
        lockEditorHeight={Boolean(workbenchComposerHost)}
        narrowMode={narrowMode}
        railGutterPx={railGutterPx}
        onActionsChange={handleSurfaceActionsChange}
        getToolLaunchers={() => getLaunchers()}
        toolLaunchersVersion={toolLaunchersVersion}
        onToolLauncherSelect={(launcher, options) => dispatchLauncher(launcher, options)}
        rootPanelLeadingItems={workbenchComposerHost ? undefined : rootPanelLeadingItems}
        rootPanelAdditionalItems={workbenchComposerHost ? homeComposer.rootPanelItems : undefined}
        rootPanelCatalogOnly={Boolean(workbenchComposerHost)}
        textHighlights={workbenchComposerHost ? HOME_COMPOSER_TEXT_HIGHLIGHTS : undefined}
        suggestionSources={homeSuggestionSources.length > 0 ? homeSuggestionSources : undefined}
        sendAccessory={sendAccessory}
        deferQuickPanel
        {...surfaceControlSlots}
      />
    </ComposerToolDerivedStateProvider>
  )
}

export const MissingAgentHomeComposer = (props: MissingAgentHomeComposerProps) => {
  const initialState = useMemo(
    () => ({
      mentionedModels: [],
      selectedKnowledgeBases: [],
      files: [] as ComposerAttachment[],
      isExpanded: false,
      couldAddImageFile: false,
      extensions: [] as string[]
    }),
    []
  )
  const actionsRef = useRef<ProviderActionHandlers>({ ...emptyActions })

  return (
    <ComposerToolRuntimeProvider
      initialState={initialState}
      actions={{
        onTextChange: (updater) => actionsRef.current.onTextChange(updater),
        addNewTopic: () => undefined
      }}>
      <MissingAgentHomeComposerInner {...props} actionsRef={actionsRef} />
    </ComposerToolRuntimeProvider>
  )
}

// Agent changes reset the composer root; session/workspace isolation is owned by AgentComposerRoot.
const AgentComposer = (props: Props) => {
  return (
    <AgentComposerRoot
      key={props.agentId}
      {...props}
      deferQuickPanel
      renderControls={props.externalContextControls ? renderAgentInputControls : renderAgentToolbarControls}
    />
  )
}

export const AgentHomeComposer = (props: Props) => {
  return (
    <AgentComposerRoot
      key={props.agentId}
      {...props}
      canChangeAgent={props.canChangeAgent ?? true}
      forceNarrowLayout
      renderControls={renderAgentHomeControls}
    />
  )
}

export default AgentComposer
