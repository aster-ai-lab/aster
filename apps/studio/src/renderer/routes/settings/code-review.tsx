import { createFileRoute } from '@tanstack/react-router'

import { CodeReviewSettings } from '@renderer/pages/settings/aligned/CodeReviewSettings'

export const Route = createFileRoute('/settings/code-review')({
  component: CodeReviewSettings
})
