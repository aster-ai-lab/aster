import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { createRsbuild } from '@rsbuild/core'
import { describe, expect, it } from 'vitest'

import config from '../../rsbuild.config'

const ACL_SOURCE = 'extension "a3s/fixture" {\n  marker = "acl-raw-source"\n}\n'

function matches(condition: unknown, filename: string): boolean {
  if (condition instanceof RegExp) return condition.test(filename)
  if (Array.isArray(condition)) return condition.some((item) => matches(item, filename))
  if (condition && typeof condition === 'object' && 'and' in condition) {
    const group = condition as { and?: unknown; not?: unknown }
    const accepted = group.and == null || matches(group.and, filename)
    const rejected = group.not != null && matches(group.not, filename)
    return accepted && !rejected
  }
  return false
}

async function javascriptOutput(root: string): Promise<string> {
  const files: string[] = []
  const walk = async (dir: string) => {
    const entries = await readdir(dir, { withFileTypes: true })
    for (const entry of entries) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) await walk(path)
      else if (entry.name.endsWith('.js')) files.push(path)
    }
  }
  await walk(root)
  const bodies = await Promise.all(files.map((path) => readFile(path, 'utf8')))
  return bodies.join('\n')
}

describe('acl raw import', () => {
  it('keeps an acl file out of the script parser and emits its source from ?raw', async () => {
    const include = config.source?.assetsInclude
    expect(matches(include, 'packages/presenton/a3s-use-extension.acl')).toBe(true)
    expect(matches(include, 'feature.ACL')).toBe(false)
    expect(matches(include, 'presentonHomepage.ts')).toBe(false)
    expect(matches(include, 'notes.md')).toBe(false)

    const root = await mkdtemp(join(tmpdir(), 'acl-raw-'))
    try {
      await writeFile(join(root, 'feature.acl'), ACL_SOURCE)
      await writeFile(join(root, 'index.js'), 'import source from "./feature.acl?raw"\nexport default source\n')
      const rsbuild = await createRsbuild({
        cwd: root,
        config: {
          source: {
            entry: { index: './index.js' },
            assetsInclude: include
          },
          output: {
            distPath: { root: 'dist' },
            cleanDistPath: true,
            legalComments: 'none'
          },
          performance: { printFileSize: false },
          dev: { progressBar: false }
        }
      })
      const result = await rsbuild.build()
      await result.close()
      const bundled = await javascriptOutput(join(root, 'dist'))
      expect(bundled).toContain(JSON.stringify(ACL_SOURCE))
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  }, 60000)

  it('emits an acl file that lives outside the compilation root', async () => {
    const parent = await mkdtemp(join(tmpdir(), 'acl-raw-outside-'))
    const root = join(parent, 'app')
    const sourceFile = join(parent, 'feature.acl')
    try {
      await mkdir(root)
      await writeFile(sourceFile, ACL_SOURCE)
      await writeFile(
        join(root, 'index.js'),
        `import source from ${JSON.stringify(`${sourceFile}?raw`)}\nexport default source\n`
      )
      const rsbuild = await createRsbuild({
        cwd: root,
        config: {
          source: {
            entry: { index: './index.js' },
            assetsInclude: config.source?.assetsInclude
          },
          tools: { rspack: config.tools?.rspack },
          output: {
            distPath: { root: 'dist' },
            cleanDistPath: true,
            legalComments: 'none'
          },
          performance: { printFileSize: false },
          dev: { progressBar: false }
        }
      })
      const result = await rsbuild.build()
      await result.close()
      const bundled = await javascriptOutput(join(root, 'dist'))
      expect(bundled).toContain(JSON.stringify(ACL_SOURCE))
    } finally {
      await rm(parent, { recursive: true, force: true })
    }
  }, 60000)
})
