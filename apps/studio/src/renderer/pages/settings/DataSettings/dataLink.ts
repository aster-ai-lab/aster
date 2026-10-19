import { loggerService } from '@logger'
import { ipcApi } from '@renderer/ipc'

const logger = loggerService.withContext('ToBotDataLink')

/** Channels the default ToBot can read or write. Local tools stay off this list. */
export const DATA_CHANNEL_IDS = [
  'local_backup',
  'webdav',
  'nutstore',
  's3',
  'markdown_export',
  'notion',
  'yuque',
  'joplin',
  'obsidian',
  'siyuan'
] as const

export type DataChannelId = (typeof DATA_CHANNEL_IDS)[number]

const BACKUP_CHANNELS = new Set<DataChannelId>(['local_backup', 'webdav', 'nutstore', 's3'])

const ALIASES: Record<DataChannelId, readonly string[]> = {
  local_backup: ['本地备份', 'local backup'],
  webdav: ['webdav'],
  nutstore: ['坚果云', 'nutstore'],
  s3: ['s3'],
  markdown_export: ['markdown export', 'markdown导出', 'markdown 导出'],
  notion: ['notion'],
  yuque: ['语雀', 'yuque'],
  joplin: ['joplin'],
  obsidian: ['obsidian'],
  siyuan: ['思源笔记', '思源', 'siyuan']
}

const LABELS: Record<DataChannelId, string> = {
  local_backup: '本地备份',
  webdav: 'WebDAV',
  nutstore: '坚果云',
  s3: 'S3',
  markdown_export: 'Markdown',
  notion: 'Notion',
  yuque: '语雀',
  joplin: 'Joplin',
  obsidian: 'Obsidian',
  siyuan: '思源'
}

const WRITE_RE = /保存|写入|导出到|导出|同步到|备份到|(?:^|[^a-z0-9])(?:save|write|export|backup|sync)(?=$|[^a-z0-9])/i
const READ_RE = /读取|查看|搜索|列出|找一下|(?:^|[^a-z0-9])(?:read|search|list|show)(?=$|[^a-z0-9])/i

export interface DataSnapshot {
  localDir: string
  webdavHost: string
  webdavUser: string
  webdavPass: string
  webdavPath: string
  nutstoreToken: string
  nutstorePath: string
  s3Endpoint: string
  s3Region: string
  s3Bucket: string
  s3AccessKeyId: string
  s3Secret: string
  s3Root: string
  markdownPath: string
  notionApiKey: string
  notionDatabaseId: string
  yuqueToken: string
  yuqueUrl: string
  yuqueRepoId: string
  joplinToken: string
  joplinUrl: string
  obsidianVault: string
  siyuanApiUrl: string
  siyuanToken: string
}

export interface ToBotDataTurn {
  directives: string[]
  write?: DataChannelId
}

export interface DataLinkDeps {
  load: () => Promise<DataSnapshot>
  read: (channel: DataChannelId, snapshot: DataSnapshot, query: string) => Promise<string | null>
}

const DIRECTIVE_LIMIT = 500

export function emptyDataSnapshot(): DataSnapshot {
  return {
    localDir: '',
    webdavHost: '',
    webdavUser: '',
    webdavPass: '',
    webdavPath: '',
    nutstoreToken: '',
    nutstorePath: '',
    s3Endpoint: '',
    s3Region: '',
    s3Bucket: '',
    s3AccessKeyId: '',
    s3Secret: '',
    s3Root: '',
    markdownPath: '',
    notionApiKey: '',
    notionDatabaseId: '',
    yuqueToken: '',
    yuqueUrl: '',
    yuqueRepoId: '',
    joplinToken: '',
    joplinUrl: '',
    obsidianVault: '',
    siyuanApiUrl: '',
    siyuanToken: ''
  }
}

export function dataPanelUsesToBot(panel: string): boolean {
  return (DATA_CHANNEL_IDS as readonly string[]).includes(panel)
}

function filled(value: string | null | undefined): boolean {
  return Boolean(value && value.trim())
}

