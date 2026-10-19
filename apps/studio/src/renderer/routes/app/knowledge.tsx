import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { parseKnowledgeRouteSearch } from '@renderer/pages/knowledge/routeSearch'

export const Route = createFileRoute('/app/knowledge')({
  validateSearch: (search) => parseKnowledgeRouteSearch(search),
  component: KnowledgeRoute
})

function KnowledgeRoute() {
  return <PluginPage page="studio.knowledge/page" />
}
