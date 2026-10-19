import { createFileRoute } from '@tanstack/react-router'

import { WorktreeSettings } from '@renderer/pages/settings/aligned/WorktreeSettings'

export const Route = createFileRoute('/settings/worktree')({
  component: WorktreeSettings
})