export function channelReady(channel: DataChannelId, snapshot: DataSnapshot): boolean {
  switch (channel) {
    case 'local_backup':
      return filled(snapshot.localDir)
    case 'webdav':
      return filled(snapshot.webdavHost)
    case 'nutstore':
      return filled(snapshot.nutstoreToken)
    case 's3':
      return [
        snapshot.s3Endpoint,
        snapshot.s3Region,
        snapshot.s3Bucket,
        snapshot.s3AccessKeyId,
        snapshot.s3Secret
      ].every(filled)
    case 'markdown_export':
      return filled(snapshot.markdownPath)
    case 'notion':
      return filled(snapshot.notionApiKey) && filled(snapshot.notionDatabaseId)
    case 'yuque':
      return filled(snapshot.yuqueToken) && (filled(snapshot.yuqueRepoId) || filled(snapshot.yuqueUrl))
    case 'joplin':
      return filled(snapshot.joplinToken) && filled(snapshot.joplinUrl)
    case 'obsidian':
      return filled(snapshot.obsidianVault)
    case 'siyuan':
      return filled(snapshot.siyuanApiUrl) && filled(snapshot.siyuanToken)
  }
}

export function configuredChannels(snapshot: DataSnapshot): DataChannelId[] {
  return DATA_CHANNEL_IDS.filter((channel) => channelReady(channel, snapshot))
}

function aliasAt(text: string, alias: string): number {
  if (/[a-z0-9]/i.test(alias)) {
    const escaped = alias.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const match = new RegExp(`(?:^|[^a-z0-9])${escaped}(?=$|[^a-z0-9])`, 'i').exec(text)
    if (!match) return -1
    return match.index + match[0].length - alias.length
  }
  const at = text.toLowerCase().indexOf(alias.toLowerCase())
  return at
}

export function mentionedChannel(text: string): DataChannelId | null {
  let best: { channel: DataChannelId; at: number; length: number } | null = null
  for (const channel of DATA_CHANNEL_IDS) {
    for (const alias of ALIASES[channel]) {
      const at = aliasAt(text, alias)
      if (at < 0) continue
      if (!best || at < best.at || (at === best.at && alias.length > best.length)) {
        best = { channel, at, length: alias.length }
      }
    }
  }
  return best?.channel ?? null
}

export function dataVerb(text: string): 'read' | 'write' | null {
  if (WRITE_RE.test(text)) return 'write'
  if (READ_RE.test(text)) return 'read'
  return null
}

export function channelQuery(text: string, channel: DataChannelId): string {
  let rest = text
  for (const alias of ALIASES[channel]) {
    rest = rest.replace(new RegExp(alias.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'ig'), ' ')
  }
  rest = rest.replace(WRITE_RE, ' ').replace(READ_RE, ' ')
  return rest.replace(/\s+/g, ' ').trim().slice(0, 80)
}

export function clipDirective(text: string): string {
  const clean = text.replace(/\0/g, '').replace(/\s+/g, ' ').trim()
  if (clean.length <= DIRECTIVE_LIMIT) return clean
  return `${clean.slice(0, DIRECTIVE_LIMIT - 1)}…`
}

export function buildDataDirectives(input: {
  text: string
  snapshot: DataSnapshot
  excerpt: string | null
  readFailed: boolean
}): ToBotDataTurn {
  const channel = mentionedChannel(input.text)
  const verb = channel ? dataVerb(input.text) : null
  const ready = configuredChannels(input.snapshot)
  const directives: string[] = []
  if (ready.length > 0) {
    directives.push(
      clipDirective(
        `ToBot data channels: ${ready.map((item) => LABELS[item]).join(', ')}. Read or write only a channel the user names. Other channels are not connected.`
      )
    )
  }
  if (!channel) return { directives }
  if (!channelReady(channel, input.snapshot)) {
    directives.push(clipDirective(`${LABELS[channel]} is not connected. Do not say that it is.`))
    return { directives }
  }
  if (verb === 'write') {
    const action = BACKUP_CHANNELS.has(channel)
      ? `back up to ${LABELS[channel]}`
      : `save this reply to ${LABELS[channel]}`
    directives.push(
      clipDirective(`The user asked to ${action}. The host does that after this turn. Do not say it already finished.`)
    )
    return { directives, write: channel }
  }
  if (input.readFailed) {
    directives.push(clipDirective(`${LABELS[channel]} read failed. Do not invent its contents.`))
  } else if (input.excerpt && input.excerpt.trim()) {
    directives.push(clipDirective(`${LABELS[channel]} read: ${input.excerpt.trim()}`))
  } else {
    directives.push(clipDirective(`${LABELS[channel]} is connected and returned nothing.`))
  }
  return { directives }
}

