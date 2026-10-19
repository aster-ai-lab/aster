import { createContext, use, useMemo, type ReactNode } from 'react'

import { createPaperloomClient } from './paperloomApi'

const ClientContext = createContext<ReturnType<typeof createPaperloomClient> | null>(null)

export function PaperloomClient({ memberId, children }: { memberId: string | null; children: ReactNode }) {
  const client = useMemo(() => createPaperloomClient(memberId), [memberId])
  return <ClientContext value={client}>{children}</ClientContext>
}

export function usePaperloomClient() {
  const client = use(ClientContext)
  if (!client) throw new Error('Paper reading requires a member-bound client')
  return client
}
