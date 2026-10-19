import './Sidebar.css'
import { MenuItem } from '@a3s-lab/ui'
import { Search } from 'lucide-react'
import React, { useCallback, useEffect, useRef, type MouseEvent } from 'react'

import { MACOS_TITLEBAR_HEIGHT } from '@renderer/components/layout/trafficLights'
import useMacTransparentWindow from '@renderer/hooks/useMacTransparentWindow'
import { isMac } from '@renderer/utils/platform'
import { cn } from '@renderer/utils/style'

import { getSidebarDisplayWidth, getSidebarLayout } from './constants'
import { DefaultLogo } from './primitives'
import { SidebarFooter, type SidebarFooterActions } from './SidebarFooter'
import { SidebarList } from './SidebarList'
import { SidebarTooltip } from './Tooltip'
import type { ResolvedSidebarEntry, SidebarUser } from './types'
import { useSidebarResize } from './useSidebarResize'

export interface SidebarProps {
  width: number
  setWidth: (width: number) => void
  entries: ResolvedSidebarEntry[]
  title?: string
  logo?: React.ReactNode
  user?: SidebarUser
  isFloating?: boolean
  isFullscreen?: boolean
  searchLabel?: string
  extensionsLabel?: string
  actions?: SidebarFooterActions
  onHoverChange?: (visible: boolean) => void
  onResizePreview?: (width: number | null) => void
  onSearchClick?: () => void
  onExtensionsClick?: () => void
  onHeaderClick?: (event: MouseEvent<HTMLButtonElement>) => void
  headerExpanded?: boolean
  onDismiss?: () => void
  onReorder?: (keys: string[]) => void
}

