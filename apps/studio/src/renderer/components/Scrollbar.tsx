import { cn } from '@a3s-lab/ui/lib/utils'
import type { FC } from 'react'

export interface ScrollbarProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onScroll'> {
  ref?: React.Ref<HTMLDivElement | null>
  onScroll?: () => void
}

/* Platform overlay scrollbar. Do not set scrollbar-color or ::-webkit-scrollbar:
   both opt WebKit into the classic always-on bar. */
const Scrollbar: FC<ScrollbarProps> = ({
  ref: passedRef,
  children,
  className,
  onScroll: externalOnScroll,
  style,
  ...htmlProps
}) => {
  return (
    <div
      {...htmlProps}
      className={cn('overflow-y-auto', className)}
      onScroll={externalOnScroll}
      ref={passedRef}
      style={style}>
      {children}
    </div>
  )
}

Scrollbar.displayName = 'Scrollbar'

export default Scrollbar
