/**
 * 组织空间面板。文件页用 pane。
 * 当前目录的文件夹和文件来自 useDrive。
 */
import { Download, FileText, Folder, FolderPlus, Upload } from 'lucide-react'
import { useId, useRef, useState, type DragEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { FileTreeSearchField } from '@renderer/components/FileTreeSearchField'
import { useRestingDialog } from '@renderer/components/restingDialog'

import { driveApi, type DriveFile, type DriveNode } from './driveTransport'
import { useDrive } from './useDrive'

const TEXT_EXTENSIONS = new Set(['txt', 'md', 'markdown', 'csv', 'json'])

function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.')
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : ''
}

/** 文本文档会编成百科。其他文件只保存原文。 */
export function compilesToEncyclopedia(file: Pick<DriveFile, 'name' | 'contentType'>): boolean {
  if (TEXT_EXTENSIONS.has(extensionOf(file.name))) return true
  const type = file.contentType.toLowerCase()
  return type.startsWith('text/') || type === 'application/json'
}

function formatWhen(raw: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})/.exec(raw)
  return match ? match[1] : raw || '—'
}

function formatBytes(size: number): string {
  if (!Number.isFinite(size) || size < 0) return '—'
  if (size < 1024) return `${Math.floor(size)} B`
  if (size < 1024 * 1024) return `${Math.floor(size / 1024)} KB`
  const mb = size / (1024 * 1024)
  const whole = Math.floor(mb)
  const tenth = Math.floor((mb - whole) * 10)
  return tenth === 0 ? `${whole} MB` : `${whole}.${tenth} MB`
}

async function saveBlob(file: { id: string; name: string }) {
  const blob = await driveApi.downloadBlob(file.id)
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = file.name
  anchor.click()
  URL.revokeObjectURL(url)
}

