import type { ComponentPropsWithoutRef } from 'react'

import { cn } from '../../lib/utils'

/** Keep inline content mounted so reversing a fold preserves both its position and state. */
export function InlineDisclosure({
  open,
  className,
  children,
  ...props
}: ComponentPropsWithoutRef<'div'> & { open: boolean }) {
  return (
    <div
      {...props}
      data-slot="inline-disclosure"
      data-open={open ? 'true' : undefined}
      aria-hidden={open ? undefined : true}
      inert={!open}
      className={cn(
        'grid min-h-0 grid-rows-[0fr] opacity-0 transition-[grid-template-rows,opacity] duration-[400ms] ease-[cubic-bezier(0.2,0,0,1)] data-[open=true]:grid-rows-[1fr] data-[open=true]:opacity-100 motion-reduce:transition-none',
        className
      )}>
      <div
        data-open={open ? 'true' : undefined}
        className="invisible min-h-0 overflow-hidden transition-[visibility] duration-0 delay-[400ms] data-[open=true]:visible data-[open=true]:delay-0 motion-reduce:delay-0">
        {children}
      </div>
    </div>
  )
}
