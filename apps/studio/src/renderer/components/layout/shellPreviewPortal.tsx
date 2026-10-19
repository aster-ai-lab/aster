import { DialogPortalContainerProvider } from '@a3s-lab/ui'
import { createContext, use, type ReactNode } from 'react'

const ShellPreviewPortalContext = createContext<HTMLElement | null>(null)
const ForegroundImagePreviewPortalContext = createContext<HTMLElement | null>(null)

/** Shell node that paints above the ActivityBar. */
export function ShellPreviewPortalProvider({
  children,
  container
}: {
  children: ReactNode
  container: HTMLElement | null
}) {
  return <ShellPreviewPortalContext value={container}>{children}</ShellPreviewPortalContext>
}

/** Foreground tab only. A hidden tab keeps its preview inside that tab. */
export function ForegroundImagePreviewPortal({ active, children }: { active: boolean; children: ReactNode }) {
  const shell = use(ShellPreviewPortalContext)
  return (
    <ForegroundImagePreviewPortalContext value={active ? shell : null}>{children}</ForegroundImagePreviewPortalContext>
  )
}

/** Sends the preview to the shell layer while this tab is in front. */
export function ImagePreviewDialogFrame({ children }: { children: ReactNode }) {
  const portal = use(ForegroundImagePreviewPortalContext)
  if (!portal) return children
  return <DialogPortalContainerProvider container={portal}>{children}</DialogPortalContainerProvider>
}
