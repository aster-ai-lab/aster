import { createFileRoute } from '@tanstack/react-router'

import { EvolutionSettings } from '@renderer/pages/settings/EvolutionSettings/EvolutionSettings'

export const Route = createFileRoute('/settings/evolution')({
  component: EvolutionSettings
})
