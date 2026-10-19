import { createFileRoute, Outlet, useRouterState } from '@tanstack/react-router'

import { appShellPaintsEntry, PageEntry } from '@renderer/components/layout/PageEntry'

function AppLayout() {
  const pathname = useRouterState({ select: (state) => state.location.pathname })
  return (
    <div className="relative flex h-full min-h-0 w-full flex-1 flex-col">
      <Outlet />
      {appShellPaintsEntry(pathname) ? <PageEntry key={pathname} id={pathname} /> : null}
    </div>
  )
}

export const Route = createFileRoute('/app')({
  component: AppLayout
})
