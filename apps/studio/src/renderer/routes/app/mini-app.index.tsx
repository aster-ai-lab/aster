import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/mini-app/')({
  component: () => <PluginPage page="studio.mini_app/page" />
})