export function Sidebar({
  width,
  setWidth,
  entries,
  title = '',
  logo,
  user,
  isFloating = false,
  isFullscreen = false,
  searchLabel = '',
  extensionsLabel = '',
  actions,
  onHoverChange,
  onResizePreview,
  onSearchClick,
  onExtensionsClick,
  onHeaderClick,
  headerExpanded = false,
  onDismiss,
  onReorder
}: SidebarProps) {
  const isMacTransparentWindow = useMacTransparentWindow()
  const { sidebarRef, startResizing } = useSidebarResize(width, setWidth, onResizePreview)
  const hoverTimeout = useRef<ReturnType<typeof setTimeout> | null>(null)
  const footerOverlayOpenRef = useRef(false)
  const floatingPointerInsideRef = useRef(false)
  const layout = getSidebarLayout(width)
  const showFooter = Boolean(extensionsLabel || user || onExtensionsClick || actions)
  const showSearch = Boolean(onSearchClick)
  const logoNode = logo ?? <DefaultLogo title={title} />

  const renderLogo = (size: 'sm' | 'default' = 'default') => (
    <div
      className={cn(
        'flex shrink-0 items-center justify-center overflow-hidden *:h-full *:w-full',
        size === 'sm' ? 'activity-bar-avatar size-9 rounded-[4px]' : 'size-6 rounded-lg'
      )}>
      {logoNode}
    </div>
  )

  const renderHeaderIdentity = (size: 'sm' | 'default', showTitle: boolean) => {
    const content = (
      <>
        {renderLogo(size)}
        {showTitle && <span className="truncate text-sm text-sidebar-foreground">{title}</span>}
      </>
    )

    if (!onHeaderClick) return content

    const popupProps = {
      'aria-haspopup': 'dialog' as const,
      'aria-expanded': headerExpanded
    }

    if (showTitle) {
      return (
        <MenuItem
          variant="ghost"
          icon={<span className="flex size-4 items-center justify-center">{renderLogo(size)}</span>}
          label={title}
          aria-label={title || undefined}
          onClick={onHeaderClick}
          className="cursor-pointer rounded-xl text-sidebar-foreground [-webkit-app-region:no-drag]"
          {...popupProps}
        />
      )
    }

    return (
      <button
        type="button"
        aria-label={title || undefined}
        onClick={onHeaderClick}
        className={
          size === 'sm'
            ? 'grid size-9 cursor-pointer place-items-center rounded-[4px] [-webkit-app-region:no-drag]'
            : 'flex min-w-0 cursor-pointer items-center [-webkit-app-region:no-drag]'
        }
        {...popupProps}>
        {content}
      </button>
    )
  }

  const handleDismiss = useCallback(() => {
    onDismiss?.()
  }, [onDismiss])

  const clearHoverDismiss = useCallback(() => {
    if (!hoverTimeout.current) return

    clearTimeout(hoverTimeout.current)
    hoverTimeout.current = null
  }, [])

  const scheduleHoverDismiss = useCallback(() => {
    clearHoverDismiss()
    hoverTimeout.current = setTimeout(handleDismiss, 300)
  }, [clearHoverDismiss, handleDismiss])

  useEffect(() => clearHoverDismiss, [clearHoverDismiss])

  const handleFooterOverlayOpenChange = useCallback(
    (open: boolean) => {
      footerOverlayOpenRef.current = open

      if (open) {
        clearHoverDismiss()
        return
      }

      if (isFloating && !floatingPointerInsideRef.current) {
        scheduleHoverDismiss()
      }
    },
    [clearHoverDismiss, isFloating, scheduleHoverDismiss]
  )

  const listProps = { entries, onReorder }
  const footerProps = {
    user,
    actions,
    extensionsLabel,
    onExtensionsClick,
    onOverlayOpenChange: handleFooterOverlayOpenChange
  }

  // --- Floating sidebar ---
  if (isFloating) {
    return (
      <div className="fixed inset-0 z-40" onClick={handleDismiss}>
        <div
          style={isMac ? { paddingTop: MACOS_TITLEBAR_HEIGHT } : undefined}
          className={cn(
            'sidebar-theme fixed top-0 bottom-0 left-0 flex w-43.5 animate-in flex-col rounded-r-sm rounded-br-2xl bg-sidebar shadow-2xl backdrop-blur-2xl backdrop-saturate-150 duration-200 select-none slide-in-from-left-2',
            '[-webkit-app-region:drag]'
          )}
          onClick={(event) => event.stopPropagation()}
          onMouseLeave={() => {
            floatingPointerInsideRef.current = false
            if (!footerOverlayOpenRef.current) {
              scheduleHoverDismiss()
            }
          }}
          onMouseEnter={() => {
            floatingPointerInsideRef.current = true
            clearHoverDismiss()
          }}>
          <div
            className={cn(
              'flex shrink-0 px-2',
              isMac && !isFullscreen ? 'h-10 items-start' : 'h-12 items-center',
              '[-webkit-app-region:drag]'
            )}>
            {renderHeaderIdentity('default', true)}
          </div>

          {showSearch && (
            <div className="px-3 py-2">
              <div
                onClick={() => {
                  onSearchClick?.()
                  handleDismiss()
                }}
                className="flex cursor-pointer items-center gap-2 rounded-md bg-sidebar-accent/50 px-2.5 py-1.5 text-xs text-muted-foreground transition-colors [-webkit-app-region:no-drag] hover:bg-accent">
                <Search size={13} />
                <span>{searchLabel}</span>
              </div>
            </div>
          )}

          <div className="flex-1 overflow-y-auto py-1">
            <SidebarList layout="full" {...listProps} />
          </div>

          {showFooter && (
            <div className="shrink-0">
              <SidebarFooter layout="full" {...footerProps} />
            </div>
          )}
        </div>
      </div>
    )
  }

  // --- Hidden sidebar (hover zone + resize handle) ---
  if (layout === 'hidden') {
    return (
      <div ref={sidebarRef} className="relative h-full w-2 shrink-0">
        <div
          className="absolute inset-y-0 left-0 z-50 w-4 [-webkit-app-region:no-drag]"
          onMouseEnter={() => {
            if (hoverTimeout.current) clearTimeout(hoverTimeout.current)
            hoverTimeout.current = setTimeout(() => onHoverChange?.(true), 200)
          }}
          onMouseLeave={() => {
            if (hoverTimeout.current) clearTimeout(hoverTimeout.current)
          }}>
          <div
            onMouseDown={(event) => {
              onHoverChange?.(false)
              startResizing(event)
            }}
            className="group/handle h-full w-full cursor-col-resize">
            <div className="ml-0.5 h-full w-0.5 rounded-full bg-foreground/30 opacity-0 transition-opacity group-hover/handle:opacity-100" />
          </div>
        </div>
      </div>
    )
  }

  // --- Visible sidebar (icon / full) ---
  const actualWidth = getSidebarDisplayWidth(width)

  return (
    <div
      ref={sidebarRef}
      style={{ width: actualWidth }}
      className={cn(
        'sidebar-theme group/sidebar relative z-20 flex h-full shrink-0 flex-col select-none',
        layout === 'icon' && 'sidebar-activity',
        '[-webkit-app-region:drag]',
        layout === 'icon' ? null : isMacTransparentWindow ? 'bg-transparent' : 'bg-sidebar'
      )}>
      {/* Header */}
      <div
        className={cn(
          'flex shrink-0',
          layout === 'icon'
            ? 'justify-center pt-3 pb-4'
            : isMac && !isFullscreen
              ? 'h-10 items-start'
              : 'h-12 items-center',
          '[-webkit-app-region:drag]',
          layout === 'full' ? 'px-2' : 'justify-center'
        )}>
        {renderHeaderIdentity(layout === 'icon' ? 'sm' : 'default', layout === 'full')}
      </div>

      {/* Search */}
      {showSearch &&
        (layout === 'full' ? (
          <div className="px-3 py-2">
            <div
              onClick={onSearchClick}
              className="flex cursor-pointer items-center gap-2 rounded-md bg-sidebar-accent px-2.5 py-1.5 text-xs text-muted-foreground transition-colors [-webkit-app-region:no-drag] hover:bg-accent">
              <Search size={13} />
              <span>{searchLabel}</span>
            </div>
          </div>
        ) : (
          <div className="flex justify-center py-1.5 [-webkit-app-region:no-drag]">
            <SidebarTooltip content={searchLabel}>
              <button
                type="button"
                onClick={onSearchClick}
                className="activity-bar-item grid size-9 place-items-center bg-transparent transition-colors duration-150">
                <Search size={22} strokeWidth={1.6} />
              </button>
            </SidebarTooltip>
          </div>
        ))}

      {/* Content */}
      <div className="flex-1 overflow-y-auto py-1">
        <SidebarList layout={layout} {...listProps} />
      </div>

      {/* Footer */}
      {showFooter && (
        <div className="shrink-0">
          <SidebarFooter layout={layout} {...footerProps} />
        </div>
      )}

      {/* The icon rail is fixed. Only a labeled sidebar keeps a drag edge. */}
      {layout !== 'icon' && (
        <div
          onMouseDown={startResizing}
          className="group/handle absolute top-0 right-0 bottom-0 z-50 w-0.75 cursor-col-resize [-webkit-app-region:no-drag]">
          <div className="h-full w-full bg-foreground/20 opacity-0 transition-opacity group-hover/handle:opacity-100" />
        </div>
      )}
    </div>
  )
}
