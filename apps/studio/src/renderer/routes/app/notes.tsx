import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/notes')({
  component: () => <PluginPage page="studio.notes/page" />
})
