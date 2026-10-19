import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/messages')({
  component: () => <PluginPage page="studio.messages/page" />
})
