/** Default width of every page sidebar. The work page is the reference. */
export const PAGE_SIDEBAR_DEFAULT_WIDTH = 260

/** Right hairline shared by page sidebars. */
export const pageSidebarEdgeClassName = 'border-r border-[#efefef]'

/** Fixed width matching PAGE_SIDEBAR_DEFAULT_WIDTH. Keep the pixel value in sync. */
export const pageSidebarWidthClassName = 'w-[260px] min-w-[260px] shrink-0'

const WORKBENCH_SIDEBAR_COLLAPSED_KEY = 'workbench.sidebarCollapsed'

/** Remember the work-page sidebar the way the console remembers its own. */
export function readWorkbenchSidebarCollapsed(): boolean {
  try {
    return window.localStorage.getItem(WORKBENCH_SIDEBAR_COLLAPSED_KEY) === '1'
  } catch {
    return false
  }
}

export function writeWorkbenchSidebarCollapsed(collapsed: boolean) {
  try {
    window.localStorage.setItem(WORKBENCH_SIDEBAR_COLLAPSED_KEY, collapsed ? '1' : '0')
  } catch {
    // Private mode can reject storage. The open page still toggles.
  }
}
