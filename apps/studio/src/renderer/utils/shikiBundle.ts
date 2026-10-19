import { createBundledHighlighter } from 'shiki/core'
import type { BundledThemeInfo, DynamicImportLanguageRegistration, DynamicImportThemeRegistration } from 'shiki/core'
import { createOnigurumaEngine } from 'shiki/engine/oniguruma'

/**
 * Web grammars plus Rust and Go. Themes are the ones the viewer,
 * task fences, and the settings list actually offer. The full Shiki
 * theme catalog is not shipped.
 */

const lang_angular_html: DynamicImportLanguageRegistration = () => import('@shikijs/langs/angular-html')
const lang_angular_ts: DynamicImportLanguageRegistration = () => import('@shikijs/langs/angular-ts')
const lang_astro: DynamicImportLanguageRegistration = () => import('@shikijs/langs/astro')
const lang_blade: DynamicImportLanguageRegistration = () => import('@shikijs/langs/blade')
const lang_c: DynamicImportLanguageRegistration = () => import('@shikijs/langs/c')
const lang_coffee: DynamicImportLanguageRegistration = () => import('@shikijs/langs/coffee')
const lang_cpp: DynamicImportLanguageRegistration = () => import('@shikijs/langs/cpp')
const lang_css: DynamicImportLanguageRegistration = () => import('@shikijs/langs/css')
const lang_csv: DynamicImportLanguageRegistration = () => import('@shikijs/langs/csv')
const lang_glsl: DynamicImportLanguageRegistration = () => import('@shikijs/langs/glsl')
const lang_graphql: DynamicImportLanguageRegistration = () => import('@shikijs/langs/graphql')
const lang_haml: DynamicImportLanguageRegistration = () => import('@shikijs/langs/haml')
const lang_handlebars: DynamicImportLanguageRegistration = () => import('@shikijs/langs/handlebars')
const lang_html: DynamicImportLanguageRegistration = () => import('@shikijs/langs/html')
const lang_html_derivative: DynamicImportLanguageRegistration = () => import('@shikijs/langs/html-derivative')
const lang_http: DynamicImportLanguageRegistration = () => import('@shikijs/langs/http')
const lang_hurl: DynamicImportLanguageRegistration = () => import('@shikijs/langs/hurl')
const lang_imba: DynamicImportLanguageRegistration = () => import('@shikijs/langs/imba')
const lang_java: DynamicImportLanguageRegistration = () => import('@shikijs/langs/java')
const lang_javascript: DynamicImportLanguageRegistration = () => import('@shikijs/langs/javascript')
const lang_jinja: DynamicImportLanguageRegistration = () => import('@shikijs/langs/jinja')
const lang_jison: DynamicImportLanguageRegistration = () => import('@shikijs/langs/jison')
const lang_json: DynamicImportLanguageRegistration = () => import('@shikijs/langs/json')
const lang_json5: DynamicImportLanguageRegistration = () => import('@shikijs/langs/json5')
const lang_jsonc: DynamicImportLanguageRegistration = () => import('@shikijs/langs/jsonc')
const lang_jsonl: DynamicImportLanguageRegistration = () => import('@shikijs/langs/jsonl')
const lang_jsx: DynamicImportLanguageRegistration = () => import('@shikijs/langs/jsx')
const lang_julia: DynamicImportLanguageRegistration = () => import('@shikijs/langs/julia')
const lang_less: DynamicImportLanguageRegistration = () => import('@shikijs/langs/less')
const lang_markdown: DynamicImportLanguageRegistration = () => import('@shikijs/langs/markdown')
const lang_marko: DynamicImportLanguageRegistration = () => import('@shikijs/langs/marko')
const lang_mdc: DynamicImportLanguageRegistration = () => import('@shikijs/langs/mdc')
const lang_mdx: DynamicImportLanguageRegistration = () => import('@shikijs/langs/mdx')
const lang_php: DynamicImportLanguageRegistration = () => import('@shikijs/langs/php')
const lang_postcss: DynamicImportLanguageRegistration = () => import('@shikijs/langs/postcss')
const lang_pug: DynamicImportLanguageRegistration = () => import('@shikijs/langs/pug')
const lang_python: DynamicImportLanguageRegistration = () => import('@shikijs/langs/python')
const lang_r: DynamicImportLanguageRegistration = () => import('@shikijs/langs/r')
const lang_regexp: DynamicImportLanguageRegistration = () => import('@shikijs/langs/regexp')
const lang_sass: DynamicImportLanguageRegistration = () => import('@shikijs/langs/sass')
const lang_scss: DynamicImportLanguageRegistration = () => import('@shikijs/langs/scss')
const lang_shellscript: DynamicImportLanguageRegistration = () => import('@shikijs/langs/shellscript')
const lang_sql: DynamicImportLanguageRegistration = () => import('@shikijs/langs/sql')
const lang_stylus: DynamicImportLanguageRegistration = () => import('@shikijs/langs/stylus')
const lang_svelte: DynamicImportLanguageRegistration = () => import('@shikijs/langs/svelte')
const lang_ts_tags: DynamicImportLanguageRegistration = () => import('@shikijs/langs/ts-tags')
const lang_tsx: DynamicImportLanguageRegistration = () => import('@shikijs/langs/tsx')
const lang_typescript: DynamicImportLanguageRegistration = () => import('@shikijs/langs/typescript')
const lang_vue: DynamicImportLanguageRegistration = () => import('@shikijs/langs/vue')
const lang_vue_html: DynamicImportLanguageRegistration = () => import('@shikijs/langs/vue-html')
const lang_vue_vine: DynamicImportLanguageRegistration = () => import('@shikijs/langs/vue-vine')
const lang_wasm: DynamicImportLanguageRegistration = () => import('@shikijs/langs/wasm')
const lang_wgsl: DynamicImportLanguageRegistration = () => import('@shikijs/langs/wgsl')
const lang_wit: DynamicImportLanguageRegistration = () => import('@shikijs/langs/wit')
const lang_xml: DynamicImportLanguageRegistration = () => import('@shikijs/langs/xml')
const lang_yaml: DynamicImportLanguageRegistration = () => import('@shikijs/langs/yaml')
const lang_rust: DynamicImportLanguageRegistration = () => import('@shikijs/langs/rust')
const lang_go: DynamicImportLanguageRegistration = () => import('@shikijs/langs/go')

