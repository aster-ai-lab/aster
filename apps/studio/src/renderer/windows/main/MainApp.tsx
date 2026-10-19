import { lazy, Suspense, useEffect, useMemo, type ReactElement } from 'react'

import { usePreference } from '@data/hooks/usePreference'
import { loggerService } from '@logger'
import { CORE_SIDEBAR_SHORTCUT_PROVIDERS } from '@renderer/components/app/sidebarShortcuts'
import { CodeStyleProvider } from '@renderer/components/CodeStyleProvider'
import { CommandContextKeyProvider, CommandProvider } from '@renderer/components/command'
import { ConversationNotificationRuntime } from '@renderer/components/ConversationNotificationRuntime'
import { ErrorBoundary } from '@renderer/components/ErrorBoundary'
import { AppShell } from '@renderer/components/layout/AppShell'
import { TabsProvider } from '@renderer/components/layout/TabsProvider'
import { MandatoryGateProvider } from '@renderer/components/MandatoryGateProvider'
import { AixWorkbenchRuntime } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { ExtensionPointHost } from '@renderer/components/plugins/ExtensionPointHost'
import { PluginSidebarRegistryProvider } from '@renderer/components/plugins/PluginSidebarRegistryProvider'
import { PopupHost } from '@renderer/components/PopupHost'
import { ThemeProvider } from '@renderer/components/ThemeProvider'
import ToastHost from '@renderer/components/ToastHost'
import { WindowFatalFallback } from '@renderer/components/WindowFatalFallback'
import { useMainWindowNavigation } from '@renderer/hooks/tab'
import { useStorageMonitorNotification } from '@renderer/hooks/useStorageMonitorNotification'
import { WindowCommandBindings } from '@renderer/hooks/useWindowCommands'
import { useWindowRuntime } from '@renderer/hooks/useWindowRuntime'
import { DocumentResultSaveHost } from '@renderer/pages/documents/DocumentResultSaveHost'
import { registerImageModeChooser } from '@renderer/services/imageExportModeChooser'
import { getSidebarDefaultLandingUrl } from '@renderer/utils/sidebar'
import { BUILTIN_WORKBENCH_PLUGINS } from '@renderer/windows/builtinWorkbenchPlugins'
import type { Tab } from '@shared/data/cache/cacheValueTypes'

import { useAppUpdateHandler } from './hooks/useAppUpdateHandler'
import { useAutoBackupEvents } from './hooks/useAutoBackupEvents'
import { useTopicNamingErrorNotification } from './hooks/useTopicNamingErrorNotification'

const logger = loggerService.withContext('MainApp')
const OnboardingPage = lazy(() => import('./onboarding/OnboardingPage'))

// `#spinner` is the only boot animation. Leave it in place while the first screen is still
// suspended, and remove it once that screen has mounted. A second fallback would start
// the wordmark again.
function ClearBootSpinner(): null {
  useEffect(() => {
    globalThis.document.getElementById('spinner')?.remove()
    // Paired with `console.time('init')` in index.html. DevTools timer, not a production log.

    globalThis.console.timeEnd('init')
  }, [])
  return null
}

// Behavior leaf inside the providers: the shared window runtime plus the main-only
// concerns, then the popup/toast hosts. It sits inside the providers but outside every
// TabRouter/<Activity>, so these window-scoped subscriptions and DOM sync are never
// torn down when a background tab hides.
//
// useAppUpdateHandler / useAutoBackupEvents / useStorageMonitorNotification / useTopicNamingErrorNotification are
// intentionally main-only (update events only reach the main window; the storage warning and
// topic-naming-failed toast must not duplicate across windows) and intentionally React hooks:
// they depend on React-visible
// cache/toast state and manage their own effect cleanup, and the renderer has no
// service lifecycle container, so a service would only add manual start/stop.
//
// Headless: it runs hooks and renders nothing. The popup/toast hosts are explicit
// siblings in the App JSX below, so a window's host composition is visible there.
function MainWindowRuntime(): null {
  useWindowRuntime()
  useMainWindowNavigation()

  // Register the real (component-layer) image-mode popup behind the services seam.
  // subWindow registers the same effect (SubWindowApp) — detached tabs render the
  // same route tree and can export too; other windows never reach these exports.
  useEffect(() => {
    registerImageModeChooser((imageCount) =>
      import('@renderer/components/MarkdownImageExportPopup').then((m) => m.default.show({ imageCount }))
    )
  }, [])

  useAppUpdateHandler()
  useAutoBackupEvents()
  useStorageMonitorNotification()
  useTopicNamingErrorNotification()

  return null
}

export function MainWindowContent(): ReactElement {
  const [providerSetupStatus] = usePreference('app.onboarding.provider_setup.status')
  const [sidebarShortcuts] = usePreference('ui.sidebar_shortcut')
  const showProviderOnboarding = providerSetupStatus === 'pending'

  const initialDefaultTab = useMemo<Tab>(
    () => ({
      id: 'home',
      type: 'route',
      url: getSidebarDefaultLandingUrl(sidebarShortcuts, '') || '/app/launchpad',
      title: '',
      lastAccessTime: Date.now(),
      isDormant: false
    }),
    [sidebarShortcuts]
  )

  return (
    <TabsProvider initialDefaultTab={initialDefaultTab}>
      <AixWorkbenchRuntime windowId="main" builtins={BUILTIN_WORKBENCH_PLUGINS}>
        <PluginSidebarRegistryProvider providers={CORE_SIDEBAR_SHORTCUT_PROVIDERS} builtins={BUILTIN_WORKBENCH_PLUGINS}>
          <MandatoryGateProvider open={false}>
            <WindowCommandBindings />
            {showProviderOnboarding ? (
              <Suspense fallback={null}>
                <ClearBootSpinner />
                <OnboardingPage />
              </Suspense>
            ) : (
              <>
                <ClearBootSpinner />
                <AppShell />
              </>
            )}
            <MainWindowRuntime />
            <DocumentResultSaveHost />
            <ConversationNotificationRuntime />
            <ExtensionPointHost />
            <PopupHost />
            <ToastHost />
          </MandatoryGateProvider>
        </PluginSidebarRegistryProvider>
      </AixWorkbenchRuntime>
    </TabsProvider>
  )
}

function MainApp(): ReactElement {
  logger.info('MainApp initialized')

  return (
    // The boundary must stay the ANCESTOR of every provider so a provider throwing
    // during render (e.g. reading preferences) falls back instead of white-screening.
    <ErrorBoundary fallbackComponent={WindowFatalFallback}>
      <ThemeProvider>
        <CodeStyleProvider>
          <CommandContextKeyProvider>
            <CommandProvider>
              <MainWindowContent />
            </CommandProvider>
          </CommandContextKeyProvider>
        </CodeStyleProvider>
      </ThemeProvider>
    </ErrorBoundary>
  )
}

export default MainApp
