import type { ReactNode } from 'react'

/** Field button shared by workspace editor notices and the open-outside action. */
export const workspaceFieldButtonClass =
  'inline-flex h-9 shrink-0 items-center rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] leading-none text-[#171717] hover:bg-[#ececec]'

/** One 36px file row for every workspace editor kind. */
export function WorkspaceToolbar({ children }: { children?: ReactNode }) {
  return (
    <div
      data-workspace-toolbar
      className="flex h-9 shrink-0 items-center gap-2 border-b border-[#ececec] bg-white px-2 text-[13px]">
      {children}
    </div>
  )
}

/** Loading, too-large, unavailable, and unsupported share this pane. */
export function WorkspaceNotice({
  title,
  description,
  action,
  progress
}: {
  title: string
  description?: string
  action?: ReactNode
  progress?: number | null
}) {
  const ratio = progress == null ? null : Math.min(1, Math.max(0, progress))
  return (
    <div role="status" data-workspace-notice className="flex h-full min-h-0 flex-1 flex-col bg-white px-6 py-8">
      <h3 className="m-0 text-[14px] font-medium text-[#171717]">{title}</h3>
      {description ? <p className="m-0 mt-2 text-[13px] leading-5 text-[#8a8a8a]">{description}</p> : null}
      {ratio != null ? (
        <div
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(ratio * 100)}
          aria-label={title}
          className="mt-4 h-0.5 w-40 overflow-hidden rounded-full bg-[#f5f5f5]">
          <div className="h-full bg-[#171717]" style={{ width: `${Math.round(ratio * 100)}%` }} />
        </div>
      ) : null}
      {action}
    </div>
  )
}