export const bundledLanguages: Record<string, DynamicImportLanguageRegistration> = {
  'angular-html': lang_angular_html,
  'angular-ts': lang_angular_ts,
  astro: lang_astro,
  blade: lang_blade,
  c: lang_c,
  coffee: lang_coffee,
  coffeescript: lang_coffee,
  cpp: lang_cpp,
  'c++': lang_cpp,
  css: lang_css,
  csv: lang_csv,
  glsl: lang_glsl,
  graphql: lang_graphql,
  gql: lang_graphql,
  haml: lang_haml,
  handlebars: lang_handlebars,
  hbs: lang_handlebars,
  html: lang_html,
  'html-derivative': lang_html_derivative,
  http: lang_http,
  hurl: lang_hurl,
  imba: lang_imba,
  java: lang_java,
  javascript: lang_javascript,
  js: lang_javascript,
  cjs: lang_javascript,
  mjs: lang_javascript,
  jinja: lang_jinja,
  jison: lang_jison,
  json: lang_json,
  json5: lang_json5,
  jsonc: lang_jsonc,
  jsonl: lang_jsonl,
  jsx: lang_jsx,
  julia: lang_julia,
  jl: lang_julia,
  less: lang_less,
  markdown: lang_markdown,
  md: lang_markdown,
  marko: lang_marko,
  mdc: lang_mdc,
  mdx: lang_mdx,
  php: lang_php,
  postcss: lang_postcss,
  pug: lang_pug,
  jade: lang_pug,
  python: lang_python,
  py: lang_python,
  r: lang_r,
  regexp: lang_regexp,
  regex: lang_regexp,
  sass: lang_sass,
  scss: lang_scss,
  shellscript: lang_shellscript,
  bash: lang_shellscript,
  sh: lang_shellscript,
  shell: lang_shellscript,
  zsh: lang_shellscript,
  sql: lang_sql,
  stylus: lang_stylus,
  styl: lang_stylus,
  svelte: lang_svelte,
  'ts-tags': lang_ts_tags,
  lit: lang_ts_tags,
  tsx: lang_tsx,
  typescript: lang_typescript,
  ts: lang_typescript,
  cts: lang_typescript,
  mts: lang_typescript,
  vue: lang_vue,
  'vue-html': lang_vue_html,
  'vue-vine': lang_vue_vine,
  wasm: lang_wasm,
  wgsl: lang_wgsl,
  wit: lang_wit,
  xml: lang_xml,
  yaml: lang_yaml,
  yml: lang_yaml,
  rust: lang_rust,
  go: lang_go
}

