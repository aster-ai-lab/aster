import { resolve } from 'node:path'

import { ESLint } from 'eslint'
import ts from 'typescript'
import { describe, expect, it } from 'vitest'

const eslint = new ESLint()
const ruleId = 'import-x/no-restricted-paths'

async function diagnostics(importer: string, imported: string) {
  const [result] = await eslint.lintText(`import '${imported}'`, { filePath: `src/renderer/${importer}` })
  expect(result.messages.filter((message) => message.fatal)).toEqual([])
  return result.messages.filter((message) => message.ruleId === ruleId)
}

describe('renderer foundation boundary', () => {
  it('accepts TypeScript browser declarations and still rejects unknown names', async () => {
    const valid = "export const title: Document['title'] = document.title"
    const [typed] = await eslint.lintText(valid, { filePath: 'src/renderer/data/lintProbe.ts' })
    expect(typed.messages.filter((message) => message.severity === 2)).toEqual([])
    const [javascript] = await eslint.lintText('export const value = metaStudioMissingHost', {
      filePath: 'src/renderer/data/lintProbe.js'
    })
    expect(javascript.messages).toContainEqual(expect.objectContaining({ ruleId: 'no-undef', severity: 2 }))

    const file = resolve('src/renderer/data/lintProbe.ts')
    const options: ts.CompilerOptions = {
      noEmit: true,
      strict: true,
      skipLibCheck: true,
      types: [],
      target: ts.ScriptTarget.ESNext,
      lib: ['lib.esnext.d.ts', 'lib.dom.d.ts']
    }
    const host = ts.createCompilerHost(options)
    const readSource = host.getSourceFile
    host.getSourceFile = (name, languageVersion, onError, shouldCreateNewSourceFile) =>
      name === file
        ? ts.createSourceFile(name, `${valid}; export const invalid = metaStudioMissingHost`, ts.ScriptTarget.ESNext)
        : readSource(name, languageVersion, onError, shouldCreateNewSourceFile)
    const program = ts.createProgram([file], options, host)
    expect(program.getSemanticDiagnostics().map((diagnostic) => diagnostic.code)).toEqual([2304])
  })

  it.each(['data/probe.ts', 'ipc/probe.ts', 'workers/probe.ts'])(
    'blocks UI and use-case imports in %s',
    async (file) => {
      for (const imported of ['@renderer/components/app/BossRoomRuntime', '@renderer/services/harnessAssemble']) {
        expect(await diagnostics(file, imported)).toEqual([expect.objectContaining({ severity: 2 })])
      }
    }
  )

  it('permits peer infrastructure and the logger facade', async () => {
    for (const imported of ['@renderer/data/memberOrgs', '@renderer/workers/shikiStreamTokenizer', '@logger']) {
      expect(await diagnostics('data/probe.ts', imported)).toEqual([])
    }
  })

  it('keeps resident runtime state below page composition', async () => {
    expect(await diagnostics('services/probe.ts', '@renderer/pages/messages/MessagesPage')).toEqual([
      expect.objectContaining({ severity: 2 })
    ])
    expect(await diagnostics('components/app/probe.ts', '@renderer/services/tobotStream')).toEqual([])
    expect(await diagnostics('components/app/probe.ts', '@renderer/services/bossRoom')).toEqual([])
  })
})
