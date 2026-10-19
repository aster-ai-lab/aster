import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/capabilities')({
  component: () => <PluginPage page="studio.capabilities/page" />
})