export const bundledLanguagesInfo = [
  { id: 'angular-html', name: 'Angular HTML', import: lang_angular_html },
  { id: 'angular-ts', name: 'Angular TypeScript', import: lang_angular_ts },
  { id: 'astro', name: 'Astro', import: lang_astro },
  { id: 'blade', name: 'Blade', import: lang_blade },
  { id: 'c', name: 'C', import: lang_c },
  { id: 'coffee', name: 'CoffeeScript', import: lang_coffee, aliases: ['coffeescript'] },
  { id: 'cpp', name: 'C++', import: lang_cpp, aliases: ['c++'] },
  { id: 'css', name: 'CSS', import: lang_css },
  { id: 'csv', name: 'CSV', import: lang_csv },
  { id: 'glsl', name: 'GLSL', import: lang_glsl },
  { id: 'graphql', name: 'GraphQL', import: lang_graphql, aliases: ['gql'] },
  { id: 'haml', name: 'Ruby Haml', import: lang_haml },
  { id: 'handlebars', name: 'Handlebars', import: lang_handlebars, aliases: ['hbs'] },
  { id: 'html', name: 'HTML', import: lang_html },
  { id: 'html-derivative', name: 'HTML (Derivative)', import: lang_html_derivative },
  { id: 'http', name: 'HTTP', import: lang_http },
  { id: 'hurl', name: 'Hurl', import: lang_hurl },
  { id: 'imba', name: 'Imba', import: lang_imba },
  { id: 'java', name: 'Java', import: lang_java },
  { id: 'javascript', name: 'JavaScript', import: lang_javascript, aliases: ['js', 'cjs', 'mjs'] },
  { id: 'jinja', name: 'Jinja', import: lang_jinja },
  { id: 'jison', name: 'Jison', import: lang_jison },
  { id: 'json', name: 'JSON', import: lang_json },
  { id: 'json5', name: 'JSON5', import: lang_json5 },
  { id: 'jsonc', name: 'JSON with Comments', import: lang_jsonc },
  { id: 'jsonl', name: 'JSON Lines', import: lang_jsonl },
  { id: 'jsx', name: 'JSX', import: lang_jsx },
  { id: 'julia', name: 'Julia', import: lang_julia, aliases: ['jl'] },
  { id: 'less', name: 'Less', import: lang_less },
  { id: 'markdown', name: 'Markdown', import: lang_markdown, aliases: ['md'] },
  { id: 'marko', name: 'Marko', import: lang_marko },
  { id: 'mdc', name: 'MDC', import: lang_mdc },
  { id: 'mdx', name: 'MDX', import: lang_mdx },
  { id: 'php', name: 'PHP', import: lang_php },
  { id: 'postcss', name: 'PostCSS', import: lang_postcss },
  { id: 'pug', name: 'Pug', import: lang_pug, aliases: ['jade'] },
  { id: 'python', name: 'Python', import: lang_python, aliases: ['py'] },
  { id: 'r', name: 'R', import: lang_r },
  { id: 'regexp', name: 'RegExp', import: lang_regexp, aliases: ['regex'] },
  { id: 'sass', name: 'Sass', import: lang_sass },
  { id: 'scss', name: 'SCSS', import: lang_scss },
  { id: 'shellscript', name: 'Shell', import: lang_shellscript, aliases: ['bash', 'sh', 'shell', 'zsh'] },
  { id: 'sql', name: 'SQL', import: lang_sql },
  { id: 'stylus', name: 'Stylus', import: lang_stylus, aliases: ['styl'] },
  { id: 'svelte', name: 'Svelte', import: lang_svelte },
  { id: 'ts-tags', name: 'TypeScript with Tags', import: lang_ts_tags, aliases: ['lit'] },
  { id: 'tsx', name: 'TSX', import: lang_tsx },
  { id: 'typescript', name: 'TypeScript', import: lang_typescript, aliases: ['ts', 'cts', 'mts'] },
  { id: 'vue', name: 'Vue', import: lang_vue },
  { id: 'vue-html', name: 'Vue HTML', import: lang_vue_html },
  { id: 'vue-vine', name: 'Vue Vine', import: lang_vue_vine },
  { id: 'wasm', name: 'WebAssembly', import: lang_wasm },
  { id: 'wgsl', name: 'WGSL', import: lang_wgsl },
  { id: 'wit', name: 'WebAssembly Interface Types', import: lang_wit },
  { id: 'xml', name: 'XML', import: lang_xml },
  { id: 'yaml', name: 'YAML', import: lang_yaml, aliases: ['yml'] },
  { id: 'rust', name: 'Rust', import: lang_rust },
  { id: 'go', name: 'Go', import: lang_go }
]

