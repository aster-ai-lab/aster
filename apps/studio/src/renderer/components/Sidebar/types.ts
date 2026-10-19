import type { ReactNode } from 'react'

export interface SidebarIconPresentation {
  slotSize: number
  glyphSize: number
}

/**
 * A fully-resolved, type-agnostic sidebar row. The app shell produces these through
 * the shortcut registry; the presentation layer has no resource-domain dependencies.
 */
export interface ResolvedSidebarEntry {
  /** Stable identity used as the React key. */
  key: string
  label: string
  renderIcon: (presentation: SidebarIconPresentation) => ReactNode
  isActive: boolean
  statusLabel?: string
  /** Unread count drawn on the icon; 0 or absent hides it. */
  badge?: number
  /** Red dot on the icon, with no count. */
  badgeDot?: boolean
  onOpen: () => void
  disabled?: boolean
  onOpenNewTab?: () => void
}

export type SidebarLayout = 'hidden' | 'icon' | 'full'

export type SidebarVisibleLayout = Exclude<SidebarLayout, 'hidden'>

export interface SidebarUser {
  name: string
  avatar?: string
  onClick?: () => void
}
