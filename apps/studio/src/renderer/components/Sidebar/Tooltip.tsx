import React from 'react'

import { Tooltip } from '@a3s-lab/ui'

const sideToPlacement = {
  bottom: 'bottom',
  left: 'left',
  right: 'right',
  top: 'top'
} as const

export function SidebarTooltip({
  children,
  content,
  side = 'right'
}: {
  children: React.ReactNode
  content: string
  side?: 'right' | 'top' | 'bottom' | 'left'
}) {
  return (
    <Tooltip
      content={content}
      placement={sideToPlacement[side]}
      delay={200}
      showArrow={false}
      sideOffset={8}
      classNames={{
        content: 'rounded-full px-2.5 py-1.5 text-[13px] font-medium leading-none'
      }}>
      {children}
    </Tooltip>
  )
}
