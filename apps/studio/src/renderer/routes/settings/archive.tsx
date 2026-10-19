import { createFileRoute } from '@tanstack/react-router'

import { ArchivedTasksPage } from '@renderer/pages/settings/aligned/ArchivedTasksPage'

export const Route = createFileRoute('/settings/archive')({ component: ArchivedTasksPage })
