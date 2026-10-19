import type { ReactNode } from 'react'

const SKELETON_ROWS = [72, 56, 64, 48, 60, 52]

/** Placeholder rows while the inbox loads, shaped like real conversation rows. */
export function ConversationListSkeleton() {
  return (
    <div data-ui="messages.list-skeleton" aria-hidden className="flex flex-col">
      {SKELETON_ROWS.map((width, index) => (
        <div key={index} className="flex items-center gap-2.5 py-2 pr-3 pl-3">
          <span className="size-9 shrink-0 animate-pulse rounded-[8px] bg-[#f0f0f0]" />
          <span className="flex min-w-0 flex-1 flex-col gap-1.5">
            <span className="h-3 animate-pulse rounded-full bg-[#f0f0f0]" style={{ width: `${width}%` }} />
            <span className="h-2.5 animate-pulse rounded-full bg-[#f5f5f5]" style={{ width: `${width + 18}%` }} />
          </span>
        </div>
      ))}
    </div>
  )
}

/** Opening of a conversation that has no messages yet. */
export function ThreadStart({ avatar, title, hint }: { avatar: ReactNode; title: string; hint: string }) {
  return (
    <div data-ui="messages.thread-start" className="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-12 text-center">
      <span className="flex size-14 items-center justify-center overflow-hidden rounded-[12px] [&>img]:size-14 [&>span]:size-14">
        {avatar}
      </span>
      <div className="flex flex-col gap-1">
        <h3 className="m-0 text-[14px] font-medium text-[#171717]">{title}</h3>
        <p className="m-0 max-w-xs text-[12px] leading-relaxed text-[#8a8a8a]">{hint}</p>
      </div>
    </div>
  )
}
