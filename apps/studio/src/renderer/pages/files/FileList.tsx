import { Button, Checkbox, type CheckedState } from '@a3s-lab/ui'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ChevronDown, ChevronsUpDown, ChevronUp, FolderOpen, Pencil, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import { memo, type RefObject, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'

import ImageViewer from '@renderer/components/ImageViewer'
import {
  ItemActionBar,
  itemActionPadClass,
  type ItemActionIcon,
  type ItemActionMenuEntry
} from '@renderer/components/ItemActionBar'
import { cn } from '@renderer/utils/style'

import { FileContextMenu, type FileContextMenuActions } from './FileContextMenu'
import type { FileItem } from './fileDisplay'
import { getFormatLabel, typeIconColors, typeIcons } from './fileDisplay'
import { InlineRename } from './InlineRename'

export type SortKey = 'name' | 'size' | 'updatedAt' | 'type'
export type SortDir = 'asc' | 'desc'

const FILE_ROW_HEIGHT_PX = 44
const FILE_LIST_GRID = 'grid grid-cols-[2.5rem_minmax(0,1fr)_4.5rem_4rem_7rem] items-center gap-2'
const FILE_LIST_CHECKBOX_CLASS_NAME =
  'inline-flex items-center justify-center align-middle text-foreground hover:bg-accent data-[state=checked]:border-border-selected data-[state=checked]:bg-background-subtle data-[state=checked]:text-foreground focus-visible:border-ring'

function SortHeader({
  label,
  field,
  sortKey,
  sortDir,
  onSort
}: {
  label: string
  field: SortKey
  sortKey: SortKey
  sortDir: SortDir
  onSort: (key: SortKey) => void
}) {
  const active = sortKey === field
  const SortIcon = active ? (sortDir === 'asc' ? ChevronUp : ChevronDown) : ChevronsUpDown
  const iconClass = active ? 'shrink-0 opacity-70' : 'shrink-0 opacity-40'
  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={() => onSort(field)}
      className="h-full min-h-0 w-full justify-start gap-1 rounded-none px-0 py-0 text-xs font-medium !text-muted-foreground shadow-none hover:bg-transparent hover:!text-foreground">
      <span>{label}</span>
      <SortIcon size={9} className={iconClass} />
    </Button>
  )
}

export const FileListHeader = memo(function FileListHeader({
  visibleSelectionState,
  onSelectAll,
  sortKey,
  sortDir,
  onSort
}: {
  visibleSelectionState: CheckedState
  onSelectAll: (checked: boolean) => void
  sortKey: SortKey
  sortDir: SortDir
  onSort: (key: SortKey) => void
}) {
  const { t } = useTranslation()

  return (
    <div className={`${FILE_LIST_GRID} mx-3 mb-2 h-10 shrink-0 border-b border-border px-2.5`}>
      <div className="flex items-center self-stretch">
        <label className="flex size-full cursor-pointer items-center">
          <Checkbox
            size="sm"
            className={FILE_LIST_CHECKBOX_CLASS_NAME}
            checked={visibleSelectionState}
            onCheckedChange={(checked) => onSelectAll(Boolean(checked))}
            aria-label={t('files.select_all')}
          />
        </label>
      </div>
      <div className="min-w-0 self-stretch">
        <SortHeader label={t('files.name')} field="name" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
      </div>
      <div className="self-stretch">
        <SortHeader label={t('files.size')} field="size" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
      </div>
      <div className="self-stretch">
        <SortHeader label={t('files.type')} field="type" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
      </div>
      <div className="self-stretch">
        <SortHeader
          label={t('files.modified_at')}
          field="updatedAt"
          sortKey={sortKey}
          sortDir={sortDir}
          onSort={onSort}
        />
      </div>
    </div>
  )
})

