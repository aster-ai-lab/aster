import { DocumentOpenView } from '@renderer/pages/documents/DocumentOpenView'
import { localDocumentFromFile, openWorkspaceFile } from '@renderer/pages/documents/useLocalDocuments'
import { normalizeFilePreviewPath } from '@renderer/utils/filePreview'
import type { AbsoluteFilePath } from '@shared/types/file'

import { reviewFilePathsMatch, useReviewDocument } from './taskConversation/reviewFindings'
import { ReviewOfficeDocument } from './taskConversation/ReviewOfficeDocument'

/** Built-in editor for one workspace file, hosted in a task-workspace tab. */
export function WorkbenchFileEditor({
  root,
  path,
  readOnly = false,
  onDirtyChange,
  onSavedAtChange
}: {
  root: string
  path: string
  readOnly?: boolean
  onDirtyChange?: (dirty: boolean) => void
  onSavedAtChange?: (at: Date) => void
}) {
  const review = useReviewDocument()
  const name = path.split(/[/\\]/u).pop() || path
  let previewPath: AbsoluteFilePath | null = null
  try {
    previewPath = normalizeFilePreviewPath(path)
  } catch {
    previewPath = null
  }
  if (
    review &&
    previewPath &&
    previewPath.toLowerCase().endsWith('.docx') &&
    reviewFilePathsMatch(review.path, previewPath)
  ) {
    return (
      <div className="flex h-full min-h-0 flex-col overflow-hidden bg-white" data-workspace-file-editor={path}>
        <ReviewOfficeDocument
          key={`${previewPath}\0${review.revision ?? ''}`}
          fileName={name}
          filePath={previewPath}
          findings={review.findings}
          revision={review.revision}
          phase={review.phase}
          progress={review.progress}
        />
      </div>
    )
  }
  const document = localDocumentFromFile({
    name,
    path,
    sizeBytes: 0,
    modifiedMs: 0,
    folder: ''
  })
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-white" data-workspace-file-editor={path}>
      <DocumentOpenView
        document={document}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
        readOnly={readOnly}
        onDirtyChange={onDirtyChange}
        onSavedAtChange={onSavedAtChange}
        onOpenOutside={
          readOnly
            ? undefined
            : () => {
                if (!root) return
                void openWorkspaceFile(root, path)
              }
        }
      />
    </div>
  )
}
