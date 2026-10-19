import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/platform')({
  component: () => <PluginPage page="studio.platform/page" />
})
