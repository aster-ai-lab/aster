import { createFileRoute, redirect } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { parseAgentRouteSearch } from '@renderer/pages/agents/routeSearch'
import { resolveAgentEntrySessionIdForAgent } from '@renderer/utils/conversationEntry'

export const Route = createFileRoute('/app/agents')({
  validateSearch: (search) => parseAgentRouteSearch(search),
  // The work page opens on the home composer. A sidebar `?agentId=` entry still
  // resumes that agent. A bare entry does not open a task.
  beforeLoad: async ({ search }) => {
    if (search.sessionId || search.intent || !search.agentId) return
    const sessionId = await resolveAgentEntrySessionIdForAgent(search.agentId)
    if (sessionId) throw redirect({ to: '/app/agents', search: { sessionId }, replace: true })
  },
  component: () => <PluginPage page="studio.agents/page" />
})
