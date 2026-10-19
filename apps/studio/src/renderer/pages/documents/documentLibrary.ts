import { documentFileIconType, type DocumentFileIconType } from '@renderer/components/icons/DocumentFileIcon'

export type DocumentKind = 'doc' | 'sheet'
export type DocumentIcon = DocumentFileIconType
export type DocumentActivity = 'view' | 'upload'
export const DOCUMENT_FORMATS = ['pdf', 'txt', 'word', 'excel', 'ppt', 'ebook', 'cad', 'xmind'] as const
export type DocumentFormat = (typeof DOCUMENT_FORMATS)[number]
export const DOCUMENT_SOURCES = [
  'share',
  'cloud_save',
  'manual',
  'scan',
  'quark',
  'phone_backup',
  'computer_backup'
] as const
export type DocumentSource = (typeof DOCUMENT_SOURCES)[number]
export const DOCUMENT_DEVICES = ['computer', 'phone'] as const
export type DocumentDevice = (typeof DOCUMENT_DEVICES)[number]
export type DocumentPlace = 'shared' | 'docker' | 'cloud' | 'local'
export type DocumentFolder = 'mine' | 'shared' | 'docker' | 'downloads' | 'desktop'
export type DocumentSection = 'recent' | 'cloud' | 'local' | 'trash' | 'storage'
export type DocumentOrigin = 'local' | 'cloud'
export type DocumentListView = 'recent' | 'favorites'
export type DocumentKindFilter = 'all' | DocumentKind
export type ViewedSort = 'desc' | 'asc'

export interface LibraryDocument {
  id: string
  name: string
  owner: string
  place: DocumentPlace
  /** 演示目录 id，或已登录云盘的目录节点 id。 */
  folder: string
  kind: DocumentKind
  icon: DocumentIcon
  format: DocumentFormat
  source: DocumentSource
  device: DocumentDevice
  activity: DocumentActivity
  locationPath: string
  viewedAt: number
  /** 更新时间（云端文档为服务端 updatedAt；本地演示缺省回落 viewedAt）。 */
  updatedAt?: number
  sizeBytes: number
  favorite: boolean
  trashed: boolean
}

export interface DocumentScope {
  section: DocumentSection
  folder: string | null
  view: DocumentListView
  origin?: DocumentOrigin
}

export interface DocumentListQuery {
  query: string
  kind: DocumentKindFilter
  owner: string | null
  sort: ViewedSort
  formats?: readonly DocumentFormat[]
  sources?: readonly DocumentSource[]
  device?: DocumentDevice | null
}

const CLOUD_PLACES: readonly DocumentPlace[] = ['cloud', 'shared']

export function documentInOrigin(document: LibraryDocument, origin: DocumentOrigin): boolean {
  if (origin === 'local') return document.place === 'local'
  return CLOUD_PLACES.includes(document.place)
}

/** 云盘下固定的两个目录。本地树不使用这份预设，只列工作区里的文件夹。 */
export const DOCUMENT_TREE_FOLDERS: Record<DocumentOrigin, readonly DocumentFolder[]> = {
  cloud: ['mine', 'shared'],
  local: []
}

export function documentsInTreeFolder(
  documents: readonly LibraryDocument[],
  origin: DocumentOrigin,
  folder: DocumentFolder
): LibraryDocument[] {
  return documents
    .filter((document) => !document.trashed && document.folder === folder && documentInOrigin(document, origin))
    .slice()
    .sort((left, right) => left.name.localeCompare(right.name, 'zh-CN'))
}

const kb = (value: number) => Math.round(value * 1024)
const mb = (value: number) => Math.round(value * 1024 * 1024)

function at(month: number, day: number, hours: number, minutes: number, seconds = 0): number {
  return new Date(2026, month - 1, day, hours, minutes, seconds).getTime()
}

function todayAt(hours: number, minutes: number, seconds = 0): number {
  const now = new Date()
  return new Date(now.getFullYear(), now.getMonth(), now.getDate(), hours, minutes, seconds).getTime()
}

export const STORAGE_QUOTA_BYTES = 10 * 1024 * 1024 * 1024

interface SeedFile {
  id: string
  name: string
  locationPath: string
  viewedAt: number
  /** 更新时间（云端文档为服务端 updatedAt；本地演示缺省回落 viewedAt）。 */
  updatedAt?: number
  sizeBytes: number
  activity?: DocumentActivity
  favorite?: boolean
  owner?: string
  place?: DocumentPlace
  folder?: DocumentFolder
  source?: DocumentSource
  device?: DocumentDevice
}

function seedFile(file: SeedFile): LibraryDocument {
  return {
    id: file.id,
    name: file.name,
    owner: file.owner ?? '我',
    place: file.place ?? 'cloud',
    folder: file.folder ?? 'mine',
    kind: kindForName(file.name),
    icon: iconForName(file.name),
    format: formatForName(file.name),
    source: file.source ?? 'manual',
    device: file.device ?? 'computer',
    activity: file.activity ?? 'view',
    locationPath: file.locationPath,
    viewedAt: file.viewedAt,
    sizeBytes: file.sizeBytes,
    favorite: file.favorite ?? false,
    trashed: false
  }
}

