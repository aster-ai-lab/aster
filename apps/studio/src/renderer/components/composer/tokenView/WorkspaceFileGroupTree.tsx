import { cn } from '@a3s-lab/ui/lib/utils'

import {
  DocumentFileIcon,
  DocumentFolderIcon,
  workspaceFileIconType
} from '@renderer/components/icons/DocumentFileIcon'

import {
  buildWorkspaceFileTree,
  workspaceGroupIcon,
  type WorkspaceFileGroupEntry,
  type WorkspaceFileTreeNode
} from '../workspaceFileGroup'

const WORKSPACE_ICON_CLASS = 'size-4 shrink-0'

export function WorkspaceSelectionIcon({ entries }: { entries: readonly WorkspaceFileGroupEntry[] }) {
  const icon = workspaceGroupIcon(entries)
  return (
    <span data-workspace-file-icon={icon} className="inline-flex">
      {icon === 'directory' ? (
        <DocumentFolderIcon className={WORKSPACE_ICON_CLASS} />
      ) : (
        <DocumentFileIcon type={icon} className={WORKSPACE_ICON_CLASS} />
      )}
    </span>
  )
}

function TreeRows({ nodes, depth }: { nodes: readonly WorkspaceFileTreeNode[]; depth: number }) {
  return nodes.map((node) => {
    const icon = node.isDirectory ? 'directory' : workspaceFileIconType(node.name)
    return (
      <div key={node.path}>
        <div
          data-workspace-file-icon={icon}
          className="flex h-8 items-center gap-2 rounded-lg pr-2.5 text-[13px] text-[#3f3f3f]"
          style={{ paddingLeft: 10 + depth * 16 }}>
          {icon === 'directory' ? (
            <DocumentFolderIcon className={WORKSPACE_ICON_CLASS} />
          ) : (
            <DocumentFileIcon type={icon} className={WORKSPACE_ICON_CLASS} />
          )}
          <span className="truncate">{node.name}</span>
        </div>
        {node.children.length > 0 ? <TreeRows nodes={node.children} depth={depth + 1} /> : null}
      </div>
    )
  })
}

export function WorkspaceFileGroupTree({ entries }: { entries: readonly WorkspaceFileGroupEntry[] }) {
  const nodes = buildWorkspaceFileTree(entries)
  return (
    <div data-workspace-file-group="true" className={cn('max-h-60 w-64 overflow-auto px-1 py-1')}>
      <TreeRows nodes={nodes} depth={0} />
    </div>
  )
}
