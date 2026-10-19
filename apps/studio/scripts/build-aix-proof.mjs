import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import process from 'node:process'
import { URL } from 'node:url'

import JSZip from 'jszip'

const destination = process.argv[2]
if (!destination?.endsWith('.aix')) throw new Error('Provide an output .aix path')
const mode = process.argv[3] ?? 'workbench'
if (
  ![
    'workbench',
    'composer',
    'composer-execution',
    'context-visibility',
    'view-commands',
    'view-contract',
    'unknown-field',
    'unsupported-manifest-schema',
    'unsupported-contract-version',
    'unsupported-port-version',
    'missing-dependency',
    'duplicate-contribution',
    'dependency-cycle',
    'path-traversal',
    'digest-substitution',
    'cross-package-target',
    'missing-service',
    'acp-cloud'
  ].includes(mode)
)
  throw new Error('Unknown AIX proof mode')
const workbench = mode === 'workbench' || mode === 'context-visibility'
const workbenchFixture = mode === 'context-visibility' ? mode : 'workbench-package'
const composerFixture = mode === 'composer-execution' ? 'composer-execution' : 'composer-normalize'
const fixture = (name) => new URL(`../tests/fixtures/aix/${name}`, import.meta.url)
const files = new Map(
  !workbench
    ? [
        ['a3s-use-extension.acl', fixture(`${composerFixture}/a3s-use-extension.acl`)],
        ['README.md', fixture(`${composerFixture}/README.md`)],
        ['index.html', fixture('composer-normalize/index.html')],
        ['provider.js', fixture('composer-normalize/provider.js')]
      ]
    : [
        ['a3s-use-extension.acl', fixture(`${workbenchFixture}/a3s-use-extension.acl`)],
        ['README.md', fixture(`${workbenchFixture}/README.md`)],
        ['skills/workbench/SKILL.md', fixture('workbench-package/SKILL.md')],
        ['ui/tasks/index.html', fixture('tree-provider/index.html')],
        ['ui/tasks/provider.js', fixture('tree-provider/provider.js')]
      ]
)
for (const surface of workbench ? ['overview', 'output', 'editor', 'app'] : []) {
  files.set(`ui/${surface}/index.html`, fixture('miniapp-environment/index.html'))
  files.set(`ui/${surface}/app.js`, fixture('miniapp-environment/app.js'))
}
if (
  mode === 'view-commands' ||
  mode === 'view-contract' ||
  mode === 'unknown-field' ||
  mode === 'unsupported-manifest-schema' ||
  mode === 'unsupported-contract-version' ||
  mode === 'unsupported-port-version' ||
  mode === 'missing-dependency' ||
  mode === 'duplicate-contribution' ||
  mode === 'dependency-cycle' ||
  mode === 'path-traversal' ||
  mode === 'digest-substitution' ||
  mode === 'cross-package-target' ||
  mode === 'missing-service'
) {
  files.clear()
  const fixtureMode = [
    'unknown-field',
    'unsupported-manifest-schema',
    'unsupported-contract-version',
    'unsupported-port-version',
    'missing-dependency',
    'duplicate-contribution',
    'dependency-cycle',
    'path-traversal',
    'digest-substitution',
    'cross-package-target',
    'missing-service'
  ].includes(mode)
    ? 'view-contract'
    : mode
  files.set('a3s-use-extension.acl', fixture(`${fixtureMode}/a3s-use-extension.acl`))
  files.set('README.md', fixture(`${fixtureMode}/README.md`))
  files.set('skills/workbench/SKILL.md', fixture('workbench-package/SKILL.md'))
  for (const surface of ['left', 'right', 'tasks', 'app']) {
    files.set(`ui/${surface}/index.html`, fixture('view-commands/index.html'))
    files.set(`ui/${surface}/app.js`, fixture('view-commands/app.js'))
  }
  if (mode === 'missing-service') files.set('ui/workspace/index.html', fixture('miniapp-environment/index.html'))
}
if (mode === 'acp-cloud') {
  files.clear()
  files.set('a3s-use-extension.acl', fixture('acp-cloud-agent/a3s-use-extension.acl'))
  files.set('README.md', fixture('acp-cloud-agent/README.md'))
  files.set('skills/acp-cloud/SKILL.md', fixture('acp-cloud-agent/skills/acp-cloud/SKILL.md'))
  files.set('agent/acp-agent.mjs', fixture('acp-cloud-agent/agent.mjs'))
  files.set('client/install-and-use.mjs', fixture('acp-cloud-agent/client/install-and-use.mjs'))
}
const zip = new JSZip()
const date = new Date('2000-01-01T00:00:00.000Z')
for (const [name, source] of [...files].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
  let contents = await readFile(source)
  if (mode === 'unknown-field' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const accepted = 'menu "view.title" { command = "refresh" order = 10 }'
    if (!acl.includes(accepted)) throw new Error('Unknown-field mutation anchor was not found')
    contents = Buffer.from(
      acl.replace(accepted, 'menu "view.title" { command = "refresh" order = 10 unsupported = true }')
    )
  }
  if (mode === 'unsupported-manifest-schema' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    if (!acl.includes('schema_version = 3')) throw new Error('Manifest-schema mutation anchor was not found')
    contents = Buffer.from(acl.replace('schema_version = 3', 'schema_version = 2'))
  }
  if (mode === 'unsupported-contract-version' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    if (!acl.includes('contract_version = 1')) throw new Error('Contract-version mutation anchor was not found')
    contents = Buffer.from(acl.replace('contract_version = 1', 'contract_version = 99'))
  }
  if (mode === 'unsupported-port-version' && name.endsWith('/app.js')) {
    const script = contents.toString('utf8')
    const handshake = "channel.port1.postMessage({ protocolVersion: 1, type: 'aix.command.ready' })"
    if (!script.includes(handshake)) throw new Error('Port-version mutation anchor was not found')
    contents = Buffer.from(script.replace(handshake, handshake.replace('protocolVersion: 1', 'protocolVersion: 2')))
  }
  if (mode === 'duplicate-contribution' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const contribution = 'command "refresh" { title = "Reload left" enablement { theme = "light" } }'
    if (!acl.includes(contribution)) throw new Error('Duplicate-contribution mutation anchor was not found')
    contents = Buffer.from(acl.replace(contribution, `${contribution}\n    ${contribution}`))
  }
  if (mode === 'dependency-cycle' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const anchor = 'repository { url = "https://example.org/proof/view-commands" }'
    if (!acl.includes(anchor)) throw new Error('Dependency-cycle mutation anchor was not found')
    contents = Buffer.from(acl.replace(anchor, `${anchor}\n  dependency "proof/view-commands" { version = "^1.0.0" }`))
  }
  if (mode === 'missing-dependency' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const anchor = 'repository { url = "https://example.org/proof/view-commands" }'
    if (!acl.includes(anchor)) throw new Error('Missing-dependency mutation anchor was not found')
    contents = Buffer.from(acl.replace(anchor, `${anchor}\n  dependency "proof/missing-base" { version = "^1.0.0" }`))
  }
  if (mode === 'cross-package-target' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const anchor = 'root = "ui/left" container = "resources"'
    if (!acl.includes(anchor)) throw new Error('Cross-package target mutation anchor was not found')
    contents = Buffer.from(acl.replace(anchor, 'root = "ui/left" container = "foreign-resources"'))
  }
  if (mode === 'missing-service' && name === 'a3s-use-extension.acl') {
    const acl = contents.toString('utf8')
    const closing = acl.lastIndexOf('}')
    if (closing < 0) throw new Error('Missing-service mutation anchor was not found')
    contents = Buffer.from(
      `${acl.slice(0, closing)}  ui "workspace" { title = "Workspace Integration" entry = "ui/workspace/index.html" point = "studio.workspace" contract_version = 1 mode = "append" }\n${acl.slice(closing)}`
    )
  }
  zip.file(name, contents, { date, createFolders: false })
}
const bytes = await zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE', compressionOptions: { level: 9 } })
if (mode === 'path-traversal') {
  const entry = Buffer.from('ui/left/app.js')
  const traversal = Buffer.from('../left/app.js')
  if (entry.length !== traversal.length) throw new Error('Path-traversal mutation must preserve ZIP header lengths')
  let offset = 0
  let replacements = 0
  while ((offset = bytes.indexOf(entry, offset)) !== -1) {
    traversal.copy(bytes, offset)
    offset += traversal.length
    replacements += 1
  }
  if (replacements < 2) throw new Error(`Expected both ZIP path records, found ${replacements}`)
}
await writeFile(destination, bytes)
process.stdout.write(`sha256:${createHash('sha256').update(bytes).digest('hex')} ${bytes.length} bytes\n`)
