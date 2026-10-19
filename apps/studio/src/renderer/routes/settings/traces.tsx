import { createFileRoute } from '@tanstack/react-router'

import { ExecutionTraceSettings } from '@renderer/pages/settings/ExecutionTraceSettings/ExecutionTraceSettings'

export const Route = createFileRoute('/settings/traces')({
  component: ExecutionTraceSettings
})
