import * as React from 'react'

import arcSkeleton from '../arc/skeleton/skeleton.module.css'

import { cn } from '../../lib/utils'

function Skeleton({ className, ...props }: React.ComponentProps<'div'>) {
  return <div data-slot="skeleton" className={cn('arc', arcSkeleton.block, className)} {...props} />
}

export { Skeleton }