function withTimeout<T>(work: Promise<T>, ms: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timeout')), ms)
    work.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error: unknown) => {
        clearTimeout(timer)
        reject(error)
      }
    )
  })
}

function textValue(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

async function loadSnapshot(): Promise<DataSnapshot> {
  const { preferenceService } = await import('@data/PreferenceService')
  const values = await preferenceService.getMultiple({
    localDir: 'data.backup.local.dir',
    webdavHost: 'data.backup.webdav.host',
    webdavUser: 'data.backup.webdav.user',
    webdavPass: 'data.backup.webdav.pass',
    webdavPath: 'data.backup.webdav.path',
    nutstoreToken: 'data.backup.nutstore.token',
    nutstorePath: 'data.backup.nutstore.path',
    s3Endpoint: 'data.backup.s3.endpoint',
    s3Region: 'data.backup.s3.region',
    s3Bucket: 'data.backup.s3.bucket',
    s3AccessKeyId: 'data.backup.s3.access_key_id',
    s3Secret: 'data.backup.s3.secret_access_key',
    s3Root: 'data.backup.s3.root',
    markdownPath: 'data.export.markdown.path',
    notionApiKey: 'data.integration.notion.api_key',
    notionDatabaseId: 'data.integration.notion.database_id',
    yuqueToken: 'data.integration.yuque.token',
    yuqueUrl: 'data.integration.yuque.url',
    yuqueRepoId: 'data.integration.yuque.repo_id',
    joplinToken: 'data.integration.joplin.token',
    joplinUrl: 'data.integration.joplin.url',
    obsidianVault: 'data.integration.obsidian.default_vault',
    siyuanApiUrl: 'data.integration.siyuan.api_url',
    siyuanToken: 'data.integration.siyuan.token'
  })
  return {
    localDir: textValue(values.localDir),
    webdavHost: textValue(values.webdavHost),
    webdavUser: textValue(values.webdavUser),
    webdavPass: textValue(values.webdavPass),
    webdavPath: textValue(values.webdavPath),
    nutstoreToken: textValue(values.nutstoreToken),
    nutstorePath: textValue(values.nutstorePath),
    s3Endpoint: textValue(values.s3Endpoint),
    s3Region: textValue(values.s3Region),
    s3Bucket: textValue(values.s3Bucket),
    s3AccessKeyId: textValue(values.s3AccessKeyId),
    s3Secret: textValue(values.s3Secret),
    s3Root: textValue(values.s3Root),
    markdownPath: textValue(values.markdownPath),
    notionApiKey: textValue(values.notionApiKey),
    notionDatabaseId: textValue(values.notionDatabaseId),
    yuqueToken: textValue(values.yuqueToken),
    yuqueUrl: textValue(values.yuqueUrl),
    yuqueRepoId: textValue(values.yuqueRepoId),
    joplinToken: textValue(values.joplinToken),
    joplinUrl: textValue(values.joplinUrl),
    obsidianVault: textValue(values.obsidianVault),
    siyuanApiUrl: textValue(values.siyuanApiUrl),
    siyuanToken: textValue(values.siyuanToken)
  }
}

function namesFrom(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  const names: string[] = []
  for (const item of value) {
    if (typeof item === 'string' && item.trim()) names.push(item.trim())
    else if (item && typeof item === 'object') {
      const record = item as Record<string, unknown>
      const name = [record.fileName, record.filename, record.name, record.title, record.basename].find(
        (entry) => typeof entry === 'string' && entry.trim()
      )
      if (typeof name === 'string') names.push(name.trim())
    }
    if (names.length >= 8) break
  }
  return names
}

function studioWindow(): Window & {
  api?: {
    fs?: { readText?: (path: string) => Promise<string> }
    file?: { write?: (path: string, data: string) => Promise<void> }
    backup?: {
      listWebdavFiles?: (config: Record<string, unknown>) => Promise<unknown>
      listS3Files?: (config: Record<string, unknown>) => Promise<unknown>
      listLocalBackupFiles?: (dir: string) => Promise<unknown>
    }
    nutstore?: {
      decryptToken?: (token: string) => Promise<{ username?: string; access_token?: string } | null>
      getDirectoryContents?: (token: string, path: string) => Promise<unknown>
    }
  }
} {
  return window
}

async function fetchJson(url: string, init: RequestInit): Promise<unknown | null> {
  const signal =
    typeof AbortSignal !== 'undefined' && typeof AbortSignal.timeout === 'function'
      ? AbortSignal.timeout(3500)
      : undefined
  const response = await fetch(url, signal ? { ...init, signal } : init)
  if (!response.ok) return null
  return response.json()
}

async function readChannel(channel: DataChannelId, snapshot: DataSnapshot, query: string): Promise<string | null> {
  switch (channel) {
    case 'notion':
      return readNotion(snapshot, query)
    case 'yuque':
      return readYuque(snapshot, query)
    case 'joplin':
      return readJoplin(snapshot, query)
    case 'obsidian':
      return readObsidian(snapshot, query)
    case 'siyuan':
      return readSiyuan(snapshot, query)
    case 'webdav': {
      const listed = await studioWindow().api?.backup?.listWebdavFiles?.({
        webdavHost: snapshot.webdavHost,
        webdavUser: snapshot.webdavUser,
        webdavPass: snapshot.webdavPass,
        webdavPath: snapshot.webdavPath,
        allowSelfSignedTls: false
      })
      return listed == null ? null : namesFrom(listed).join(', ')
    }
    case 's3': {
      const listed = await studioWindow().api?.backup?.listS3Files?.({
        endpoint: snapshot.s3Endpoint,
        region: snapshot.s3Region,
        bucket: snapshot.s3Bucket,
        accessKeyId: snapshot.s3AccessKeyId,
        secretAccessKey: snapshot.s3Secret,
        root: snapshot.s3Root,
        autoSync: false,
        syncInterval: 0,
        maxBackups: 0
      })
      return listed == null ? null : namesFrom(listed).join(', ')
    }
    case 'nutstore':
      return readNutstore(snapshot)
    case 'local_backup': {
      const listed = await studioWindow().api?.backup?.listLocalBackupFiles?.(snapshot.localDir)
      return listed == null ? null : namesFrom(listed).join(', ')
    }
    case 'markdown_export':
      return 'export folder is ready'
  }
}

async function readNotion(snapshot: DataSnapshot, query: string): Promise<string | null> {
  const body = await fetchJson('https://api.notion.com/v1/search', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${snapshot.notionApiKey}`,
      'Notion-Version': '2022-06-28',
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      query,
      page_size: 5,
      filter: { property: 'object', value: 'page' }
    })
  })
  if (!body || typeof body !== 'object') return null
  const results = (body as { results?: unknown }).results
  if (!Array.isArray(results)) return null
  return results
    .map((page) => notionTitle(page))
    .filter(Boolean)
    .slice(0, 5)
    .join(', ')
}

function notionTitle(page: unknown): string {
  if (!page || typeof page !== 'object') return ''
  const properties = (page as { properties?: unknown }).properties
  if (!properties || typeof properties !== 'object') return ''
  for (const value of Object.values(properties as Record<string, unknown>)) {
    if (!value || typeof value !== 'object') continue
    const title =
      (value as { type?: unknown; title?: unknown }).type === 'title' ? (value as { title?: unknown }).title : null
    if (!Array.isArray(title)) continue
    const text = title
      .map((part) => (part && typeof part === 'object' ? (part as { plain_text?: unknown }).plain_text : ''))
      .filter((part) => typeof part === 'string')
      .join('')
    if (text.trim()) return text.trim()
  }
  return ''
}

async function readYuque(snapshot: DataSnapshot, query: string): Promise<string | null> {
  let repoId = snapshot.yuqueRepoId.trim()
  if (!repoId && snapshot.yuqueUrl.trim()) {
    const slug = snapshot.yuqueUrl
      .trim()
      .replace(/^https:\/\/www\.yuque\.com\//, '')
      .replace(/\/$/, '')
    const repo = await fetchJson(`https://www.yuque.com/api/v2/repos/${slug}`, {
      headers: { 'X-Auth-Token': snapshot.yuqueToken }
    })
    const id = repo && typeof repo === 'object' ? (repo as { data?: { id?: unknown } }).data?.id : undefined
    if (typeof id === 'string' || typeof id === 'number') repoId = String(id)
  }
  if (!repoId) return null
  const body = await fetchJson(`https://www.yuque.com/api/v2/repos/${repoId}/docs`, {
    headers: { 'X-Auth-Token': snapshot.yuqueToken }
  })
  const docs = body && typeof body === 'object' ? (body as { data?: unknown }).data : null
  const titles = namesFrom(docs)
  if (!query) return titles.join(', ')
  const matched = titles.filter((title) => title.toLowerCase().includes(query.toLowerCase()))
  return (matched.length > 0 ? matched : titles).join(', ')
}