export const SEED_DOCUMENTS: readonly LibraryDocument[] = [
  seedFile({
    id: 'privacy',
    name: '隐私政策.docx',
    locationPath: '5、用户注册协议，',
    viewedAt: todayAt(23, 4, 40),
    sizeBytes: kb(17.1),
    favorite: true
  }),
  seedFile({
    id: 'consignment',
    name: '3.寄售市场规则.docx',
    locationPath: '3、word寄售市场规则，包…',
    viewedAt: todayAt(23, 3, 40),
    sizeBytes: kb(13.8)
  }),
  seedFile({
    id: 'business-plan',
    name: '浙文交体育-商业计划书.docx',
    locationPath: '数链，',
    viewedAt: todayAt(23, 3),
    sizeBytes: kb(16.3)
  }),
  seedFile({
    id: 'resume',
    name: '简历-林志霄.pdf',
    locationPath: '夸克上传文件，',
    viewedAt: at(7, 5, 14, 58),
    sizeBytes: kb(205.2),
    owner: '林志霄',
    place: 'shared',
    folder: 'shared',
    source: 'quark'
  }),
  seedFile({
    id: 'checkup',
    name: '体检报告.pdf',
    locationPath: '材料，',
    viewedAt: at(7, 5, 14, 55),
    sizeBytes: mb(32.6),
    favorite: true,
    source: 'phone_backup',
    device: 'phone'
  }),
  seedFile({
    id: 'resume-copy',
    name: '简历-林志霄.pdf',
    locationPath: '林志霄_18770221825，',
    viewedAt: at(4, 1, 13, 15, 30),
    sizeBytes: kb(205.2),
    owner: '林志霄',
    place: 'shared',
    folder: 'shared',
    source: 'share'
  }),
  seedFile({
    id: 'resume-flip',
    name: '林志霄-简历.pdf',
    locationPath: '林志霄_18770221825，',
    viewedAt: at(4, 1, 13, 15),
    sizeBytes: kb(728.1),
    owner: '林志霄',
    place: 'shared',
    folder: 'shared',
    source: 'share'
  }),
  seedFile({
    id: 'biome',
    name: 'biome.json',
    locationPath: 'web，',
    viewedAt: at(3, 2, 21, 21, 50),
    sizeBytes: 477,
    activity: 'upload',
    place: 'local',
    folder: 'downloads',
    source: 'computer_backup'
  }),
  seedFile({
    id: 'readme',
    name: 'README.md',
    locationPath: 'web，',
    viewedAt: at(3, 2, 21, 21, 45),
    sizeBytes: 262,
    activity: 'upload',
    place: 'local',
    folder: 'downloads',
    source: 'computer_backup'
  }),
  seedFile({
    id: 'platform',
    name: '平台介绍.docx',
    locationPath: '1、word平台介绍，',
    viewedAt: at(3, 2, 21, 21, 40),
    sizeBytes: kb(2.5),
    activity: 'upload'
  }),
  seedFile({
    id: 'chain-plan',
    name: '数链项目计划书.pdf',
    locationPath: '数链，',
    viewedAt: at(3, 2, 21, 21, 35),
    sizeBytes: mb(5.4),
    activity: 'upload',
    place: 'docker',
    folder: 'docker',
    source: 'cloud_save'
  }),
  seedFile({
    id: 'complaint',
    name: '4.投诉处理规则.docx',
    locationPath: '4、用户投诉处理规则，',
    viewedAt: at(3, 2, 21, 21, 30),
    sizeBytes: kb(14.9),
    activity: 'upload'
  }),
  seedFile({
    id: 'nft-rules',
    name: '2.数字藏品发布规则.docx',
    locationPath: '2、word数字藏品发布制度…',
    viewedAt: at(3, 2, 21, 21, 25),
    sizeBytes: kb(13.2),
    activity: 'upload'
  }),
  seedFile({
    id: 'users',
    name: 't_user.csv',
    locationPath: '数链，',
    viewedAt: at(3, 2, 21, 21, 20),
    sizeBytes: mb(6.4),
    activity: 'upload'
  }),
  seedFile({
    id: 'stats-sql',
    name: '统计报表SQL.md',
    locationPath: '数链，',
    viewedAt: at(3, 2, 21, 21, 15),
    sizeBytes: kb(8.5),
    activity: 'upload'
  }),
  seedFile({
    id: 'ops',
    name: '运维统计报表.xlsx',
    locationPath: '数链，',
    viewedAt: at(3, 2, 21, 21, 10),
    sizeBytes: kb(9.3),
    activity: 'upload'
  })
]

export function iconForName(name: string): DocumentIcon {
  return documentFileIconType(name)
}

