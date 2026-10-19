import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

function PluginStoreRoute() {
  return <PluginPage page="studio.plugins/page" />
}

export const Route = createFileRoute('/app/plugins')({
  component: PluginStoreRoute
})