export const FileList = memo(function FileList({
  files,
  selectedIds,
  onSelect,
  onOpen,
  onDelete,
  onRename,
  onShowInFolder,
  menuActions,
  scrollRef,
  renamingId,
  onRenameConfirm,
  onRenameCancel,
  deleteDisabled = false
}: {
  files: FileItem[]
  selectedIds: Set<string>
  onSelect: (id: string, isChecked: boolean, shouldSelectRange: boolean) => void
  onOpen: (file: FileItem) => void
  onDelete: (id: string) => void
  onRename: (id: string) => void
  onShowInFolder: (id: string) => void
  menuActions: FileContextMenuActions
  scrollRef: RefObject<HTMLDivElement | null>
  renamingId: string | null
  onRenameConfirm: (id: string, name: string) => void
  onRenameCancel: () => void
  deleteDisabled?: boolean
}) {
  const { t } = useTranslation()
  const getItemKey = useCallback((index: number) => files[index]?.id ?? index, [files])
  const rowVirtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => FILE_ROW_HEIGHT_PX,
    getItemKey,
    overscan: 8
  })

  useEffect(() => {
    if (!renamingId) return
    const index = files.findIndex((file) => file.id === renamingId)
    if (index >= 0) rowVirtualizer.scrollToIndex(index, { align: 'auto' })
  }, [files, renamingId, rowVirtualizer])

  return (
    <div className="relative flex flex-col" style={{ height: rowVirtualizer.getTotalSize() }}>
      {rowVirtualizer.getVirtualItems().map((virtualRow) => {
        const file = files[virtualRow.index]
        if (!file) return null
        const selected = selectedIds.has(file.id)
        const Icon = typeIcons[file.type]
        const previewUrl = file.type === 'image' && !file.isMissing ? file.previewUrl : undefined
        const isRenaming = renamingId === file.id
        const canUseFileActions = !file.isMissing
        const canOpen = canUseFileActions
        const canRename = canUseFileActions
        const canShowInFolder = canUseFileActions
        const deleteLabel = file.origin === 'external' ? t('files.remove_from_library') : t('files.delete.label')
        const pinned = menuActions.isSidebarPinned(file.id)
        const inline: ItemActionIcon[] = []
        if (canOpen) {
          inline.push({
            key: 'open',
            label: t('files.open'),
            icon: <SquareArrowOutUpRight className="size-4" strokeWidth={1.7} />,
            onClick: () => onOpen(file)
          })
        }
        if (canRename) {
          inline.push({
            key: 'rename',
            label: t('files.rename'),
            icon: <Pencil className="size-4" strokeWidth={1.7} />,
            onClick: () => onRename(file.id)
          })
        }
        if (canShowInFolder) {
          inline.push({
            key: 'folder',
            label: t('files.show_in_folder'),
            icon: <FolderOpen className="size-4" strokeWidth={1.7} />,
            onClick: () => onShowInFolder(file.id)
          })
        }
        inline.push({
          key: 'delete',
          label: deleteLabel,
          disabled: deleteDisabled,
          icon: <Trash2 className="size-4" strokeWidth={1.7} />,
          onClick: () => onDelete(file.id)
        })
        const menu: ItemActionMenuEntry[] =
          !file.isMissing || pinned
            ? [
                {
                  type: 'item',
                  key: 'pin',
                  label: t(pinned ? 'launchpad.unpin_from_sidebar' : 'launchpad.pin_to_sidebar'),
                  onClick: () => menuActions.onToggleSidebar(file)
                }
              ]
            : []

        return (
          <FileContextMenu key={file.id} file={file} actions={menuActions} deleteDisabled={deleteDisabled}>
            <div
              onClick={(event) => {
                if (isRenaming || file.isMissing) return
                if (event.target instanceof Element && event.target.closest('[data-file-image]')) return
                onOpen(file)
              }}
              className={`${FILE_LIST_GRID} group absolute top-0 right-3 left-3 h-10 cursor-default rounded-md px-2.5 transition-colors ${
                selected ? 'bg-accent ring-1 ring-border-subtle ring-inset' : 'hover:bg-accent'
              }`}
              style={{ transform: `translateY(${virtualRow.start}px)` }}>
              <div className="flex items-center self-stretch" onClick={(e) => e.stopPropagation()}>
                <label className="flex size-full cursor-pointer items-center">
                  <Checkbox
                    size="sm"
                    className={FILE_LIST_CHECKBOX_CLASS_NAME}
                    checked={selected}
                    onClick={(e) => {
                      e.stopPropagation()
                      onSelect(file.id, !selected, e.shiftKey)
                    }}
                    data-file-selection-checkbox
                    aria-label={t('files.select_file', { name: file.name })}
                  />
                </label>
              </div>
              <div
                className={cn(
                  'relative flex min-w-0 flex-1 items-center gap-2',
                  !isRenaming && itemActionPadClass(inline.length + (menu.length > 0 ? 1 : 0))
                )}>
                <span className="relative flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border-subtle bg-background-subtle">
                  {previewUrl ? (
                    <ImageViewer
                      alt={file.name}
                      className="absolute inset-0 size-full object-contain"
                      data-file-image={file.id}
                      decoding="async"
                      draggable={false}
                      src={previewUrl}
                    />
                  ) : (
                    <Icon size={14} strokeWidth={1.4} className={`shrink-0 ${typeIconColors[file.type]}`} />
                  )}
                </span>
                {isRenaming ? (
                  <InlineRename
                    value={file.name}
                    onConfirm={(v) => onRenameConfirm(file.id, v)}
                    onCancel={onRenameCancel}
                    className="flex-1 px-2"
                  />
                ) : (
                  <>
                    <span className="truncate text-sm text-foreground">{file.name}</span>
                    {file.isMissing && (
                      <span className="shrink-0 rounded border border-error-border bg-error-subtle px-1.5 py-0.5 text-[10px] text-error-subtle-foreground">
                        {t('files.missing')}
                      </span>
                    )}
                  </>
                )}
                {isRenaming ? null : <ItemActionBar inline={inline} menu={menu} />}
              </div>
              <span className="truncate text-xs text-muted-foreground">{file.size}</span>
              <span className="truncate text-xs text-muted-foreground">{getFormatLabel(file.format)}</span>
              <span className="truncate text-xs text-foreground-tertiary">{file.updatedAt}</span>
            </div>
          </FileContextMenu>
        )
      })}
    </div>
  )
})
