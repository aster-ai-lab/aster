import { createContext } from 'react'

export type IntelligentUiValues = Record<string, string | number | boolean>
export type IntelligentUiSubmitOutcome = 'sent' | 'busy' | 'failed'
export const IntelligentUiSubmitContext = createContext<((text: string) => Promise<IntelligentUiSubmitOutcome>) | null>(
  null
)
