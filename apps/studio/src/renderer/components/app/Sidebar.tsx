import { CircleOff, LoaderCircle, WifiOff } from 'lucide-react'
import type { MouseEvent, Ref } from 'react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { usePersistCache } from '@data/hooks/useCache'
import { usePreference } from '@data/hooks/usePreference'
import {
  PERSONAL_CENTER,
  bindCenterMember,
  publishStoredCenter,
  readCenterId,
  readMemberAccount,
  subscribeCenter
} from '@renderer/data/memberOrgs'
import useAvatar from '@renderer/hooks/useAvatar'
import { usePlatformAdminVisible } from '@renderer/hooks/usePlatformAdminVisible'
import { useSidebarShortcuts } from '@renderer/hooks/useSidebarShortcuts'
import { useImUnread } from '@renderer/im/useIm'
import { openSettingsTab } from '@renderer/services/mainWindowNavigation'
import { toast } from '@renderer/services/toast'
import { isPlatformActivityApp, isTeamOnlyActivityApp, SIDEBAR_SHORTCUT_PROVIDER_IDS } from '@renderer/utils/sidebar'
import type { SidebarShortcutItem, SidebarShortcutTarget } from '@shared/data/preference/preferenceTypes'

import { AccountCenterMenu } from '../account/AccountCenterMenu'
import { publishMemberProfile, useMemberProfile } from '../account/memberProfile'
import { SidebarShellActions } from '../layout/ShellTabBarActions'
import { usePluginActivityShortcuts } from '../plugins/PluginSidebarRegistryProvider'
import {
  getSidebarDisplayWidth,
  getSidebarLayout,
  normalizeSidebarWidth,
  type SidebarIconPresentation,
  type SidebarUser,
  type SidebarVisibleLayout,
  Sidebar as UISidebar,
  UserAvatar
} from '../Sidebar'
import {
  useResolvedSidebarShortcuts,
  useSidebarShortcutActivation,
  useSidebarNavigationSnapshot,
  useSidebarShortcutRegistry
} from './sidebarShortcuts'

function isApp(target: SidebarShortcutTarget, id: string): boolean {
  return (
    target.kind === 'resource' &&
    target.locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
    target.locator.resourceId === id
  )
}

const isMessagesApp = (target: SidebarShortcutTarget) => isApp(target, 'messages')
/** 团队入口只在团队中心出现；日程和邮件由团队侧边栏承载。 */
export function isTeamOnlyShortcut(target: SidebarShortcutTarget): boolean {
  return (
    target.kind === 'resource' &&
    target.locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
    isTeamOnlyActivityApp(target.locator.resourceId)
  )
}

function isPlatformShortcut(target: SidebarShortcutTarget): boolean {
  return (
    target.kind === 'resource' &&
    target.locator.providerId === SIDEBAR_SHORTCUT_PROVIDER_IDS.APP &&
    isPlatformActivityApp(target.locator.resourceId)
  )
}

