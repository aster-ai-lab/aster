import { BookOpen, CloudUpload, FileText, FolderCog, FolderInput, HardDrive, Import, Server } from 'lucide-react'
import type { ReactNode } from 'react'
import { createElement } from 'react'

import { NutstoreIcon } from '@renderer/components/icons/NutstoreIcons'
import { JoplinIcon, SiyuanIcon } from '@renderer/components/icons/SvgIcon'
import type { DataPanelKey } from '@renderer/pages/settings/DataSettings/dataPanels'

export interface DataNavSpec {
  id: string
  panel: DataPanelKey
  titleKey: string
  icon: ReactNode
}

function lucide(icon: typeof FolderCog): ReactNode {
  return createElement(icon, { strokeWidth: 1.7, 'aria-hidden': true })
}

function brand(node: ReactNode): ReactNode {
  return createElement(
    'span',
    {
      'aria-hidden': true,
      className: 'inline-flex size-4 items-center justify-center [&_.iconfont]:text-[16px] [&_.iconfont]:leading-4'
    },
    node
  )
}

/** Sidebar rows for the data menu. Order matches the old in-page menu. */
export const dataNavSpec: readonly DataNavSpec[] = [
  { id: 'data-directory', panel: 'data', titleKey: 'settings.data.data.title', icon: lucide(FolderCog) },
  { id: 'data-local-backup', panel: 'local_backup', titleKey: 'settings.data.local.title', icon: lucide(HardDrive) },
  { id: 'data-webdav', panel: 'webdav', titleKey: 'settings.data.webdav.title', icon: lucide(CloudUpload) },
  {
    id: 'data-nutstore',
    panel: 'nutstore',
    titleKey: 'settings.data.nutstore.title',
    icon: brand(createElement(NutstoreIcon))
  },
  { id: 'data-s3', panel: 's3', titleKey: 'settings.data.s3.title.label', icon: lucide(Server) },
  {
    id: 'data-import',
    panel: 'import_settings',
    titleKey: 'settings.data.import_settings.title',
    icon: lucide(Import)
  },
  { id: 'data-export', panel: 'export_menu', titleKey: 'settings.data.export_menu.title', icon: lucide(FolderInput) },
  {
    id: 'data-markdown',
    panel: 'markdown_export',
    titleKey: 'settings.data.markdown_export.title',
    icon: lucide(FileText)
  },
  {
    id: 'data-notion',
    panel: 'notion',
    titleKey: 'settings.data.notion.title',
    icon: brand(createElement('i', { className: 'iconfont icon-notion' }))
  },
  { id: 'data-yuque', panel: 'yuque', titleKey: 'settings.data.yuque.title', icon: lucide(BookOpen) },
  {
    id: 'data-joplin',
    panel: 'joplin',
    titleKey: 'settings.data.joplin.title',
    icon: brand(createElement(JoplinIcon))
  },
  {
    id: 'data-obsidian',
    panel: 'obsidian',
    titleKey: 'settings.data.obsidian.title',
    icon: brand(createElement('i', { className: 'iconfont icon-obsidian' }))
  },
  { id: 'data-siyuan', panel: 'siyuan', titleKey: 'settings.data.siyuan.title', icon: brand(createElement(SiyuanIcon)) }
]