const theme_one_light: DynamicImportThemeRegistration = () => import('@shikijs/themes/one-light')
const theme_one_dark_pro: DynamicImportThemeRegistration = () => import('@shikijs/themes/one-dark-pro')
const theme_material_theme_lighter: DynamicImportThemeRegistration = () =>
  import('@shikijs/themes/material-theme-lighter')
const theme_material_theme_darker: DynamicImportThemeRegistration = () =>
  import('@shikijs/themes/material-theme-darker')
const theme_github_light: DynamicImportThemeRegistration = () => import('@shikijs/themes/github-light')
const theme_github_light_default: DynamicImportThemeRegistration = () => import('@shikijs/themes/github-light-default')
const theme_github_dark: DynamicImportThemeRegistration = () => import('@shikijs/themes/github-dark')
const theme_github_dark_default: DynamicImportThemeRegistration = () => import('@shikijs/themes/github-dark-default')
const theme_nord: DynamicImportThemeRegistration = () => import('@shikijs/themes/nord')
const theme_dracula: DynamicImportThemeRegistration = () => import('@shikijs/themes/dracula')
const theme_min_light: DynamicImportThemeRegistration = () => import('@shikijs/themes/min-light')
const theme_min_dark: DynamicImportThemeRegistration = () => import('@shikijs/themes/min-dark')
const theme_catppuccin_latte: DynamicImportThemeRegistration = () => import('@shikijs/themes/catppuccin-latte')
const theme_catppuccin_mocha: DynamicImportThemeRegistration = () => import('@shikijs/themes/catppuccin-mocha')
const theme_light_plus: DynamicImportThemeRegistration = () => import('@shikijs/themes/light-plus')
const theme_dark_plus: DynamicImportThemeRegistration = () => import('@shikijs/themes/dark-plus')
const theme_solarized_light: DynamicImportThemeRegistration = () => import('@shikijs/themes/solarized-light')
const theme_solarized_dark: DynamicImportThemeRegistration = () => import('@shikijs/themes/solarized-dark')
const theme_tokyo_night: DynamicImportThemeRegistration = () => import('@shikijs/themes/tokyo-night')
const theme_vesper: DynamicImportThemeRegistration = () => import('@shikijs/themes/vesper')
const theme_rose_pine_dawn: DynamicImportThemeRegistration = () => import('@shikijs/themes/rose-pine-dawn')