export default function Sidebar({
  ref,
  isFullscreen = false
}: {
  ref?: Ref<HTMLDivElement | null>
  isFullscreen?: boolean
}) {
  const { t } = useTranslation()
  const [userName] = usePreference('app.user.name')
  const { shortcuts: configuredShortcuts, reorder } = useSidebarShortcuts()
  const shortcuts = usePluginActivityShortcuts(configuredShortcuts)
  const registry = useSidebarShortcutRegistry()
  const resolutions = useResolvedSidebarShortcuts(shortcuts, registry)
  const activateShortcut = useSidebarShortcutActivation()
  const navigation = useSidebarNavigationSnapshot()
  const messagesUnread = useImUnread()
  const personal = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER) === PERSONAL_CENTER
  const showPlatform = usePlatformAdminVisible()
  const isHidden = useCallback(
    (shortcut: SidebarShortcutItem) =>
      (personal && isTeamOnlyShortcut(shortcut.target)) || (isPlatformShortcut(shortcut.target) && !showPlatform),
    [personal, showPlatform]
  )

  const [sidebarWidth, setSidebarWidth] = usePersistCache('ui.sidebar.width')
  const [previewSidebarWidth, setPreviewSidebarWidth] = useState<number | null>(null)
  const activeSidebarWidth = previewSidebarWidth ?? sidebarWidth

  useLayoutEffect(() => {
    document.documentElement.style.setProperty('--sidebar-width', `${getSidebarDisplayWidth(activeSidebarWidth)}px`)
  }, [activeSidebarWidth])

  useEffect(() => {
    if (previewSidebarWidth !== null) return
    const normalizedWidth = normalizeSidebarWidth(sidebarWidth)
    if (normalizedWidth !== sidebarWidth) setSidebarWidth(normalizedWidth)
  }, [previewSidebarWidth, setSidebarWidth, sidebarWidth])

  const localAvatar = useAvatar()
  const profile = useMemberProfile()
  const avatar = profile?.avatar || localAvatar
  useEffect(() => {
    void readMemberAccount().then(
      (account) => {
        publishMemberProfile(account)
        if (account.id) bindCenterMember(account.id)
        else publishStoredCenter()
      },
      () => undefined
    )
  }, [])
  const [centerOpen, setCenterOpen] = useState(false)
  const [centerAnchor, setCenterAnchor] = useState<HTMLElement | null>(null)
  const toggleCenter = useCallback((event: MouseEvent<HTMLButtonElement>) => {
    setCenterAnchor(event.currentTarget)
    setCenterOpen((open) => !open)
  }, [])
  const sidebarUser = useMemo<SidebarUser>(
    () => ({
      name:
        profile?.displayName ||
        userName ||
        t('chat.user', { defaultValue: t('export.user', { defaultValue: 'User' }) }),
      avatar: avatar || undefined
    }),
    [avatar, profile?.displayName, t, userName]
  )
  const sidebarLogo = useMemo(
    () => <UserAvatar user={sidebarUser} className="h-full w-full" ring={false} />,
    [sidebarUser]
  )

  const [hoverVisible, setHoverVisible] = useState(false)
  const layout = getSidebarLayout(activeSidebarWidth)
  const resolvedEntries = useMemo(
    () =>
      resolutions.flatMap((resolution) => {
        const { shortcut } = resolution
        if (isHidden(shortcut)) return []
        const provider = registry.resolve(shortcut.target)
        const isResolved = resolution.status === 'resolved'
        const label = isResolved
          ? resolution.resource.label
          : shortcut.fallbackLabel || shortcut.target.locator.resourceId
        const renderIcon = isResolved
          ? resolution.resource.renderIcon
          : ({ glyphSize }: SidebarIconPresentation) => {
              const Icon =
                resolution.status === 'loading' ? LoaderCircle : resolution.status === 'missing' ? CircleOff : WifiOff
              return (
                <Icon
                  size={glyphSize}
                  strokeWidth={1.6}
                  className={resolution.status === 'loading' ? 'animate-spin' : undefined}
                />
              )
            }
        const activate = (inNewTab = false) => {
          if (!isResolved || !provider) return
          void activateShortcut(provider, shortcut.target, resolution.resource, inNewTab).catch(() =>
            toast.error(t('common.error'))
          )
        }
        const activateInNewTab =
          isResolved && provider && resolution.resource.supportsNewTab ? () => activate(true) : undefined

        return [
          {
            key: shortcut.id,
            label,
            renderIcon,
            disabled: !isResolved || !provider,
            isActive: !!provider?.isActive?.(shortcut.target, navigation),
            badgeDot:
              !provider?.isActive?.(shortcut.target, navigation) && isMessagesApp(shortcut.target) && messagesUnread > 0
                ? true
                : undefined,
            statusLabel: isResolved
              ? undefined
              : resolution.status === 'loading'
                ? t('common.loading')
                : resolution.status === 'missing'
                  ? t('sidebar.resource_missing')
                  : t('sidebar.resource_unavailable'),
            onOpen: () => activate(),
            onOpenNewTab: activateInNewTab
          }
        ]
      }),
    [activateShortcut, isHidden, messagesUnread, navigation, registry, resolutions, t]
  )

  const handleOpenSettingsTab = useCallback(() => openSettingsTab(), [])
  const [pendingOrder, setPendingOrder] = useState<string[] | null>(null)
  const orderedEntries = useMemo(() => {
    if (!pendingOrder) return resolvedEntries
    const byKey = new Map(resolvedEntries.map((entry) => [entry.key, entry]))
    const next = pendingOrder.flatMap((key) => {
      const entry = byKey.get(key)
      return entry ? [entry] : []
    })
    for (const entry of resolvedEntries) {
      if (!pendingOrder.includes(entry.key)) next.push(entry)
    }
    return next
  }, [pendingOrder, resolvedEntries])
  useEffect(() => {
    if (!pendingOrder) return
    const current = resolvedEntries.map((entry) => entry.key)
    if (current.length === pendingOrder.length && current.every((key, index) => key === pendingOrder[index])) {
      setPendingOrder(null)
    }
  }, [pendingOrder, resolvedEntries])
  const handleReorder = useCallback(
    (keys: string[]) => {
      setPendingOrder(keys)
      const byId = new Map(shortcuts.map((item) => [item.id, item]))
      const ordered = keys.flatMap((key) => {
        const item = byId.get(key)
        return item ? [item] : []
      })
      if (ordered.length > 0) void reorder(ordered)
    },
    [reorder, shortcuts]
  )

  const sidebarProps = {
    isFullscreen,
    entries: orderedEntries,
    onReorder: handleReorder,
    title: sidebarUser.name,
    logo: sidebarLogo,
    onHeaderClick: toggleCenter,
    headerExpanded: centerOpen,
    actions: (footerLayout: SidebarVisibleLayout) => (
      <SidebarShellActions layout={footerLayout} onSettingsClick={handleOpenSettingsTab} />
    )
  }

  return (
    <div ref={ref} id="app-sidebar" data-ui="app.sidebar" className="relative h-full [-webkit-app-region:no-drag]">
      <UISidebar
        width={activeSidebarWidth}
        setWidth={setSidebarWidth}
        onHoverChange={setHoverVisible}
        onResizePreview={setPreviewSidebarWidth}
        {...sidebarProps}
      />
      {hoverVisible && layout === 'hidden' && (
        <UISidebar
          width={activeSidebarWidth}
          setWidth={setSidebarWidth}
          isFloating
          onDismiss={() => setHoverVisible(false)}
          {...sidebarProps}
        />
      )}
      <AccountCenterMenu
        open={centerOpen}
        anchor={centerAnchor}
        onOpenChange={setCenterOpen}
        avatar={avatar || undefined}
        fallbackName={sidebarUser.name}
      />
    </div>
  )
}
