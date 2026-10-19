import type { TestProjectInlineConfiguration } from 'vitest/config'

import base from './vitest.config'

const renderer = base.test?.projects?.find(
  (project): project is TestProjectInlineConfiguration =>
    typeof project === 'object' && project !== null && 'test' in project && project.test?.name === 'renderer'
)
if (!renderer || typeof renderer === 'string') throw new Error('Renderer test project missing')

export default {
  ...base,
  test: {
    ...base.test,
    projects: [
      {
        ...renderer,
        test: {
          ...renderer.test,
          name: 'workspace-documents',
          include: ['tests/native/workspaceDocuments.test.tsx', 'tests/native/reviewObservation.test.tsx']
        }
      }
    ]
  }
}