export const bundledThemes: Record<string, DynamicImportThemeRegistration> = {
  'one-light': theme_one_light,
  'one-dark-pro': theme_one_dark_pro,
  'material-theme-lighter': theme_material_theme_lighter,
  'material-theme-darker': theme_material_theme_darker,
  'github-light': theme_github_light,
  'github-light-default': theme_github_light_default,
  'github-dark': theme_github_dark,
  'github-dark-default': theme_github_dark_default,
  nord: theme_nord,
  dracula: theme_dracula,
  'min-light': theme_min_light,
  'min-dark': theme_min_dark,
  'catppuccin-latte': theme_catppuccin_latte,
  'catppuccin-mocha': theme_catppuccin_mocha,
  'light-plus': theme_light_plus,
  'dark-plus': theme_dark_plus,
  'solarized-light': theme_solarized_light,
  'solarized-dark': theme_solarized_dark,
  'tokyo-night': theme_tokyo_night,
  vesper: theme_vesper,
  'rose-pine-dawn': theme_rose_pine_dawn
}

export const bundledThemesInfo: BundledThemeInfo[] = [
  { id: 'one-light', displayName: 'One Light', type: 'light', import: bundledThemes['one-light'] },
  { id: 'one-dark-pro', displayName: 'One Dark Pro', type: 'dark', import: bundledThemes['one-dark-pro'] },
  {
    id: 'material-theme-lighter',
    displayName: 'Material Theme Lighter',
    type: 'light',
    import: bundledThemes['material-theme-lighter']
  },
  {
    id: 'material-theme-darker',
    displayName: 'Material Theme Darker',
    type: 'dark',
    import: bundledThemes['material-theme-darker']
  },
  { id: 'github-light', displayName: 'GitHub Light', type: 'light', import: bundledThemes['github-light'] },
  {
    id: 'github-light-default',
    displayName: 'GitHub Light Default',
    type: 'light',
    import: bundledThemes['github-light-default']
  },
  { id: 'github-dark', displayName: 'GitHub Dark', type: 'dark', import: bundledThemes['github-dark'] },
  {
    id: 'github-dark-default',
    displayName: 'GitHub Dark Default',
    type: 'dark',
    import: bundledThemes['github-dark-default']
  },
  { id: 'nord', displayName: 'Nord', type: 'dark', import: bundledThemes['nord'] },
  { id: 'dracula', displayName: 'Dracula Theme', type: 'dark', import: bundledThemes['dracula'] },
  { id: 'min-light', displayName: 'Min Light', type: 'light', import: bundledThemes['min-light'] },
  { id: 'min-dark', displayName: 'Min Dark', type: 'dark', import: bundledThemes['min-dark'] },
  { id: 'catppuccin-latte', displayName: 'Catppuccin Latte', type: 'light', import: bundledThemes['catppuccin-latte'] },
  { id: 'catppuccin-mocha', displayName: 'Catppuccin Mocha', type: 'dark', import: bundledThemes['catppuccin-mocha'] },
  { id: 'light-plus', displayName: 'Light Plus', type: 'light', import: bundledThemes['light-plus'] },
  { id: 'dark-plus', displayName: 'Dark Plus', type: 'dark', import: bundledThemes['dark-plus'] },
  { id: 'solarized-light', displayName: 'Solarized Light', type: 'light', import: bundledThemes['solarized-light'] },
  { id: 'solarized-dark', displayName: 'Solarized Dark', type: 'dark', import: bundledThemes['solarized-dark'] },
  { id: 'tokyo-night', displayName: 'Tokyo Night', type: 'dark', import: bundledThemes['tokyo-night'] },
  { id: 'vesper', displayName: 'Vesper', type: 'dark', import: bundledThemes['vesper'] },
  { id: 'rose-pine-dawn', displayName: 'Ros\u00e9 Pine Dawn', type: 'light', import: bundledThemes['rose-pine-dawn'] }
]

export const createHighlighter = /* @__PURE__ */ createBundledHighlighter({
  langs: bundledLanguages,
  themes: bundledThemes,
  engine: () => createOnigurumaEngine(import('shiki/wasm'))
})

export async function loadStudioShiki() {
  return {
    bundledLanguages,
    bundledThemes,
    bundledThemesInfo,
    createHighlighter
  }
}
