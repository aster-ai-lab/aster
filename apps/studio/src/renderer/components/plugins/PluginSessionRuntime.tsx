import type { ReactNode } from 'react'

import { useCache } from '@renderer/data/hooks/useCache'
import type { PluginServices } from '@renderer/types/workbenchPlugin'
import { buildAgentSessionTopicId } from '@renderer/utils/agentSession'

import { PluginScopeRuntime } from './PluginScopeRuntime'

/** UI ownership follows the selected session; it does not terminate a background Code run. */
export function PluginSessionRuntime({
  sessionId,
  workspace,
  children
}: {
  sessionId: string
  workspace?: PluginServices['workspace']
  children: ReactNode
}) {
  const [observations] = useCache('studio.execution.observations')
  const observed = observations?.[buildAgentSessionTopicId(sessionId)]
  const run = observed && { coreSessionId: observed.coreSessionId, runId: observed.runId, promptId: observed.promptId }
  return (
    <PluginScopeRuntime kind="workspace" services={{ workspace }}>
      <PluginScopeRuntime kind="session" services={{ conversation: { sessionId } }}>
        <PluginScopeRuntime kind="run" enabled={!!observed} services={{ run }}>
          <PluginScopeRuntime
            kind="turn"
            enabled={observed?.modelTurn != null}
            services={{ turn: observed?.modelTurn != null ? { modelTurn: observed.modelTurn } : undefined }}>
            {children}
          </PluginScopeRuntime>
        </PluginScopeRuntime>
      </PluginScopeRuntime>
    </PluginScopeRuntime>
  )
}
