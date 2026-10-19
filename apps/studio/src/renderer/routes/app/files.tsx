import { createFileRoute } from '@tanstack/react-router'

import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { parseFilesRouteSearch } from '@renderer/pages/files/routeSearch'

export const Route = createFileRoute('/app/files')({
  validateSearch: (search) => parseFilesRouteSearch(search),
  component: FilesRoute
})

function FilesRoute() {
  return <PluginPage page="studio.files/page" />
}
