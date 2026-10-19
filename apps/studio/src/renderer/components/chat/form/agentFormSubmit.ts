import { createContext } from 'react'

import type { AgentFormSubmission } from '@shared/types/agentForm'

import type { IntelligentUiSubmitOutcome } from '../messages/markdown/intelligentUiHost'

/** Resolves the pending agent form without starting a second conversation turn. */
export type AgentFormSubmit = (
  submission: AgentFormSubmission,
  signal: globalThis.AbortSignal
) => Promise<IntelligentUiSubmitOutcome>
export const AgentFormSubmitContext = createContext<AgentFormSubmit | undefined>(undefined)
