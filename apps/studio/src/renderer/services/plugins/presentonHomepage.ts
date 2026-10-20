import type { AixComposerMode } from '@shared/types/aixComposerMode'
import {
  aixWorkbenchCatalogueSchema,
  type AixWorkbenchCatalogue,
  type AixWorkbenchPackage
} from '@shared/types/aixWorkbench'

import aclSource from '../../../../../use-registry/packages/presenton/a3s-use-extension.acl?raw'

export const PRESENTON_PACKAGE_ID = 'use/a3s/presenton'
export const PRESENTON_TITLE = '演示文稿'

export const PRESENTON_SLUG = 'a3s-presenton'

function sliceBlock(source: string, from: number): { body: string; end: number } | null {
  const open = source.indexOf('{', from)
  if (open < 0) return null
  let depth = 0
  let quote: string | null = null
  for (let i = open; i < source.length; i += 1) {
    const ch = source[i]
    if (quote) {
      if (ch === '\\') {
        i += 1
        continue
      }
      if (ch === quote) quote = null
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      continue
    }
    if (ch === '{') depth += 1
    else if (ch === '}') {
      depth -= 1
      if (depth === 0) return { body: source.slice(open + 1, i), end: i + 1 }
    }
  }
  return null
}

function assigned(body: string, name: string): string | null {
  const match = new RegExp(`(?:^|\\n)\\s*${name}\\s*=\\s*`).exec(body)
  if (!match) return null
  let at = match.index + match[0].length
  while (body[at] === ' ' || body[at] === '\t') at += 1
  if (body[at] === '[') {
    const end = body.indexOf(']', at)
    return end < 0 ? null : body.slice(at, end + 1)
  }
  if (body[at] === '"') {
    let end = at + 1
    while (end < body.length && body[end] !== '"') {
      if (body[end] === '\\') end += 1
      end += 1
    }
    return body.slice(at, end + 1)
  }
  const word = /^[^\s]+/.exec(body.slice(at))
  return word?.[0] ?? null
}

function unquote(value: string | null): string {
  if (!value) return ''
  const match = /^"([\s\S]*)"$/.exec(value)
  return match?.[1] ?? value
}

function quotedList(value: string | null): string[] {
  if (!value) return []
  return [...value.matchAll(/"([^"\\]*)"/g)].map((item) => item[1] ?? '')
}

function composerModeFromAcl(source: string): Record<string, unknown> {
  const header = source.indexOf('composer_mode "slides"')
  if (header < 0) throw new Error('presenton composer mode is missing')
  const block = sliceBlock(source, header)
  if (!block) throw new Error('presenton composer mode is incomplete')
  const controls: Record<string, unknown>[] = []
  let from = 0
  while (from < block.body.length) {
    const match = /control "([^"]+)"/.exec(block.body.slice(from))
    if (!match?.[1]) break
    const at = from + match.index
    const control = sliceBlock(block.body, at)
    if (!control) break
    const kind = unquote(assigned(control.body, 'kind'))
    const initialRaw = assigned(control.body, 'initial')
    const initial =
      initialRaw === 'true' ? true : initialRaw === 'false' ? false : initialRaw ? unquote(initialRaw) : undefined
    const options = quotedList(assigned(control.body, 'options'))
    const optionLabels = quotedList(assigned(control.body, 'option_labels'))
    const min = assigned(control.body, 'min')
    const max = assigned(control.body, 'max')
    controls.push({
      id: match[1],
      kind,
      label: unquote(assigned(control.body, 'label')),
      ...(min ? { min: Number(min) } : {}),
      ...(max ? { max: Number(max) } : {}),
      ...(options.length > 0 ? { options } : {}),
      ...(optionLabels.length > 0 ? { optionLabels } : {}),
      ...(initial !== undefined ? { initial } : {})
    })
    from = control.end
  }
  const context = quotedList(assigned(block.body, 'context'))
  const editor = unquote(assigned(block.body, 'editor'))
  const kept = unquote(assigned(block.body, 'kept_notice'))
  const keptNotice = kept === 'draft' || kept === 'editor' ? kept : undefined
  return {
    id: 'slides',
    contractVersion: Number(assigned(block.body, 'contract_version')),
    title: PRESENTON_TITLE,
    description: unquote(assigned(block.body, 'description')),
    icon: unquote(assigned(block.body, 'icon')),
    agent: unquote(assigned(block.body, 'agent')),
    ...(editor ? { editor } : {}),
    ...(keptNotice ? { keptNotice } : {}),
    scope: unquote(assigned(block.body, 'scope')),
    context,
    order: Number(assigned(block.body, 'order')),
    prompt: unquote(assigned(block.body, 'prompt')),
    controls
  }
}

