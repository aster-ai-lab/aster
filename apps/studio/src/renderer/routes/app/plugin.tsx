import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { parsePluginPageSearch } from '@renderer/utils/pluginNavigation'

export const Route = createFileRoute('/app/plugin')({
  validateSearch: parsePluginPageSearch,
  component: PluginRoute
})

function PluginRoute() {
  return <PluginPage page={Route.useSearch().page} allowBuiltins={false} />
}
