import assert from 'node:assert/strict'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { LANGUAGES, TONES, VERBOSITIES } from '../agent/deck.mjs'
import { armDeckAutostart } from '../tools/deck-autostart.mjs'
import { ICON_NAMES } from '../agent/icons.mjs'
import { TEMPLATES as AGENT_TEMPLATES } from '../agent/templates.mjs'
import { ICONS } from '../view/vendor/phosphor/icons.js'
import general from '../view/vendor/templates/general.js'
import modern from '../view/vendor/templates/modern.js'
import standard from '../view/vendor/templates/standard.js'
import swift from '../view/vendor/templates/swift.js'

const root = fileURLToPath(new URL('..', import.meta.url))
const SOURCE_ONLY = ['brand/', 'tests/', 'tools/', 'view/']
/** Hand-written package files; everything under ui/workbench is built by tools/. */
const AUTHORED = [
  'README.md',
  'aster-use-extension.acl',
  'agent/deck.mjs',
  'agent/icons.mjs',
  'agent/languages.mjs',
  'agent/run.mjs',
  'agent/templates.mjs',
  'agents/presenton/INSTRUCTIONS.md',
  'skills/presenton/SKILL.md'
]
const VIEW_TEMPLATES = [general, modern, standard, swift]
const FILE_MAX_BYTES = 8 * 1024 * 1024
const COVER_MAX_BYTES = 2 * 1024 * 1024
/** Studio serves a view file only with one of these types (apps/studio/src-tauri/src/authority.rs). */
const VIEW_TYPES = /\.(?:html|js|mjs|css|json|svg|png|jpe?g|webp|woff2?|wasm)$/
/** Notices shipped beside the files they cover; the view never loads them. */
const NOTICE = /(?:^|\/)(?:LICENSE[^/]*|UPSTREAM|THIRD_PARTY_NOTICES\.md|[^/]+-LICENSE\.md)$/

function files(directory) {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name)
    return statSync(path).isDirectory() ? files(path) : [relative(root, path).split('\\').join('/')]
  })
}

const read = (path) => readFileSync(join(root, path), 'utf8')
const VIEW_FILES = files(join(root, 'ui/workbench')).sort()
const SHIPPED = [...AUTHORED, ...VIEW_FILES].sort()

test('the package ships the authored files and the built view, each under the API file limit', () => {
  const shipped = files(root)
    .filter((path) => !path.startsWith('.') && !SOURCE_ONLY.some((prefix) => path.startsWith(prefix)))
    .sort()
  assert.deepEqual(shipped, SHIPPED)
  for (const path of SHIPPED) assert.ok(statSync(join(root, path)).size <= FILE_MAX_BYTES, path)
  for (const path of ['index.html', 'app.js', 'app.css', 'fonts/ui.css', 'fonts/fonts.css', 'THIRD_PARTY_NOTICES.md', 'vendor/presenton/LICENSE']) {
    assert.ok(VIEW_FILES.includes(`ui/workbench/${path}`), path)
  }
})

test('the API seed lists the same files', () => {
  const seed = readFileSync(join(root, '../../../api/src/contexts/plugin/infrastructure/presenton_package.rs'), 'utf8')
  const listed = [...seed.slice(seed.indexOf('package_files!')).matchAll(/^\s*"([^"]+)",?$/gm)].map((match) => match[1])
  assert.deepEqual([...listed].sort(), SHIPPED)
})

test('the manifest declares schema 3 with an edge agent and a homepage composer mode', () => {
  const manifest = read('aster-use-extension.acl')
  assert.match(manifest, /^extension "aster\/presenton" \{$/m)
  assert.match(manifest, /^\s*schema_version\s*=\s*3$/m)
  assert.match(manifest, /^\s*harness\s*=\s*"package"$/m)
  assert.match(manifest, /^\s*runtime\s*=\s*"edge"$/m)
  assert.match(manifest, /^\s*entry\s*=\s*"agent\/run\.mjs"$/m)
  assert.match(manifest, /composer_mode "slides"/)
  assert.match(manifest, /title = "幻灯片"/)
  assert.match(manifest, /editor = "workbench"/)
  assert.match(manifest, /ui "workbench"/)
  assert.match(manifest, /point\s*=\s*"studio\.applets"/)
  assert.doesNotMatch(manifest, /view_container/)
  for (const path of manifest.matchAll(/"((?:agent|agents|skills|ui)\/[^"]+)"/g)) {
    assert.ok(SHIPPED.includes(path[1]) || path[1] === 'ui/workbench', path[1])
  }
})

