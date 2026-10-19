import { cn } from '@renderer/utils/style'

export type DocumentFileIconType =
  | 'word'
  | 'pdf'
  | 'slides'
  | 'excel'
  | 'markdown'
  | 'code'
  | 'json'
  | 'sql'
  | 'config'
  | 'text'
  | 'image'
  | 'audio'
  | 'video'
  | 'archive'
  | 'file'

const ICON_TONE: Record<
  DocumentFileIconType,
  { page: string; edge: string; fold: string; mark: string; line: string }
> = {
  word: { page: '#F2F6FE', edge: '#D9E6FA', fold: '#4C84FF', mark: '#2F6BFF', line: '#D5E4FA' },
  pdf: { page: '#FFF7F7', edge: '#F6DDDD', fold: '#F05252', mark: '#E23B3B', line: '#F6C2C2' },
  slides: { page: '#FFF6EE', edge: '#F8E0CC', fold: '#F08A3C', mark: '#E06A12', line: '#F6D2B4' },
  excel: { page: '#F3FBF5', edge: '#D4EEDD', fold: '#34B56A', mark: '#1C9A4C', line: '#8ED4AE' },
  markdown: { page: '#F4F2FE', edge: '#DDD8F8', fold: '#6A5AF5', mark: '#5346E0', line: '#D9D2FB' },
  code: { page: '#F4F5F7', edge: '#E3E5EA', fold: '#C5CAD3', mark: '#7C8492', line: '#D5D8DE' },
  json: { page: '#F4F7FB', edge: '#D9E2F0', fold: '#7A93B5', mark: '#4E6584', line: '#D0DBE8' },
  sql: { page: '#F3F8FB', edge: '#D4E6F0', fold: '#3C8CB5', mark: '#1F6F96', line: '#C5DDEC' },
  config: { page: '#F8F6F2', edge: '#E8E1D6', fold: '#C4B49A', mark: '#8A7356', line: '#E4D9C8' },
  text: { page: '#F7F7F6', edge: '#E6E6E4', fold: '#C8C8C4', mark: '#6F6F6A', line: '#D9D9D6' },
  image: { page: '#F3F8FF', edge: '#D5E6FA', fold: '#5B93E6', mark: '#3B74C9', line: '#C9DDF6' },
  audio: { page: '#FBF4F8', edge: '#F3DCE8', fold: '#E07AA8', mark: '#C44E84', line: '#F0D0E0' },
  video: { page: '#F6F4FB', edge: '#E4DDF3', fold: '#8B74D6', mark: '#6A52C2', line: '#DDD4F3' },
  archive: { page: '#FBF6F1', edge: '#F0E2D2', fold: '#D29A62', mark: '#B0753A', line: '#EBD3BA' },
  file: { page: '#F4F5F7', edge: '#E3E5EA', fold: '#C5CAD3', mark: '#7C8492', line: '#D5D8DE' }
}

const WORD_EXTENSIONS = new Set(['doc', 'docx', 'docm', 'dot', 'dotm', 'dotx', 'odt', 'pages', 'rtf'])
const SLIDE_EXTENSIONS = new Set(['odp', 'pps', 'ppsx', 'ppt', 'pptm', 'pptx'])
const EXCEL_EXTENSIONS = new Set(['csv', 'numbers', 'ods', 'tsv', 'xls', 'xlsb', 'xlsm', 'xlsx'])
const IMAGE_EXTENSIONS = new Set(['avif', 'bmp', 'gif', 'ico', 'jpeg', 'jpg', 'png', 'svg', 'webp'])
const AUDIO_EXTENSIONS = new Set(['aac', 'flac', 'm4a', 'mp3', 'ogg', 'opus', 'wav', 'weba'])
const VIDEO_EXTENSIONS = new Set(['m4v', 'mov', 'mp4', 'ogv', 'webm'])
const ARCHIVE_EXTENSIONS = new Set(['7z', 'bz2', 'gz', 'rar', 'tar', 'tgz', 'zip'])
const CONFIG_EXTENSIONS = new Set([
  'cfg',
  'conf',
  'config',
  'editorconfig',
  'env',
  'ini',
  'lock',
  'properties',
  'toml',
  'xml',
  'yaml',
  'yml'
])
const TEXT_EXTENSIONS = new Set(['log', 'rst', 'txt'])
const CODE_EXTENSIONS = new Set([
  'bash',
  'c',
  'cc',
  'cjs',
  'cpp',
  'css',
  'go',
  'gql',
  'gradle',
  'graphql',
  'h',
  'hpp',
  'htm',
  'html',
  'java',
  'js',
  'jsx',
  'kt',
  'less',
  'mjs',
  'mod',
  'php',
  'py',
  'rb',
  'rs',
  'scss',
  'sh',
  'sum',
  'svelte',
  'swift',
  'ts',
  'tsx',
  'vue',
  'zsh'
])
const LABEL: Partial<Record<DocumentFileIconType, string>> = {
  archive: 'ZIP',
  config: 'CFG',
  json: '{}',
  markdown: 'MD',
  pdf: 'PDF',
  slides: 'PPT',
  sql: 'SQL'
}