export function formatForName(name: string): DocumentFormat {
  const extension = name.split('.').pop()?.toLocaleLowerCase() ?? ''
  if (extension === 'pdf') return 'pdf'
  if (extension === 'ppt' || extension === 'pptx') return 'ppt'
  if (extension === 'csv' || extension === 'xls' || extension === 'xlsx' || extension === 'numbers') return 'excel'
  if (extension === 'doc' || extension === 'docx') return 'word'
  if (extension === 'epub' || extension === 'mobi') return 'ebook'
  if (extension === 'dwg' || extension === 'dxf') return 'cad'
  if (extension === 'xmind') return 'xmind'
  return 'txt'
}

export function kindForName(name: string): DocumentKind {
  return iconForName(name) === 'excel' ? 'sheet' : 'doc'
}

export function documentsInScope(documents: readonly LibraryDocument[], scope: DocumentScope): LibraryDocument[] {
  if (scope.section === 'storage') return []

  return documents.filter((document) => {
    if (scope.section === 'trash') {
      if (!document.trashed) return false
    } else if (document.trashed) {
      return false
    }

    if (scope.section === 'cloud' && !CLOUD_PLACES.includes(document.place)) return false
    if (scope.section === 'local' && document.place !== 'local') return false
    if (scope.origin && !documentInOrigin(document, scope.origin)) return false
    if (scope.folder && document.folder !== scope.folder) return false
    if (scope.view === 'favorites' && !document.favorite) return false
    return true
  })
}

export function listDocuments(
  documents: readonly LibraryDocument[],
  scope: DocumentScope,
  query: DocumentListQuery
): LibraryDocument[] {
  const needle = query.query.trim().toLocaleLowerCase()
  const matched = documentsInScope(documents, scope).filter((document) => {
    if (query.kind !== 'all' && document.kind !== query.kind) return false
    if (query.owner && document.owner !== query.owner) return false
    if (query.formats && query.formats.length > 0 && !query.formats.includes(document.format)) return false
    if (query.sources && query.sources.length > 0 && !query.sources.includes(document.source)) return false
    if (query.device && document.device !== query.device) return false
    if (needle && !document.name.toLocaleLowerCase().includes(needle)) return false
    return true
  })

  return matched.slice().sort((left, right) => {
    const delta = left.viewedAt - right.viewedAt
    return query.sort === 'asc' ? delta : -delta
  })
}

export function documentOwners(documents: readonly LibraryDocument[]): string[] {
  return [...new Set(documents.map((document) => document.owner))]
}

export function storageUsage(documents: readonly LibraryDocument[]): {
  usedBytes: number
  quotaBytes: number
  docBytes: number
  sheetBytes: number
} {
  let docBytes = 0
  let sheetBytes = 0
  for (const document of documents) {
    if (document.kind === 'sheet') sheetBytes += document.sizeBytes
    else docBytes += document.sizeBytes
  }
  return { usedBytes: docBytes + sheetBytes, quotaBytes: STORAGE_QUOTA_BYTES, docBytes, sheetBytes }
}

export function formatDocumentSize(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)}B`
  const gigabytes = bytes / (1024 * 1024 * 1024)
  if (gigabytes >= 1) return formatUnit(gigabytes, 'GB')
  const megabytes = bytes / (1024 * 1024)
  if (megabytes >= 1) return formatUnit(megabytes, 'MB')
  return formatUnit(bytes / 1024, 'KB')
}

function formatUnit(value: number, unit: string): string {
  const rounded = Math.round(value * 10) / 10
  const text = Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1)
  return `${text}${unit}`
}

export function formatDocumentActivity(viewedAt: number): string {
  const date = new Date(viewedAt)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  return `${month}/${day} ${hours}:${minutes}`
}

export function isSameCalendarDay(left: number, right: number): boolean {
  const a = new Date(left)
  const b = new Date(right)
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

export function blankDocument(
  kind: DocumentKind,
  owner: string,
  name: string,
  now: number,
  id: string,
  locationPath = '我的文档'
): LibraryDocument {
  return {
    id,
    name,
    owner,
    place: 'cloud',
    folder: 'mine',
    kind,
    icon: kind === 'sheet' ? 'excel' : 'word',
    format: kind === 'sheet' ? 'excel' : 'word',
    source: 'manual',
    device: 'computer',
    activity: 'view',
    locationPath,
    viewedAt: now,
    sizeBytes: kind === 'sheet' ? 4096 : 2048,
    favorite: false,
    trashed: false
  }
}

export function uploadedDocument(
  name: string,
  sizeBytes: number,
  owner: string,
  now: number,
  id: string,
  locationPath = '下载'
): LibraryDocument {
  return {
    id,
    name,
    owner,
    place: 'local',
    folder: 'downloads',
    kind: kindForName(name),
    icon: iconForName(name),
    format: formatForName(name),
    source: 'manual',
    device: 'computer',
    activity: 'upload',
    locationPath,
    viewedAt: now,
    sizeBytes: Math.max(0, sizeBytes),
    favorite: false,
    trashed: false
  }
}

export function replaceDocument(
  documents: readonly LibraryDocument[],
  id: string,
  patch: Partial<LibraryDocument>
): LibraryDocument[] {
  return documents.map((document) => (document.id === id ? { ...document, ...patch, id: document.id } : document))
}