function workbenchViewFromAcl(source: string): Record<string, unknown> {
  const header = source.indexOf('ui "workbench"')
  if (header < 0) throw new Error('presenton workbench view is missing')
  const block = sliceBlock(source, header)
  if (!block) throw new Error('presenton workbench view is incomplete')
  const orderRaw = assigned(block.body, 'order')
  return {
    id: 'workbench',
    title: unquote(assigned(block.body, 'title')),
    description: unquote(assigned(block.body, 'description')),
    icon: unquote(assigned(block.body, 'icon')),
    order: orderRaw && /^\d+$/.test(orderRaw) ? Number(orderRaw) : 100,
    container: null,
    point: unquote(assigned(block.body, 'point')),
    presentation: unquote(assigned(block.body, 'presentation'))
  }
}

function presentonComposerPackage(): AixWorkbenchPackage {
  const parsed = aixWorkbenchCatalogueSchema.parse({
    protocolVersion: 1,
    revision: '',
    packages: [
      {
        slug: PRESENTON_SLUG,
        packageId: PRESENTON_PACKAGE_ID,
        generation: 1,
        revision: `sha256:${'cd'.repeat(32)}`,
        containers: [],
        views: [workbenchViewFromAcl(aclSource)],
        composerModes: [composerModeFromAcl(aclSource)]
      }
    ]
  })
  return parsed.packages[0]
}

const shipped = presentonComposerPackage()

function homepageMode(mode: AixComposerMode): AixComposerMode & { task?: true; taskPrompt?: string } {
  if (mode.id !== 'slides') return mode
  const fallback = shipped.composerModes?.[0]
  return {
    ...mode,
    title: PRESENTON_TITLE,
    editor: mode.editor ?? fallback?.editor,
    keptNotice: mode.keptNotice ?? fallback?.keptNotice,
    // The outline stream in the right-hand editor is the only model author.
    task: true,
    taskPrompt:
      '调用 mcp__presenton__generate_deck，把上面的正文作为 content，打开右侧预览。这不是 PPTX 文件。要读当前预览调用 mcp__presenton__read_deck；没有快照时停止轮询。要改一节大纲调用 mcp__presenton__edit_outline。要改一页正文和备注调用 mcp__presenton__edit_slide。成员要求交付 .pptx 时，使用 office_file 保存并读回校验文件。'
  }
}

function acceptsWindow(view: { container: string | null; presentation?: string; point: string }): boolean {
  return (
    view.container === null &&
    view.presentation !== 'tree' &&
    (view.point === 'studio.applets' || view.point === 'studio.editor')
  )
}

function viewsFor(current: AixWorkbenchPackage, editor: string | undefined) {
  const views = [...current.views]
  if (editor && views.some((view) => view.id === editor && acceptsWindow(view))) return views
  const shippedView = shipped.views.find((view) => view.id === editor && acceptsWindow(view))
  return shippedView ? [...views, shippedView] : views
}

function memberView<T extends { id: string; title: string }>(view: T): T {
  if (view.id !== 'workbench' || view.title === PRESENTON_TITLE) return view
  return { ...view, title: PRESENTON_TITLE }
}

/** An installed package keeps its homepage work mode. Uninstall drops the package, and the mode with it. */
export function withPresentonHomepage(catalogue: AixWorkbenchCatalogue): AixWorkbenchCatalogue {
  const index = catalogue.packages.findIndex(
    (item) => item.packageId === PRESENTON_PACKAGE_ID || item.slug === PRESENTON_SLUG
  )
  if (index < 0) return catalogue
  const current = catalogue.packages[index]
  if (!current) return catalogue
  const modes = (current.composerModes?.length ? current.composerModes : (shipped.composerModes ?? [])).map(
    homepageMode
  )
  const editor = modes?.find((mode) => mode.id === 'slides')?.editor
  const packages = catalogue.packages.slice()
  packages[index] = {
    ...current,
    views: viewsFor(current, editor).map(memberView),
    ...(modes ? { composerModes: modes } : {})
  }
  return { ...catalogue, packages }
}