const LABEL_FONT = 'Inter, -apple-system, BlinkMacSystemFont, sans-serif'

function fileExtension(name: string): string {
  const separator = name.lastIndexOf('.')
  if (separator <= 0 || separator === name.length - 1) return ''
  return name.slice(separator + 1).toLocaleLowerCase()
}

/** One icon family for the documents page and the workspace file manager. */
export function documentFileIconType(name: string): DocumentFileIconType {
  const extension = fileExtension(name)
  if (!extension) return 'code'
  if (extension === 'pdf') return 'pdf'
  if (extension === 'md' || extension === 'markdown' || extension === 'mdx') return 'markdown'
  if (extension === 'sql') return 'sql'
  if (extension === 'json' || extension === 'jsonc') return 'json'
  if (SLIDE_EXTENSIONS.has(extension)) return 'slides'
  if (EXCEL_EXTENSIONS.has(extension)) return 'excel'
  if (WORD_EXTENSIONS.has(extension)) return 'word'
  if (IMAGE_EXTENSIONS.has(extension)) return 'image'
  if (AUDIO_EXTENSIONS.has(extension)) return 'audio'
  if (VIDEO_EXTENSIONS.has(extension)) return 'video'
  if (ARCHIVE_EXTENSIONS.has(extension)) return 'archive'
  if (CONFIG_EXTENSIONS.has(extension)) return 'config'
  if (TEXT_EXTENSIONS.has(extension)) return 'text'
  if (CODE_EXTENSIONS.has(extension)) return 'code'
  return 'file'
}

export function workspaceFileIconType(name: string): DocumentFileIconType {
  return documentFileIconType(name)
}

function PageLabel({ text, fill, size, y }: { text: string; fill: string; size: number; y: number }) {
  return (
    <text x="16" y={y} textAnchor="middle" fontSize={size} fontWeight="800" fill={fill} fontFamily={LABEL_FONT}>
      {text}
    </text>
  )
}

function TextLines({ stroke }: { stroke: string }) {
  return <path d="M10.5 18h11M10.5 22h11M10.5 26h7" stroke={stroke} strokeWidth="1.4" strokeLinecap="round" />
}