async function readJoplin(snapshot: DataSnapshot, query: string): Promise<string | null> {
  const base = snapshot.joplinUrl.endsWith('/') ? snapshot.joplinUrl : `${snapshot.joplinUrl}/`
  const path = query
    ? `search?query=${encodeURIComponent(query)}&fields=title&limit=5`
    : 'notes?fields=title&limit=5&order_by=updated_time&order_dir=DESC'
  const body = await fetchJson(`${base}${path}&token=${encodeURIComponent(snapshot.joplinToken)}`, {})
  if (!body || typeof body !== 'object') return null
  const items = (body as { items?: unknown }).items
  return namesFrom(items).join(', ')
}

async function readSiyuan(snapshot: DataSnapshot, query: string): Promise<string | null> {
  const root = snapshot.siyuanApiUrl.replace(/\/$/, '')
  const headers = { 'Content-Type': 'application/json', Authorization: `Token ${snapshot.siyuanToken}` }
  if (!query) {
    const body = await fetchJson(`${root}/api/notebook/lsNotebooks`, { method: 'POST', headers, body: '{}' })
    const notebooks =
      body && typeof body === 'object' ? (body as { data?: { notebooks?: unknown } }).data?.notebooks : null
    return namesFrom(notebooks).join(', ')
  }
  const body = await fetchJson(`${root}/api/search/search`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ query, page: 1 })
  })
  const blocks = body && typeof body === 'object' ? (body as { data?: { blocks?: unknown } }).data?.blocks : null
  return namesFrom(blocks).join(', ')
}

