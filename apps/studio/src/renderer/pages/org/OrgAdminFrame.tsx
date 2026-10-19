import type { ReactNode } from 'react'

import { cn } from '@renderer/utils/style'

/** 团队管理页的统一外框：标题、说明、右侧操作，下面是可滚动的正文。 */
export function OrgAdminFrame({
  ui,
  title,
  description,
  actions,
  children,
  className
}: {
  ui: string
  title: string
  description?: ReactNode
  actions?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <div data-ui={ui} className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-start justify-between gap-4 px-6 pt-4 pb-4">
        <div className="min-w-0">
          <h2 className="m-0 truncate text-[24px] leading-tight font-bold tracking-[-0.01em] text-foreground">
            {title}
          </h2>
          {description ? (
            <p className="m-0 mt-1 max-w-2xl text-[15px] leading-6 text-[#8a8a8a]">{description}</p>
          ) : null}
        </div>
        {actions ? <div className="flex min-w-0 flex-wrap items-center gap-2">{actions}</div> : null}
      </header>
      <div className={cn('flex min-h-0 flex-1 flex-col overflow-auto px-6 pb-6', className)}>{children}</div>
    </div>
  )
}

/** 正文里的一张卡片。 */
export function OrgCard({ title, children, ui }: { title?: ReactNode; children: ReactNode; ui?: string }) {
  return (
    <section data-ui={ui} className="rounded-[12px] border border-[#efefef] p-5">
      {title ? <h3 className="m-0 mb-3 text-[14px] leading-5 font-medium text-[#171717]">{title}</h3> : null}
      {children}
    </section>
  )
}
