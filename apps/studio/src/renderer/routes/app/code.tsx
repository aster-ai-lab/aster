import { createFileRoute } from '@tanstack/react-router'
import * as z from 'zod'

import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { CodeCli } from '@shared/types/codeCli'

export const Route = createFileRoute('/app/code')({
  validateSearch: (search) => z.object({ tool: z.enum(CodeCli).optional() }).parse(search),
  component: CodeCliRoute
})

function CodeCliRoute() {
  return <PluginPage page="studio.code_tools/page" />
}