test('the skill and the instructions are present with the skill front matter', () => {
  const skill = read('skills/presenton/SKILL.md')
  assert.match(skill, /^---\nname: presenton\ndescription: .+\nversion: 1\.0\.0\n---\n/)
  assert.ok(read('agents/presenton/INSTRUCTIONS.md').trim().length > 0)
})

test('the cover is a JPEG inside the catalog limit', () => {
  const cover = readFileSync(join(root, 'brand/cover.jpg'))
  assert.deepEqual([...cover.subarray(0, 3)], [0xff, 0xd8, 0xff])
  assert.ok(cover.length > 10_000)
  assert.ok(cover.length <= COVER_MAX_BYTES)
})

test('every view file has a type Studio serves, and every file the view names exists', () => {
  for (const path of VIEW_FILES) if (!NOTICE.test(path)) assert.match(path, VIEW_TYPES, path)
  const html = read('ui/workbench/index.html')
  for (const [, path] of html.matchAll(/(?:src|href)="([^"]+)"/g)) assert.ok(VIEW_FILES.includes(`ui/workbench/${path}`), `index.html names ${path}`)
  for (const sheet of ['app.css', 'fonts/ui.css', 'fonts/fonts.css']) {
    const base = sheet.includes('/') ? sheet.slice(0, sheet.lastIndexOf('/') + 1) : ''
    for (const [, url] of read(`ui/workbench/${sheet}`).matchAll(/url\(\s*['"]?([^'")]+)/g)) {
      if (url.startsWith('data:')) continue
      assert.ok(VIEW_FILES.includes(`ui/workbench/${base}${url.replace(/^\.\//, '')}`), `${sheet} names ${url}`)
    }
  }
  const app = read('ui/workbench/app.js')
  for (const [, path] of app.matchAll(/["'`]((?:public|vendor|assets)\/[\w./\[\],-]+\.(?:svg|png|jpe?g|webp|js|woff2))["'`]/g)) {
    if (path.startsWith('public/providers/')) continue
    assert.ok(VIEW_FILES.includes(`ui/workbench/${path}`), `app.js names ${path}`)
  }
  for (const template of VIEW_TEMPLATES) {
    for (const [, path] of JSON.stringify(template).matchAll(/"(vendor\/[^"]+\.(?:png|svg|jpe?g|webp))"/g)) {
      assert.ok(VIEW_FILES.includes(`ui/workbench/${path}`), `${template.id} uses ${path}`)
    }
  }
})

test('the view runs under the Studio CSP and loads nothing remote', () => {
  const page = read('ui/workbench/index.html') + read('ui/workbench/app.css') + read('ui/workbench/chrome.css') + read('ui/workbench/fonts/fonts.css') + read('ui/workbench/fonts/ui.css')
  assert.doesNotMatch(page, /(?:src|href)\s*=\s*"(?:https?:)?\/\//)
  assert.doesNotMatch(page, /url\(\s*['"]?(?:https?:)?\/\//)
  for (const path of VIEW_FILES.filter((p) => p.endsWith('.js'))) {
    // script-src has no 'unsafe-eval'.
    assert.doesNotMatch(read(path), /\beval\(|new Function\b|[^\w.$]Function\(|\bimport\(/, path)
  }
  for (const path of ['agent/run.mjs', 'agent/deck.mjs']) {
    assert.doesNotMatch(read(path), /\bfetch\(|https?:\/\/|Authorization|Bearer|API_KEY/i, path)
  }
})

test('a fresh outline uses the general template and starts the slide stream once', () => {
  const picker =
    '[d,u]=(0,Yl.useState)(!0),[f,m]=(0,Yl.useState)(null),[g,y]=(0,Yl.useState)(s?lue(s):UPe)'
  const effect = '(0,Yl.useEffect)(()=>{if(!l||!S){_(!1);return}H&&_(!0)},[S,H,l]);'
  const next = armDeckAutostart(`before ${picker} mid ${effect} after`)
  assert.match(next, /useState\)\(!1\),\[f,m\]=\(0,Yl\.useState\)\(a\|\|"general"\)/)
  assert.match(next, /let deckAuto=\(0,Yl\.useRef\)\(!1\)/)
  assert.match(next, /statusMessage!=="\\u5927\\u7EB2\\u5DF2\\u751F\\u6210"/)
  assert.throws(() => armDeckAutostart(next))
  const shipped = read('ui/workbench/app.js')
  assert.match(shipped, /a\|\|"general"/)
  assert.match(shipped, /let deckAuto=\(0,Yl\.useRef\)\(!1\)/)
  assert.equal(shipped.split('let deckAuto=').length - 1, 1)
})

test('editor chrome uses the product ink, field, and selected row', () => {
  const chrome = read('ui/workbench/chrome.css')
  assert.match(read('ui/workbench/index.html'), /href="chrome\.css"/)
  assert.match(chrome, /#171717/)
  assert.match(chrome, /#f2f3f5/)
  assert.match(chrome, /#f5f5f5/)
  assert.match(chrome, /logo-with-bg/)
  assert.match(chrome, /不在「文件」，只保存在当前任务/)
  assert.match(chrome, /button\[style\*='linear-gradient'\]/)
  assert.match(chrome, /button\[role='combobox'\]/)
  assert.match(chrome, /\.animate-pulse > div/)
  assert.doesNotMatch(chrome, /#7A5AF8/i)
  const image = readFileSync(join(root, 'ui/workbench/vendor/presenton/template-image.png'))
  const logo = readFileSync(join(root, 'ui/workbench/public/logo-with-bg.png'))
  assert.equal(image.subarray(0, 8).toString('hex'), '89504e470d0a1a0a')
  assert.ok(image.length < 8000)
  assert.equal(logo.subarray(0, 8).toString('hex'), '89504e470d0a1a0a')
  assert.ok(logo.length < 200)
})

function controlBody(acl, id) {
  const start = acl.indexOf(`control "${id}" {`)
  assert.ok(start >= 0, id)
  const next = acl.indexOf('\n    control ', start + 10)
  return acl.slice(start, next < 0 ? acl.length : next)
}

function quotedList(body, name) {
  const match = body.match(new RegExp(`${name} = \\[([\\s\\S]*?)\\]`))
  assert.ok(match, name)
  return [...match[1].matchAll(/"([^"\\]*)"/g)].map((item) => item[1])
}

test('the slides composer mode lists the outline closed sets and not the templates', () => {
  const manifest = read('aster-use-extension.acl')
  assert.match(manifest, /composer_mode "slides"/)
  assert.match(manifest, /kept_notice = "draft"/)
  const languageOptions = quotedList(controlBody(manifest, 'language'), 'options')
  const languageLabels = quotedList(controlBody(manifest, 'language'), 'option_labels')
  assert.deepEqual(languageOptions, LANGUAGES)
  assert.equal(languageLabels.length, languageOptions.length)
  for (const label of languageLabels) assert.ok([...label].length <= 32, label)
  const languagePairs = [
    ['自动（英语）', 'Auto (English)'],
    ['英语', 'English'],
    ['简体中文', 'Chinese (Simplified - 中文, 汉语)'],
    ['繁体中文', 'Chinese (Traditional - 中文, 漢語)']
  ]
  for (const [label, option] of languagePairs) {
    assert.equal(languageOptions[languageLabels.indexOf(label)], option)
  }
  assert.deepEqual(quotedList(controlBody(manifest, 'tone'), 'options'), TONES)
  assert.deepEqual(quotedList(controlBody(manifest, 'verbosity'), 'options'), VERBOSITIES)
  assert.deepEqual(quotedList(controlBody(manifest, 'tone'), 'option_labels'), ['默认', '轻松', '专业', '幽默', '教学', '销售推介'])
  assert.doesNotMatch(manifest, /control "template"/)
  for (const id of ['general', 'modern', 'swift']) assert.equal(manifest.includes(`"${id}"`), false)
})

test('the agent and the view share the templates, layouts, and icon names', () => {
  assert.deepEqual(Object.keys(AGENT_TEMPLATES).sort(), VIEW_TEMPLATES.map((template) => template.id).sort())
  for (const template of VIEW_TEMPLATES) {
    assert.deepEqual(
      AGENT_TEMPLATES[template.id].layouts.map((layout) => layout.id),
      template.layouts.map((layout) => layout.id),
      template.id
    )
    for (const layout of template.layouts) assert.equal(layout.repeated?.length, layout.components.length, `${template.id}/${layout.id}`)
  }
  assert.equal(ICON_NAMES.length, 1512)
  assert.deepEqual([...ICON_NAMES].sort(), Object.keys(ICONS).sort())
})
