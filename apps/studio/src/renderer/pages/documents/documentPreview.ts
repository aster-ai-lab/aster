export type DocumentPreviewKind =
  | 'document'
  | 'markdown'
  | 'spreadsheet'
  | 'presentation'
  | 'pdf'
  | 'image'
  | 'audio'
  | 'video'
  | 'text'
  | 'unsupported'

export type OfficePreviewKind = 'document' | 'markdown' | 'spreadsheet' | 'presentation' | 'pdf'

const KIND_BY_EXTENSION: Readonly<Record<string, DocumentPreviewKind>> = {
  doc: 'document',
  docm: 'document',
  docx: 'document',
  dot: 'document',
  dotm: 'document',
  dotx: 'document',
  odt: 'document',
  rtf: 'document',
  markdown: 'markdown',
  md: 'markdown',
  csv: 'spreadsheet',
  ods: 'spreadsheet',
  tsv: 'spreadsheet',
  xls: 'spreadsheet',
  xlsb: 'spreadsheet',
  xlsm: 'spreadsheet',
  xlsx: 'spreadsheet',
  odp: 'presentation',
  pps: 'presentation',
  ppsx: 'presentation',
  ppt: 'presentation',
  pptm: 'presentation',
  pptx: 'presentation',
  pdf: 'pdf',
  avif: 'image',
  bmp: 'image',
  gif: 'image',
  ico: 'image',
  jpeg: 'image',
  jpg: 'image',
  png: 'image',
  svg: 'image',
  heic: 'image',
  heif: 'image',
  tif: 'image',
  tiff: 'image',
  webp: 'image',
  aac: 'audio',
  flac: 'audio',
  m4a: 'audio',
  mp3: 'audio',
  ogg: 'audio',
  opus: 'audio',
  aif: 'audio',
  aiff: 'audio',
  wav: 'audio',
  weba: 'audio',
  m4v: 'video',
  mov: 'video',
  mp4: 'video',
  avi: 'video',
  mkv: 'video',
  ogv: 'video',
  webm: 'video',
  acl: 'text',
  bib: 'text',
  cmake: 'text',
  diff: 'text',
  dockerfile: 'text',
  editorconfig: 'text',
  env: 'text',
  gradle: 'text',
  hcl: 'text',
  makefile: 'text',
  nix: 'text',
  patch: 'text',
  prisma: 'text',
  proto: 'text',
  rst: 'text',
  srt: 'text',
  tex: 'text',
  tf: 'text',
  vtt: 'text',
  bash: 'text',
  c: 'text',
  cc: 'text',
  cjs: 'text',
  cpp: 'text',
  css: 'text',
  go: 'text',
  gql: 'text',
  graphql: 'text',
  h: 'text',
  hpp: 'text',
  htm: 'text',
  html: 'text',
  ini: 'text',
  java: 'text',
  js: 'text',
  json: 'text',
  jsx: 'text',
  kt: 'text',
  less: 'text',
  log: 'text',
  mjs: 'text',
  php: 'text',
  py: 'text',
  rb: 'text',
  rs: 'text',
  scss: 'text',
  sh: 'text',
  cfg: 'text',
  conf: 'text',
  lock: 'text',
  properties: 'text',
  sql: 'text',
  toml: 'text',
  svelte: 'text',
  swift: 'text',
  ts: 'text',
  tsx: 'text',
  txt: 'text',
  vue: 'text',
  xml: 'text',
  yaml: 'text',
  yml: 'text',
  zsh: 'text'
}

