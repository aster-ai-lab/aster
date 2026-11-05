import { Tooltip } from '@aster-ai-lab/ui'
import { type ReactElement, type ReactNode } from 'react'
import { officeFloatingPortalRoot } from '../overlay/portal-root'

/** DESIGN.md tooltip: black pill, white 13px medium, no arrow — shared Tooltip chrome. */
export function OfficeTooltip({
  label,
  children
}: {
  label: string
  children: ReactNode
}): ReactElement {
  const text = label.trim()
  if (!text) {
    return <>{children}</>
  }

  const portal =
    typeof document !== 'undefined' ? officeFloatingPortalRoot(document) : undefined

  return (
    <Tooltip
      content={text}
      showArrow={false}
      delay={200}
      sideOffset={8}
      placement="top"
      asChild
      portalContainer={portal}
      classNames={{ content: 'work-office-tooltip-bubble' }}>
      <span className="work-office-tooltip">{children}</span>
    </Tooltip>
  )
}