export function DocumentFileIcon({ type, className }: { type: DocumentFileIconType; className?: string }) {
  const tone = ICON_TONE[type]
  const label = LABEL[type]
  return (
    <svg viewBox="0 0 32 40" className={cn('h-9 w-8 shrink-0', className)} aria-hidden="true">
      <path
        d="M8 3.2h12.2L25.2 8.6v25a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V6.2a3 3 0 0 1 3-3Z"
        fill={tone.page}
        stroke={tone.edge}
        strokeWidth="1"
        strokeLinejoin="round"
      />
      <path d="M20.1 3.5 25 8.5h-3.3a1.7 1.7 0 0 1-1.6-1.7V3.5Z" fill={tone.fold} />
      {type === 'word' ? (
        <path
          d="M9.2 21.2 12.8 32.2 16 24.6l3.2 7.6 3.6-11"
          fill="none"
          stroke={tone.mark}
          strokeWidth="2.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ) : null}
      {type === 'pdf' ? (
        <>
          <path d="M11 14.2h5.2M11 16.6h3.4" stroke={tone.line} strokeWidth="1.3" strokeLinecap="round" />
          <PageLabel text="PDF" fill={tone.mark} size={8} y={30.6} />
        </>
      ) : null}
      {type === 'code' ? (
        <text
          x="16"
          y="25.4"
          textAnchor="middle"
          fontSize="11"
          fontWeight="700"
          fill={tone.mark}
          fontFamily="ui-monospace, SFMono-Regular, Menlo, monospace">
          {'>_'}
        </text>
      ) : null}
      {type === 'excel' ? (
        <>
          <g fill="none" stroke={tone.line} strokeWidth="0.9">
            <rect x="10.2" y="12.4" width="11.2" height="7.2" rx="0.7" />
            <path d="M10.2 14.8h11.2M10.2 17.2h11.2M13.9 12.4v7.2M17.6 12.4v7.2" />
          </g>
          <PageLabel text="E" fill={tone.mark} size={13} y={31.4} />
        </>
      ) : null}
      {type === 'image' ? (
        <>
          <circle cx="13" cy="17" r="1.3" fill={tone.mark} />
          <path
            d="M10.4 26.2 14.2 21l2.4 2.6 2.2-2.2 3.2 4.8"
            fill="none"
            stroke={tone.mark}
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      ) : null}
      {type === 'audio' ? (
        <path
          d="M14 15.2v10.2a2.6 2.6 0 1 1-1.6-2.4V17.2l7-1.6v7.4a2.6 2.6 0 1 1-1.6-2.4V14.2l-3.8.9"
          fill="none"
          stroke={tone.mark}
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ) : null}
      {type === 'video' ? (
        <path
          d="M13.2 16.4h7.2a1.2 1.2 0 0 1 1.2 1.2v6.2a1.2 1.2 0 0 1-1.2 1.2h-7.2a1.2 1.2 0 0 1-1.2-1.2v-6.2a1.2 1.2 0 0 1 1.2-1.2Zm2.1 1.8 3.6 2.2-3.6 2.2v-4.4Z"
          fill={tone.mark}
        />
      ) : null}
      {type === 'text' || type === 'file' ? <TextLines stroke={tone.line} /> : null}
      {label && type !== 'pdf' ? (
        <PageLabel text={label} fill={tone.mark} size={label.length > 2 ? 7.5 : 9.5} y={31} />
      ) : null}
    </svg>
  )
}

export function DocumentFolderIcon({ open = false, className }: { open?: boolean; className?: string }) {
  return (
    <svg viewBox="0 0 32 40" className={cn('h-9 w-8 shrink-0', className)} aria-hidden="true">
      <path
        d="M3 11.2a2.6 2.6 0 0 1 2.6-2.6h7.1l2.6 2.8h11.1A2.6 2.6 0 0 1 29 14v15.4a2.6 2.6 0 0 1-2.6 2.6H5.6A2.6 2.6 0 0 1 3 29.4V11.2Z"
        fill="#9DBBFF"
      />
      {open ? (
        <path
          d="M6.4 16.6a2 2 0 0 1 1.9-1.4h20.4a1.4 1.4 0 0 1 1.3 1.9l-4 12.9a2.6 2.6 0 0 1-2.5 2H5.4a1.6 1.6 0 0 1-1.5-2.1l2.5-13.3Z"
          fill="#D6E3FF"
          stroke="#B4CBFF"
          strokeWidth="1"
          strokeLinejoin="round"
        />
      ) : (
        <path
          d="M3 15.6a2.4 2.4 0 0 1 2.4-2.4h21.2a2.4 2.4 0 0 1 2.4 2.4v13.8a2.6 2.6 0 0 1-2.6 2.6H5.6A2.6 2.6 0 0 1 3 29.4V15.6Z"
          fill="#D6E3FF"
          stroke="#B4CBFF"
          strokeWidth="1"
          strokeLinejoin="round"
        />
      )}
    </svg>
  )
}
