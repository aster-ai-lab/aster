import type { FC } from 'react'

import { Skeleton } from '@a3s-lab/ui'

export interface SkeletonSpanProps {
  width?: string
}

export const SkeletonSpan: FC<SkeletonSpanProps> = ({ width = '60px' }) => {
  return (
    <Skeleton
      className="inline-block h-[1em] align-middle"
      style={{
        width,
        minWidth: width
      }}
    />
  )
}

SkeletonSpan.displayName = 'SkeletonSpan'