function joinPath(root: string, name: string): string {
  const sep = root.includes('\\') ? '\\' : '/'
  return `${root.replace(/[\\/]+$/, '')}${sep}${name.replace(/^[/\\]+/, '')}`
}

async function readObsidian(snapshot: DataSnapshot, query: string): Promise<string | null> {
  const vaults = await ipcApi.request('export.obsidian.get_vaults')
  const vault = vaults.find((item) => item.name === snapshot.obsidianVault) ?? vaults[0]
  if (!vault) return null
  const files = await ipcApi.request('export.obsidian.get_files', { vaultName: vault.name })
  const notes = files.filter((file) => file.type === 'markdown')
  const matched = query
    ? notes.filter(
        (file) =>
          file.name.toLowerCase().includes(query.toLowerCase()) || file.path.toLowerCase().includes(query.toLowerCase())
      )
    : []
  const picked = matched[0]
  if (picked) {
    const text = await studioWindow().api?.fs?.readText?.(joinPath(vault.path, picked.path))
    if (typeof text === 'string' && text.trim()) return `${picked.name}: ${text.trim().slice(0, 320)}`
  }
  return notes
    .slice(0, 8)
    .map((file) => file.name)
    .join(', ')
}

async function readNutstore(snapshot: DataSnapshot): Promise<string | null> {
  const api = studioWindow().api?.nutstore
  const decrypted = await api?.decryptToken?.(snapshot.nutstoreToken)
  if (!decrypted?.username || !decrypted.access_token) return null
  const token = btoa(`${decrypted.username}:${decrypted.access_token}`)
  const items = await api?.getDirectoryContents?.(token, snapshot.nutstorePath || '/')
  return namesFrom(items).join(', ')
}

