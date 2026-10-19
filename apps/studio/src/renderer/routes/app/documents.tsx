import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/documents')({
  component: () => <PluginPage page="studio.documents/page" />
})