const MIME_BY_EXTENSION: Readonly<Record<string, string>> = {
  avif: 'image/avif',
  bmp: 'image/bmp',
  gif: 'image/gif',
  ico: 'image/x-icon',
  heic: 'image/heic',
  heif: 'image/heif',
  jpeg: 'image/jpeg',
  jpg: 'image/jpeg',
  png: 'image/png',
  tif: 'image/tiff',
  tiff: 'image/tiff',
  svg: 'image/svg+xml',
  webp: 'image/webp',
  aac: 'audio/aac',
  aif: 'audio/aiff',
  aiff: 'audio/aiff',
  flac: 'audio/flac',
  m4a: 'audio/mp4',
  mp3: 'audio/mpeg',
  ogg: 'audio/ogg',
  opus: 'audio/ogg',
  wav: 'audio/wav',
  weba: 'audio/webm',
  avi: 'video/x-msvideo',
  m4v: 'video/mp4',
  mkv: 'video/x-matroska',
  mov: 'video/quicktime',
  mp4: 'video/mp4',
  ogv: 'video/ogg',
  webm: 'video/webm'
}

/** Compressed, compiled, and packaged bytes. These stay on the system app. */
const BINARY_EXTENSIONS = new Set([
  '7z',
  'a',
  'aar',
  'ai',
  'apk',
  'app',
  'arj',
  'arw',
  'bin',
  'blend',
  'bz2',
  'cab',
  'class',
  'com',
  'cr2',
  'cr3',
  'dat',
  'db',
  'deb',
  'der',
  'dll',
  'dmg',
  'dng',
  'dwg',
  'dylib',
  'eot',
  'epub',
  'exe',
  'fbx',
  'fig',
  'flv',
  'gz',
  'h5',
  'icns',
  'img',
  'indd',
  'iso',
  'jar',
  'jks',
  'lib',
  'lnk',
  'mobi',
  'msi',
  'nef',
  'numbers',
  'o',
  'orf',
  'otf',
  'p12',
  'pages',
  'parquet',
  'pfx',
  'pkg',
  'psd',
  'pyc',
  'pyo',
  'qcow2',
  'rar',
  'rlib',
  'rpm',
  'rw2',
  'sketch',
  'so',
  'sqlite',
  'sqlite3',
  'tar',
  'tga',
  'tgz',
  'ttf',
  'vdi',
  'vmdk',
  'vsd',
  'vsdx',
  'war',
  'wasm',
  'whl',
  'wmv',
  'woff',
  'woff2',
  'xd',
  'xmind',
  'xz',
  'zip',
  'zst'
])

const BINARY_BASENAMES = new Set(['.ds_store', 'thumbs.db'])

function fileBase(fileName: string): string {
  const normalized = fileName.replaceAll('\\', '/')
  const slash = normalized.lastIndexOf('/')
  return slash >= 0 ? normalized.slice(slash + 1) : normalized
}

export function fileExtension(fileName: string): string {
  const base = fileBase(fileName)
  const separator = base.lastIndexOf('.')
  if (separator <= 0 || separator === base.length - 1) return ''
  return base.slice(separator + 1).toLowerCase()
}

/** Chooses the documents open surface from the file name alone. */
export function previewKind(fileName: string): DocumentPreviewKind {
  const base = fileBase(fileName)
  const extension = fileExtension(base)
  if (extension) {
    const known = KIND_BY_EXTENSION[extension]
    if (known) return known
    if (BINARY_EXTENSIONS.has(extension)) return 'unsupported'
    return 'text'
  }
  if (!base || base.endsWith('.')) return 'unsupported'
  if (BINARY_BASENAMES.has(base.toLowerCase())) return 'unsupported'
  return 'text'
}

export function isOfficePreviewKind(kind: DocumentPreviewKind): kind is OfficePreviewKind {
  return (
    kind === 'document' || kind === 'markdown' || kind === 'spreadsheet' || kind === 'presentation' || kind === 'pdf'
  )
}

/** Recognized text, office, and media open in the file-editor window. Known binaries use the system app. */
export function isWorkspaceFileEditorKind(kind: DocumentPreviewKind): boolean {
  return kind !== 'unsupported'
}

export function previewMime(fileName: string): string | null {
  return MIME_BY_EXTENSION[fileExtension(fileName)] ?? null
}
