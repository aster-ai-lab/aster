import { Flex } from '@a3s-lab/ui'
import { isEmpty } from 'es-toolkit/compat'
import React from 'react'

import {
  WorkbenchProjectLockedContext,
  WorkbenchTurnStreamContext
} from '@renderer/components/composer/workbenchComposerHost'
import { createUniqueModelId } from '@shared/data/types/model'

import MessagePartsRenderer from '../blocks/MessagePartsRenderer'
import type { MessageListItem } from '../types'
import { taskTurnPresentation } from './taskTurnPresentation'
import { WorkbenchAssistantMessageContext } from './workbenchAssistantMessage'

interface Props {
  message: MessageListItem
  hoistAttachments?: boolean
  /** Latest assistant row. Only this row may show "still working" after its text has settled. */
  latest?: boolean
}

const MessageContent: React.FC<Props> = ({ message, hoistAttachments, latest = false }) => {
  const workbenchConversation = React.use(WorkbenchProjectLockedContext)
  const turnStream = React.use(WorkbenchTurnStreamContext)
  const AssistantMessage = React.use(WorkbenchAssistantMessageContext)
  if (workbenchConversation && message.role === 'assistant' && AssistantMessage) {
    const turn = taskTurnPresentation(message.status === 'pending', turnStream, latest)
    return (
      <AssistantMessage
        awaiting={turn.awaiting}
        createdAt={message.createdAt}
        messageId={message.id}
        streaming={turn.streaming}
        turnActive={turn.active}
      />
    )
  }

  return (
    <>
      {!isEmpty(message.mentions) && (
        <Flex className="mb-2.5 flex-wrap gap-2">
          {message.mentions?.map((model) => (
            <span key={createUniqueModelId(model.provider, model.id)} className="text-foreground">
              {'@' + model.name}
            </span>
          ))}
        </Flex>
      )}
      <MessagePartsRenderer message={message} hoistAttachments={hoistAttachments} />
    </>
  )
}

export default React.memo(MessageContent)
