import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/schedule')({
  component: () => <PluginPage page="studio.schedule/page" />
})