const defaultDeps: DataLinkDeps = { load: loadSnapshot, read: readChannel }

/** One ToBot turn. Directives stay off the stored user sentence. */
export async function prepareToBotDataTurn(text: string, deps: DataLinkDeps = defaultDeps): Promise<ToBotDataTurn> {
  let snapshot: DataSnapshot
  try {
    snapshot = await withTimeout(deps.load(), 1500)
  } catch (error) {
    logger.error('ToBot data preferences failed', error as Error)
    return { directives: [] }
  }
  const channel = mentionedChannel(text)
  const verb = channel ? dataVerb(text) : null
  const shouldRead = Boolean(channel && channelReady(channel, snapshot) && verb !== 'write')
  let excerpt: string | null = null
  let readFailed = false
  if (shouldRead && channel) {
    try {
      excerpt = await withTimeout(deps.read(channel, snapshot, channelQuery(text, channel)), 3500)
      if (excerpt == null) readFailed = true
    } catch {
      logger.error(`ToBot data read failed: ${channel}`)
      readFailed = true
    }
  }
  return buildDataDirectives({ text, snapshot, excerpt, readFailed })
}

function clipTitle(title: string): string {
  const clean = title.replace(/\s+/g, ' ').trim()
  return (clean || 'ToBot').slice(0, 80)
}

function stamp(): string {
  const date = new Date()
  const part = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}${part(date.getMonth() + 1)}${part(date.getDate())}-${part(date.getHours())}${part(date.getMinutes())}${part(date.getSeconds())}`
}

/** Write the finished ToBot reply, or run the backup the channel already knows. */
export async function deliverToBotReply(channel: DataChannelId, title: string, markdown: string): Promise<void> {
  const body = markdown.trim()
  if (!body) return
  try {
    if (BACKUP_CHANNELS.has(channel)) {
      if (channel === 'webdav') {
        const { backupToWebdav } = await import('@renderer/services/BackupService')
        await backupToWebdav({})
      } else if (channel === 's3') {
        const { backupToS3 } = await import('@renderer/services/BackupService')
        await backupToS3({})
      } else if (channel === 'local_backup') {
        const { backupToLocal } = await import('@renderer/services/BackupService')
        await backupToLocal({})
      } else {
        const { backupToNutstore } = await import('@renderer/services/NutstoreService')
        await backupToNutstore({})
      }
      return
    }
    const heading = clipTitle(title)
    if (channel === 'notion') {
      const { exportMessageToNotion } = await import('@renderer/services/ExportService')
      await exportMessageToNotion(heading.slice(0, 32), body)
    } else if (channel === 'yuque') {
      const { exportMarkdownToYuque } = await import('@renderer/services/ExportService')
      await exportMarkdownToYuque(heading, body)
    } else if (channel === 'joplin') {
      const { exportMarkdownToJoplin } = await import('@renderer/services/ExportService')
      await exportMarkdownToJoplin(heading, body)
    } else if (channel === 'siyuan') {
      const { exportMarkdownToSiyuan } = await import('@renderer/services/ExportService')
      await exportMarkdownToSiyuan(heading, body)
    } else if (channel === 'markdown_export') {
      const { exportMarkdownContentAsFile } = await import('@renderer/services/ExportService')
      await exportMarkdownContentAsFile(heading, body)
    } else if (channel === 'obsidian') {
      const snapshot = await loadSnapshot()
      const vaults = await ipcApi.request('export.obsidian.get_vaults')
      const vault = vaults.find((item) => item.name === snapshot.obsidianVault) ?? vaults[0]
      if (!vault?.path) return
      await studioWindow().api?.file?.write?.(joinPath(vault.path, `ToBot-${stamp()}.md`), `# ${heading}\n\n${body}`)
    }
  } catch {
    logger.error(`ToBot data write failed: ${channel}`)
  }
}
