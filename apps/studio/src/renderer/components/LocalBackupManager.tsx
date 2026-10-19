import type { ColumnDef } from '@a3s-lab/ui'
import {
  Button,
  DataTable,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Flex,
  Spinner,
  Tooltip
} from '@a3s-lab/ui'
import dayjs from 'dayjs'
import { ChevronLeft, ChevronRight, CircleAlert, RefreshCw, Trash2 } from 'lucide-react'
import type { Key } from 'react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { beginOptimistic, useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { restoreFromLocal } from '@renderer/services/BackupService'
import { popup } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import { getLocalizedBackupErrorMessage } from '@renderer/utils/backup'
import { formatFileSize } from '@renderer/utils/file'

interface BackupFile {
  fileName: string
  modifiedTime: string
  size: number
}

interface LocalBackupManagerProps {
  visible: boolean
  onClose: () => void
  localBackupDir?: string
  restoreMethod?: (fileName: string) => Promise<void>
}

const PAGE_SIZE = 5
const backupFileName = (file: BackupFile) => file.fileName

export function LocalBackupManager({ visible, onClose, localBackupDir, restoreMethod }: LocalBackupManagerProps) {
  const { t } = useTranslation()
  const [backupFiles, setBackupFiles] = useState<BackupFile[]>([])
  const [loading, setLoading] = useState(false)
  const [selectedRowKeys, setSelectedRowKeys] = useState<Key[]>([])
  const [deleting, setDeleting] = useState(false)
  const [restoring, setRestoring] = useState(false)
  const [currentPage, setCurrentPage] = useState(1)

  const collection = `backup/local/${localBackupDir ?? ''}`
  const optimistic = useOptimisticRows(collection, backupFiles, backupFileName)

  const fetchBackupFiles = useCallback(
    async ({ quiet = false }: { quiet?: boolean } = {}) => {
      if (!localBackupDir) {
        return
      }

      if (!quiet) setLoading(true)
      try {
        const files = await window.api.backup.listLocalBackupFiles(localBackupDir)
        setBackupFiles(files)
      } catch {
        toast.error(t('settings.data.local.backup.manager.fetch.error'))
      } finally {
        setLoading(false)
      }
    },
    [localBackupDir, t]
  )

  useEffect(() => {
    if (visible) {
      void fetchBackupFiles()
      setSelectedRowKeys([])
      setCurrentPage(1)
    }
  }, [visible, fetchBackupFiles])

  const totalPages = Math.max(1, Math.ceil(optimistic.rows.length / PAGE_SIZE))
  const safeCurrentPage = Math.min(currentPage, totalPages)

  useEffect(() => {
    setCurrentPage((page) => Math.min(page, totalPages))
  }, [totalPages])

  const paginatedBackupFiles = useMemo(() => {
    const start = (safeCurrentPage - 1) * PAGE_SIZE
    return optimistic.rows.slice(start, start + PAGE_SIZE)
  }, [optimistic.rows, safeCurrentPage])

  const currentPageKeys = useMemo(
    () => new Set(paginatedBackupFiles.map((file) => file.fileName)),
    [paginatedBackupFiles]
  )

  const handleSelectionChange = useCallback(
    (nextSelectedRowKeys: Key[]) => {
      setSelectedRowKeys((previousKeys) => {
        const preservedKeys = previousKeys.filter((key) => !currentPageKeys.has(key.toString()))
        return Array.from(new Set([...preservedKeys, ...nextSelectedRowKeys]))
      })
    },
    [currentPageKeys]
  )

  const handleDeleteSelected = async () => {
    if (selectedRowKeys.length === 0) {
      toast.warning(t('settings.data.local.backup.manager.select.files.delete'))
      return
    }

    if (!localBackupDir) {
      return
    }

    const confirmed = await popup.confirm({
      title: t('settings.data.local.backup.manager.delete.confirm.title'),
      icon: <CircleAlert />,
      content: t('settings.data.local.backup.manager.delete.confirm.multiple', { count: selectedRowKeys.length }),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      centered: true
    })
    if (!confirmed) return

    const keys = selectedRowKeys.map(String)
    const change = beginOptimistic<BackupFile>(collection, { remove: keys })
    setSelectedRowKeys([])
    setDeleting(true)
    try {
      for (const key of keys) {
        await window.api.backup.deleteLocalBackupFile(key.toString(), localBackupDir)
      }
      change.confirm()
      toast.success(t('settings.data.local.backup.manager.delete.success.multiple', { count: keys.length }))
    } catch {
      change.rollback()
      toast.error(t('settings.data.local.backup.manager.delete.error'))
    } finally {
      setDeleting(false)
      await fetchBackupFiles({ quiet: true })
      change.release()
    }
  }

  const handleDeleteSingle = async (fileName: string) => {
    if (!localBackupDir) {
      return
    }

    const confirmed = await popup.confirm({
      title: t('settings.data.local.backup.manager.delete.confirm.title'),
      icon: <CircleAlert />,
      content: t('settings.data.local.backup.manager.delete.confirm.single', { fileName }),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      centered: true
    })
    if (!confirmed) return

    const change = beginOptimistic<BackupFile>(collection, { remove: [fileName] })
    setSelectedRowKeys((keys) => keys.filter((key) => key !== fileName))
    setDeleting(true)
    try {
      await window.api.backup.deleteLocalBackupFile(fileName, localBackupDir)
      change.confirm()
      toast.success(t('settings.data.local.backup.manager.delete.success.single'))
    } catch {
      change.rollback()
      toast.error(t('settings.data.local.backup.manager.delete.error'))
    } finally {
      setDeleting(false)
      await fetchBackupFiles({ quiet: true })
      change.release()
    }
  }

  const handleRestore = async (fileName: string) => {
    if (!localBackupDir) {
      return
    }

    const confirmed = await popup.confirm({
      title: t('settings.data.local.restore.confirm.title'),
      icon: <CircleAlert />,
      content: t('settings.data.local.restore.confirm.content'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      centered: true
    })
    if (!confirmed) return

    setRestoring(true)
    try {
      await (restoreMethod || restoreFromLocal)(fileName)
      toast.success(t('settings.data.local.backup.manager.restore.success'))
      onClose() // Close the modal
    } catch (error) {
      toast.error(getLocalizedBackupErrorMessage(error, 'settings.data.local.backup.manager.restore.error'))
    } finally {
      setRestoring(false)
    }
  }

  const columns: ColumnDef<BackupFile>[] = [
    {
      accessorKey: 'fileName',
      header: t('settings.data.local.backup.manager.columns.fileName'),
      meta: { width: 'calc(100% - 504px)', className: 'min-w-0' },
      cell: ({ getValue }) => {
        const fileName = getValue() as string
        return (
          <Tooltip content={fileName} placement="top-start" fullWidthTrigger>
            <span className="block w-full min-w-0 truncate">{fileName}</span>
          </Tooltip>
        )
      }
    },
    {
      accessorKey: 'modifiedTime',
      header: t('settings.data.local.backup.manager.columns.modifiedTime'),
      meta: { width: 180 },
      cell: ({ getValue }) => dayjs(getValue() as string).format('YYYY-MM-DD HH:mm:ss')
    },
    {
      accessorKey: 'size',
      header: t('settings.data.local.backup.manager.columns.size'),
      meta: { width: 120 },
      cell: ({ getValue }) => formatFileSize(getValue() as number)
    },
    {
      id: 'action',
      header: t('settings.data.local.backup.manager.columns.actions'),
      meta: { width: 160 },
      cell: ({ row }) => {
        const record = row.original
        return (
          <Flex className="items-center gap-1">
            <Button
              className="inline-flex"
              size="sm"
              variant="ghost"
              onClick={() => handleRestore(record.fileName)}
              disabled={restoring || deleting}>
              {t('settings.data.local.backup.manager.restore.text')}
            </Button>
            <Button
              className="inline-flex"
              size="sm"
              variant="ghost"
              onClick={() => handleDeleteSingle(record.fileName)}
              disabled={deleting || restoring}>
              {t('settings.data.local.backup.manager.delete.text')}
            </Button>
          </Flex>
        )
      }
    }
  ]

  return (
    <Dialog open={visible} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-[800px]">
        <DialogHeader>
          <DialogTitle>{t('settings.data.local.backup.manager.title')}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <div className="relative">
            <DataTable
              rowKey="fileName"
              rowMotion={(file) => optimistic.motion(file.fileName)}
              columns={columns}
              data={paginatedBackupFiles}
              selection={{
                type: 'multiple',
                selectedRowKeys,
                onChange: handleSelectionChange
              }}
              emptyText={loading ? t('common.loading') : t('common.no_results')}
              tableLayout="fixed"
            />
            {loading && backupFiles.length > 0 && (
              <div className="absolute inset-0 flex items-center justify-center rounded-lg bg-background/60">
                <Spinner text={t('common.loading')} />
              </div>
            )}
          </div>
          {optimistic.rows.length > PAGE_SIZE && (
            <div className="flex items-center justify-end gap-2 text-sm text-muted-foreground">
              <span>
                {safeCurrentPage} / {totalPages}
              </span>
              <Button
                type="button"
                size="icon-sm"
                variant="ghost"
                aria-label={t('common.previous')}
                disabled={safeCurrentPage <= 1}
                onClick={() => setCurrentPage((page) => Math.max(1, page - 1))}>
                <ChevronLeft className="size-4" />
              </Button>
              <Button
                type="button"
                size="icon-sm"
                variant="ghost"
                aria-label={t('common.next')}
                disabled={safeCurrentPage >= totalPages}
                onClick={() => setCurrentPage((page) => Math.min(totalPages, page + 1))}>
                <ChevronRight className="size-4" />
              </Button>
            </div>
          )}
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => void fetchBackupFiles()} disabled={loading}>
            <RefreshCw className="size-4" />
            {t('settings.data.local.backup.manager.refresh')}
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={handleDeleteSelected}
            disabled={selectedRowKeys.length === 0 || deleting}>
            <Trash2 className="size-4" />
            {t('settings.data.local.backup.manager.delete.selected')} ({selectedRowKeys.length})
          </Button>
          <Button type="button" onClick={onClose}>
            {t('common.close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
