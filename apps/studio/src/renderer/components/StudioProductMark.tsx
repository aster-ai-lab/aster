import { studioIsAsterTenant, studioProductName } from '@renderer/tenantBrand'
import { cn } from '@renderer/utils/style'

import { ToWorkWordmark } from './ToWorkWordmark'

/** One tenant mark for the shared workbench; ToWork keeps its existing geometry. */
export function StudioProductMark({
  className,
  drawn = false,
  mark = drawn ? 'page.loader.wordmark' : 'workbench.wordmark.mark'
}: {
  className?: string
  drawn?: boolean
  mark?: string
}) {
  if (studioIsAsterTenant()) {
    return (
      <img
        data-ui={mark}
        alt=""
        src="/tenant-mark.png"
        draggable={false}
        className={cn('block h-12 w-auto', drawn && 'aster-dream-mark', className)}
      />
    )
  }
  if (studioProductName() === 'ToWork') return <ToWorkWordmark className={className} drawn={drawn} />
  return <span className={className}>{studioProductName()}</span>
}