export function DrivePanel({ driveFileId }: { driveFileId?: string } = {}) {
  const { t } = useTranslation()
  const drive = useDrive()
  const fileInput = useRef<HTMLInputElement>(null)
  const folderTitleId = useId()
  const [trail, setTrail] = useState<DriveNode[]>([])
  const [query, setQuery] = useState('')
  const [folderOpen, setFolderOpen] = useState(false)
  const folderDialog = useRestingDialog(folderOpen)
  const [folderName, setFolderName] = useState('')
  const [dragOver, setDragOver] = useState(false)

  if (!drive.ready) return null

  const needle = query.trim().toLowerCase()
  const nodes = needle ? drive.nodes.filter((node) => node.name.toLowerCase().includes(needle)) : drive.nodes
  const files = needle ? drive.files.filter((file) => file.name.toLowerCase().includes(needle)) : drive.files
  const noRows = drive.nodes.length === 0 && drive.files.length === 0
  const emptySource = !drive.loading && noRows
  const noMatch = !drive.loading && !noRows && nodes.length === 0 && files.length === 0

  const openAt = (id: string | null, nextTrail: DriveNode[]) => {
    setQuery('')
    setTrail(nextTrail)
    void drive.openFolder(id)
  }

  const createFolder = () => {
    const name = folderName.trim()
    if (!name) return
    setFolderOpen(false)
    setFolderName('')
    void drive.createFolder(name)
  }

  const onDrop = (event: DragEvent<HTMLElement>) => {
    event.preventDefault()
    setDragOver(false)
    const file = event.dataTransfer.files?.[0]
    if (file) void drive.upload(file)
  }

  return (
    <section
      data-ui="drive.panel"
      data-drive-file={driveFileId}
      className={`flex min-h-0 flex-1 flex-col ${dragOver ? 'bg-[#f7f7f7]' : ''}`}
      onDragOver={(event) => {
        event.preventDefault()
        setDragOver(true)
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={onDrop}>
      <header className="flex items-center gap-2 px-4 py-2">
        <nav
          aria-label={t('drive.org_space')}
          className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden text-[13px]">
          <button
            type="button"
            className={`shrink-0 rounded-[6px] px-1.5 py-1 ${trail.length === 0 ? 'font-medium text-[#171717]' : 'text-[#8a8a8a] hover:text-[#171717]'}`}
            onClick={() => openAt(null, [])}>
            {t('drive.org_space')}
          </button>
          {trail.map((node, index) => (
            <span key={node.id} className="flex min-w-0 items-center gap-1">
              <span className="text-[#8a8a8a]" aria-hidden>
                /
              </span>
              <button
                type="button"
                className={`truncate rounded-[6px] px-1.5 py-1 ${index === trail.length - 1 ? 'font-medium text-[#171717]' : 'text-[#8a8a8a] hover:text-[#171717]'}`}
                onClick={() => openAt(node.id, trail.slice(0, index + 1))}>
                {node.name}
              </button>
            </span>
          ))}
        </nav>
        <FileTreeSearchField
          className="w-[180px] shrink-0"
          value={query}
          onChange={setQuery}
          placeholder={t('drive.search')}
          searchLabel={t('drive.search')}
          refreshLabel={t('drive.refresh')}
          refreshing={drive.loading}
          onRefresh={() => void drive.refresh()}
        />
        <button
          type="button"
          className="h-[30px] shrink-0 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa]"
          onClick={() => {
            setFolderName('')
            setFolderOpen(true)
          }}>
          <span className="inline-flex items-center gap-1.5">
            <FolderPlus size={14} strokeWidth={1.7} aria-hidden />
            {t('drive.new_folder')}
          </span>
        </button>
        <button
          type="button"
          className="h-8 shrink-0 rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white hover:bg-[#2a2a2a]"
          onClick={() => fileInput.current?.click()}>
          <span className="inline-flex items-center gap-1.5">
            <Upload size={14} strokeWidth={1.7} aria-hidden />
            {t('drive.upload')}
          </span>
        </button>
        <input
          ref={fileInput}
          type="file"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0]
            if (file) void drive.upload(file)
            event.target.value = ''
          }}
        />
      </header>

      {drive.error ? <p className="m-0 px-4 pb-2 text-[13px] text-[#b42318]">{drive.error}</p> : null}
      {drive.loading && noRows ? (
        <p className="m-0 px-4 py-6 text-[13px] text-[#8a8a8a]">{t('drive.loading')}</p>
      ) : null}

      {emptySource || noMatch ? (
        <div className="px-6 py-16 text-center">
          <p className="m-0 text-[13px] text-[#8a8a8a]">{noMatch ? t('drive.no_match') : t('drive.empty')}</p>
          {emptySource ? <p className="m-0 mt-1 text-[13px] text-[#8a8a8a]">{t('drive.empty_hint')}</p> : null}
        </div>
      ) : null}

      {!drive.loading && !emptySource && !noMatch ? (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full table-fixed border-collapse text-left">
            <thead className="sticky top-0 bg-white">
              <tr className="h-9 border-b border-[#efefef] text-[12px] font-normal text-[#8a8a8a]">
                <th className="px-4 font-normal">{t('drive.name')}</th>
                <th className="w-[128px] font-normal">{t('drive.updated')}</th>
                <th className="w-[148px] px-4 font-normal">{t('drive.size')}</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((node) => (
                <tr key={node.id} className="h-12 border-b border-[#efefef] hover:bg-[#f7f7f7]" data-ui="drive.folder">
                  <td className="px-4">
                    <button
                      type="button"
                      className="flex max-w-full items-center gap-2.5 text-left"
                      onClick={() => openAt(node.id, [...trail, node])}>
                      <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#f2f3f5] text-[#3f3f3f]">
                        <Folder size={16} strokeWidth={1.7} aria-hidden />
                      </span>
                      <span className="truncate text-[13px] font-medium text-[#1a1a1a]">{node.name}</span>
                    </button>
                  </td>
                  <td className="text-[13px] text-[#8a8a8a]">{formatWhen(node.createdAt)}</td>
                  <td className="px-4">
                    {node.canExclude ? (
                      <button
                        type="button"
                        data-ui="drive.folder-exclude"
                        aria-pressed={node.excluded}
                        className={`text-[13px] hover:text-[#171717] ${node.excluded ? 'font-medium text-[#171717]' : 'text-[#8a8a8a]'}`}
                        onClick={() => void drive.setFolderEncyclopedia(node.id, !node.excluded)}>
                        {t('drive.exclude_encyclopedia')}
                      </button>
                    ) : (
                      <span className="text-[13px] text-[#8a8a8a]">—</span>
                    )}
                  </td>
                </tr>
              ))}
              {files.map((file) => (
                <tr
                  key={file.id}
                  data-ui="drive.file"
                  data-drive-file={file.id}
                  aria-current={file.id === driveFileId ? 'true' : undefined}
                  className="h-12 border-b border-[#efefef] hover:bg-[#f7f7f7]">
                  <td className="px-4">
                    <div className="flex min-w-0 items-center gap-2.5">
                      <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#f2f3f5] text-[#3f3f3f]">
                        <FileText size={16} strokeWidth={1.7} aria-hidden />
                      </span>
                      <span className="min-w-0">
                        <span className="block truncate text-[13px] font-medium text-[#1a1a1a]">{file.name}</span>
                        <span className="block truncate text-[12px] text-[#8a8a8a]">
                          {compilesToEncyclopedia(file) ? t('drive.compile_hint') : t('drive.kept')}
                        </span>
                      </span>
                    </div>
                  </td>
                  <td className="text-[13px] text-[#8a8a8a]">{formatWhen(file.updatedAt || file.createdAt)}</td>
                  <td className="px-4">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[13px] text-[#3f3f3f]">{formatBytes(file.sizeBytes)}</span>
                      <button
                        type="button"
                        aria-label={`${t('drive.download')} ${file.name}`}
                        className="grid size-6 place-items-center rounded-[6px] text-[#3f3f3f] hover:bg-[#f2f3f5]"
                        onClick={() => void saveBlob(file)}>
                        <Download size={14} strokeWidth={1.7} aria-hidden />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      {folderDialog.present ? (
        <div
          data-state={folderDialog.state}
          className={`a3s-dialog-scrim fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-6 ${folderDialog.state === 'closed' ? 'pointer-events-none' : ''}`}
          onMouseDown={() => setFolderOpen(false)}>
          <form
            role="dialog"
            aria-labelledby={folderTitleId}
            data-state={folderDialog.state}
            className="a3s-dialog-surface w-[384px] rounded-[12px] bg-white p-6 shadow-[0_12px_32px_rgba(0,0,0,0.12)]"
            onMouseDown={(event) => event.stopPropagation()}
            onSubmit={(event) => {
              event.preventDefault()
              createFolder()
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') setFolderOpen(false)
            }}>
            <h2 id={folderTitleId} className="m-0 text-[15px] font-medium text-[#171717]">
              {t('drive.new_folder')}
            </h2>
            <label className="mt-4 block text-[13px] text-[#3f3f3f]">
              {t('drive.folder_name')}
              <input
                autoFocus
                value={folderName}
                onChange={(event) => setFolderName(event.target.value)}
                className="mt-1 block h-9 w-full rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10"
              />
            </label>
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                className="h-9 rounded-[8px] border border-[#e5e5e5] bg-white px-3.5 text-[13px] text-[#171717] hover:bg-[#fafafa]"
                onClick={() => setFolderOpen(false)}>
                {t('common.cancel')}
              </button>
              <button
                type="submit"
                disabled={!folderName.trim()}
                className="h-9 rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white disabled:opacity-40">
                {t('drive.new_folder')}
              </button>
            </div>
          </form>
        </div>
      ) : null}
    </section>
  )
}
