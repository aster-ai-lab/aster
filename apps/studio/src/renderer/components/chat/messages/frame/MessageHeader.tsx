import { Checkbox, Tooltip } from '@a3s-lab/ui'
import { useIcon } from '@a3s-lab/ui/icons'
import dayjs from 'dayjs'
import { ArrowUpRight, Bot, MousePointerClick, Sparkle, Target } from 'lucide-react'
import type { FC, ReactNode } from 'react'
import { createContext, memo, use, useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import ModelAvatar from '@renderer/components/Avatar/ModelAvatar'
import { A3SBotAvatar } from '@renderer/components/bot/A3SBotAvatar'
import { botActivityForMessageStatus, latestTurnBotActivity } from '@renderer/components/bot/botAvatarActivity'
import {
  WorkbenchProjectLockedContext,
  WorkbenchTurnStreamContext
} from '@renderer/components/composer/workbenchComposerHost'
import { useTheme } from '@renderer/hooks/useTheme'
import type { Model } from '@renderer/types/model'
import { getModelLogoRef } from '@renderer/utils/model'
import { firstLetter, removeLeadingEmoji } from '@renderer/utils/naming'
import type { AutonomousTurnOrigin } from '@shared/ai/agentSessionTurnOrigin'

import {
  useMessageListActions,
  useMessageListMeta,
  useMessageListSelection,
  useMessageRenderConfig,
  useOptionalMessageListActions
} from '../MessageListProvider'
import { defaultMessageRenderConfig, type MessageListItem } from '../types'
import { getMessageListItemModel } from '../utils/messageListItem'
import MessageAvatar, {
  MESSAGE_AVATAR_SIZE,
  MESSAGE_MODEL_AVATAR_ICON_CLASS,
  MessageAvatarFrame
} from './MessageAvatar'
import MessageTokens from './MessageTokens'

interface Props {
  message: MessageListItem
  model?: Model
  isGroupContextMessage?: boolean
  showModelIdentity?: boolean
  actionsSlot?: ReactNode
  contentSlot?: ReactNode
  footerSlot?: ReactNode
  /** Latest assistant row plays the document-expert pose for this turn. Older rows stay still. */
  animateAssistant?: boolean
}

/** Drop avatar and name. The message body stays. */
export const HideMessageIdentityContext = createContext(false)

/** Why a runtime opened an assistant turn with no user message; the transcript's only explanation. */
export const AutonomousTurnOriginBadge: FC<{ origin: AutonomousTurnOrigin }> = ({ origin }) => {
  const { t } = useTranslation()
  const label =
    origin.kind === 'goal-round'
      ? t('agent.session_turn_origin.goal_round', { round: origin.round })
      : t('agent.session_turn_origin.background_work')
  const Icon = origin.kind === 'goal-round' ? Target : Bot
  return (
    <Tooltip content={label}>
      <span className="flex h-5 max-w-[min(18rem,45vw)] min-w-0 items-center gap-1 text-xs text-foreground-tertiary">
        <Icon aria-hidden="true" className="size-3.5 shrink-0" />
        <span className="min-w-0 truncate">{label}</span>
      </span>
    </Tooltip>
  )
}

export const AgentSessionDeliveryBadge: FC<{
  delivery: NonNullable<MessageListItem['delivery']>
}> = ({ delivery }) => {
  const { t } = useTranslation()
  const actions = useOptionalMessageListActions()
  const senderSessionLabel = delivery.senderSnapshot?.sessionName.trim() || delivery.sender.sessionId
  const senderAgentLabel = delivery.senderSnapshot?.agentName.trim() || delivery.sender.agentId
  const senderLabel = t('agent.session_delivery.from', {
    agent: senderAgentLabel,
    session: senderSessionLabel
  })
  const content = (
    <>
      <MousePointerClick aria-hidden="true" className="size-3.5 shrink-0" />
      <span className="min-w-0 truncate">{senderLabel}</span>
      {actions?.navigateToRoute ? <ArrowUpRight aria-hidden="true" className="size-3.5 shrink-0" /> : null}
    </>
  )

  const openSenderSession = () => {
    if (!actions?.navigateToRoute) return
    void actions.navigateToRoute({ path: '/app/agents', query: { sessionId: delivery.sender.sessionId } })
  }

  return (
    <Tooltip content={senderLabel}>
      {actions?.navigateToRoute ? (
        <button
          type="button"
          aria-label={senderLabel}
          className="flex h-5 max-w-[min(18rem,45vw)] cursor-pointer items-center gap-1 text-xs text-foreground-tertiary hover:text-link hover:underline focus-visible:text-link focus-visible:underline focus-visible:outline-none"
          onClick={openSenderSession}>
          {content}
        </button>
      ) : (
        <span className="flex h-5 max-w-[min(18rem,45vw)] items-center gap-1 text-xs text-foreground-tertiary">
          {content}
        </span>
      )}
    </Tooltip>
  )
}

const MessageHeader: FC<Props> = memo(
  ({
    model,
    message,
    isGroupContextMessage,
    showModelIdentity = false,
    actionsSlot,
    contentSlot,
    footerSlot,
    animateAssistant = false
  }) => {
    const { theme } = useTheme()
    const workbenchConversation = use(WorkbenchProjectLockedContext)
    const hideMessageIdentity = use(HideMessageIdentityContext)
    const turnStreamStatus = use(WorkbenchTurnStreamContext)
    const actions = useMessageListActions()
    const meta = useMessageListMeta()
    const renderConfig = useMessageRenderConfig() ?? defaultMessageRenderConfig
    const selection = useMessageListSelection()
    const userName = renderConfig.userName
    const assistantProfile = meta.assistantProfile
    const { t } = useTranslation()
    const messageStyle = renderConfig.messageStyle
    const isBubbleStyle = messageStyle === 'bubble'
    const userAvatar = meta.userProfile?.avatar ?? ''

    const isMultiSelectMode = selection?.isMultiSelectMode ?? false
    const selectedMessageIds = selection?.selectedMessageIds

    const isSelected = selectedMessageIds?.includes(message.id)

    const messageModel = useMemo(() => getMessageListItemModel(message), [message])
    const displayModel = messageModel ?? model
    const displayModelName = displayModel?.name || displayModel?.id
    const ModelIcon = useIcon(useMemo(() => getModelLogoRef(displayModel), [displayModel]))

    // Producing author (assistant/agent) snapshotted at creation — shown first; the model is secondary.
    // Once a snapshot exists the header is frozen: consult the live profile only when it's entirely absent,
    // so editing/deleting the live entity never changes a past message's name or avatar.
    const authorSnapshot = message.messageSnapshot
    const authorName = authorSnapshot ? authorSnapshot.name : assistantProfile?.name
    const authorAvatar = authorSnapshot ? authorSnapshot.emoji : assistantProfile?.avatar
    const conversationAgentName = t('workbench.conversation.agent')
    const getUserName = useCallback(() => {
      if (message.role === 'assistant') {
        if (workbenchConversation) return conversationAgentName
        return authorName || displayModel?.name || displayModel?.id || ''
      }

      return userName || t('common.you')
    }, [authorName, conversationAgentName, displayModel, message.role, t, userName, workbenchConversation])

    const isAssistantMessage = message.role === 'assistant'
    const delivery = message.delivery
    const hiddenContentHoverClass = isAssistantMessage
      ? 'group-hover/header:opacity-100'
      : 'group-hover/message:opacity-100'
    const hiddenActionsHoverClass = isAssistantMessage
      ? 'group-hover/header:pointer-events-auto group-hover/header:opacity-100'
      : 'group-hover/message:pointer-events-auto group-hover/message:opacity-100'

    const username = useMemo(() => removeLeadingEmoji(getUserName()), [getUserName])
    const avatarName = useMemo(() => firstLetter(authorName ?? username ?? '').toUpperCase(), [authorName, username])

    const openUserProfile = useCallback(() => {
      void actions.openUserProfile?.()
    }, [actions])

    const canOpenUserProfile = !!actions.openUserProfile
    const hasBodySlot = !!contentSlot || !!footerSlot
    const assistantActivity = animateAssistant
      ? latestTurnBotActivity(turnStreamStatus, message.status)
      : botActivityForMessageStatus(message.status)

    if (hideMessageIdentity) {
      return (
        <div className="message-header group/header relative flex items-start">
          <div className="message-body-column flex min-h-0 min-w-0 flex-1 flex-col">
            {contentSlot && <div className="message-body-content min-h-0 max-w-full min-w-0 flex-1">{contentSlot}</div>}
            {footerSlot && <div className="message-footer-slot mt-auto min-w-0 shrink-0">{footerSlot}</div>}
          </div>
        </div>
      )
    }

    return (
      <div
        className={`message-header group/header relative flex gap-2.5 ${hasBodySlot ? 'mb-0 items-start' : 'mb-2 items-center'}`}>
        {isAssistantMessage ? (
          workbenchConversation ? (
            <A3SBotAvatar
              activity={assistantActivity}
              animated={animateAssistant}
              label={conversationAgentName}
              size={MESSAGE_AVATAR_SIZE}
              variant="default"
            />
          ) : authorAvatar ? (
            <MessageAvatar avatar={authorAvatar} fallback={avatarName} />
          ) : ModelIcon ? (
            <MessageAvatarFrame className="bg-background">
              <ModelIcon className={MESSAGE_MODEL_AVATAR_ICON_CLASS} aria-hidden="true" />
            </MessageAvatarFrame>
          ) : (
            <MessageAvatar
              fallback={avatarName}
              fallbackAvatarStyle={{
                border: 'none',
                filter: theme === 'dark' ? 'invert(0.05)' : undefined
              }}
            />
          )
        ) : (
          <MessageAvatar avatar={userAvatar} onClick={canOpenUserProfile ? openUserProfile : undefined} />
        )}
        <div
          className={hasBodySlot ? 'message-body-column flex min-h-0 min-w-0 flex-1 flex-col' : 'flex min-w-0 flex-1'}>
          <div className="flex w-full min-w-0 items-center gap-1.5">
            <span
              className="truncate text-sm leading-5 font-semibold"
              style={{
                color: isBubbleStyle && theme === 'dark' ? 'white' : 'var(--foreground)'
              }}>
              {username}
            </span>
            {!isAssistantMessage && delivery && <AgentSessionDeliveryBadge delivery={delivery} />}
            {isAssistantMessage && message.turnOrigin && <AutonomousTurnOriginBadge origin={message.turnOrigin} />}
            {isAssistantMessage && showModelIdentity && !workbenchConversation && displayModelName && (
              <span className="flex min-w-0 shrink items-center gap-1 text-xs leading-5 text-foreground-tertiary">
                <span aria-hidden="true" className="shrink-0">
                  <ModelAvatar className="rounded-full" model={displayModel} size={16} />
                </span>
                <span className="truncate">{displayModelName}</span>
              </span>
            )}
            {isGroupContextMessage && (
              <Tooltip content={t('chat.message.useful.tip')}>
                <Sparkle className="shrink-0" fill="var(--foreground)" strokeWidth={0} size={16} />
              </Tooltip>
            )}
            <div
              className={`message-header-info-wrap flex shrink-0 items-center gap-1 text-[10px] leading-none text-foreground-tertiary opacity-0 transition-opacity duration-150 focus-within:opacity-100 no-hover:opacity-100 ${hiddenContentHoverClass}`}>
              <span>{dayjs(message?.updatedAt ?? message.createdAt).format('MM/DD HH:mm')}</span>
              {renderConfig.showEstimatedTokens &&
                isBubbleStyle &&
                !isAssistantMessage &&
                message.stats !== undefined && (
                  <>
                    |
                    <MessageTokens message={message} />
                  </>
                )}
            </div>
            {actionsSlot && (
              <div
                className={`message-header-actions pointer-events-none ml-auto flex shrink-0 items-center gap-1 opacity-0 transition-opacity duration-150 focus-within:pointer-events-auto focus-within:opacity-100 no-hover:pointer-events-auto no-hover:opacity-100 ${hiddenActionsHoverClass}`}>
                {actionsSlot}
              </div>
            )}
          </div>
          {contentSlot && (
            <div className="message-body-content mt-2 min-h-0 max-w-full min-w-0 flex-1">{contentSlot}</div>
          )}
          {footerSlot && <div className="message-footer-slot mt-auto min-w-0 shrink-0">{footerSlot}</div>}
        </div>
        {isMultiSelectMode && (
          <Checkbox
            data-message-select-checkbox
            checked={isSelected}
            onCheckedChange={(checked) => actions.selectMessage?.(message.id, checked === true)}
            className="absolute top-0 right-0"
          />
        )}
      </div>
    )
  }
)

MessageHeader.displayName = 'MessageHeader'

export default MessageHeader
