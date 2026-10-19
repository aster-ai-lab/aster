import { createContext, type ComponentType } from 'react'

export interface WorkbenchAssistantMessageProps {
  messageId: string
  streaming: boolean
  /** Settled row on a topic stream that is still live. */
  awaiting?: boolean
  /** Latest row whose turn is still active, including a pause for tool approval. */
  turnActive?: boolean
  createdAt: string
}

/** Page-supplied task transcript. The shared message frame does not import the page. */
export const WorkbenchAssistantMessageContext = createContext<ComponentType<WorkbenchAssistantMessageProps> | null>(
  null
)
