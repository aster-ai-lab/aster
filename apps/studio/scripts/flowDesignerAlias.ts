import { resolve } from 'node:path'

/**
 * Specifiers for the console Flow UI designer.
 * Longer keys stay ahead of shorter prefixes. Do not add a bare `@` alias.
 */
export function flowDesignerResolveAlias(studioRoot: string): Record<string, string> {
  const repoRoot = resolve(studioRoot, '../..')
  const webSrc = resolve(repoRoot, 'apps/web/src')
  const flowSrc = resolve(repoRoot, 'crates/flow/packages/ui/src')
  const formSrc = resolve(repoRoot, 'packages/form/src')
  const ceDist = resolve(repoRoot, 'packages/ce/dist')
  const phosphor = resolve(studioRoot, 'node_modules/@phosphor-icons/react')
  const shim = resolve(studioRoot, 'src/renderer/shims')
  return {
    '@a3s-lab/flow-ui/workflow-node-preview': resolve(flowSrc, 'react/workflow-node-preview.tsx'),
    '@a3s-lab/flow-ui/workflow-node.css': resolve(flowSrc, 'styles/workflow-node.css'),
    '@a3s-lab/flow-ui/node.css': resolve(flowSrc, 'styles/a3s-flow-node.css'),
    '@a3s-lab/flow-ui/react': resolve(flowSrc, 'react.ts'),
    '@a3s-lab/flow-ui': resolve(flowSrc, 'index.ts'),
    '@a3s-lab/form/styles.css': resolve(formSrc, 'styles.css'),
    '@a3s-lab/form/core': resolve(formSrc, 'core.ts'),
    '@a3s-lab/form/react': resolve(formSrc, 'react.tsx'),
    '@a3s-lab/form': resolve(formSrc, 'index.ts'),
    '@a3s-lab/ce/components/code-editor.css': resolve(ceDist, 'components/code-editor.css'),
    '@a3s-lab/ce/basecoat': resolve(ceDist, 'js/basecoat.js'),
    '@a3s-lab/ce/select': resolve(ceDist, 'js/select.js'),
    '@a3s-lab/ce/code-editor': resolve(ceDist, 'js/code-editor.js'),
    '@phosphor-icons/react': resolve(phosphor, 'dist/index.es.js'),
    '@xyflow/react': resolve(studioRoot, 'node_modules/@xyflow/react'),
    '@rspress/core/runtime': resolve(shim, 'rspress-runtime.ts'),
    '@/lib/a3sFlowFile': resolve(webSrc, 'lib/a3sFlowFile.ts'),
    '@/components/DetailPageNav': resolve(webSrc, 'components/DetailPageNav.tsx'),
    '@/i18n/useI18n': resolve(shim, 'console-i18n.ts'),
    '@/shell/shellBack': resolve(webSrc, 'shell/shellBack.tsx')
  }
}
