import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'

export const Route = createFileRoute('/app/mail')({
  component: () => <PluginPage page="studio.mail/page" />
})
