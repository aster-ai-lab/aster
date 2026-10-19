import { useEffect } from 'react'

import { CORE_SIDEBAR_SHORTCUT_PROVIDERS } from '@renderer/components/app/sidebarShortcuts'
import { CodeStyleProvider } from '@renderer/components/CodeStyleProvider'
import { CommandContextKeyProvider, CommandProvider } from '@renderer/components/command'
import { ConversationNotificationRuntime } from '@renderer/components/ConversationNotificationRuntime'
import { ErrorBoundary } from '@renderer/components/ErrorBoundary'
import { TabsProvider } from '@renderer/components/layout/TabsProvider'
import { PluginSidebarRegistryProvider } from '@renderer/components/plugins/PluginSidebarRegistryProvider'
import { PluginWindowRuntime } from '@renderer/components/plugins/PluginWindowRuntime'
import { PopupHost } from '@renderer/components/PopupHost'
import { ThemeProvider } from '@renderer/components/ThemeProvider'
import ToastHost from '@renderer/components/ToastHost'
import { WindowFatalFallback } from '@renderer/components/WindowFatalFallback'
import { WindowCommandBindings } from '@renderer/hooks/useWindowCommands'
import { useWindowRuntime } from '@renderer/hooks/useWindowRuntime'
import { registerImageModeChooser } from '@renderer/services/imageExportModeChooser'
import { BUILTIN_WORKBENCH_PLUGINS } from '@renderer/windows/builtinWorkbenchPlugins'
import { SubWindowAppShell } from '@renderer/windows/subWindow/SubWindowAppShell'

// Headless behavior leaf inside the providers: the shared window runtime (same route
// tree as main, so it needs the same window-level side effects). It renders nothing;
// the popup/toast hosts are explicit siblings in the App JSX below. The subWindow has
// none of the main-only concerns (boot spinner/timer, update/storage notification).
function SubWindowRuntime(): null {
  useWindowRuntime()

  // Same route tree as main, so topic/message exports run here too — register the
  // image-mode popup behind the services seam like MainApp does.
  useEffect(() => {
    registerImageModeChooser((imageCount) =>
      import('@renderer/components/MarkdownImageExportPopup').then((m) => m.default.show({ imageCount }))
    )
  }, [])

  return null
}

function SubWindowApp(): React.ReactElement {
  return (
    // The boundary must stay the ANCESTOR of every provider so a provider throwing
    // during render falls back instead of white-screening.
    <ErrorBoundary fallbackComponent={WindowFatalFallback}>
      <ThemeProvider>
        <CodeStyleProvider>
          <CommandContextKeyProvider>
            <CommandProvider>
              <TabsProvider initialDefaultTab={null} includePinnedTabs={false}>
                <PluginWindowRuntime
                  namespace="studio-builtin-ui"
                  windowId="detached"
                  plugins={BUILTIN_WORKBENCH_PLUGINS}>
                  <PluginSidebarRegistryProvider
                    providers={CORE_SIDEBAR_SHORTCUT_PROVIDERS}
                    builtins={BUILTIN_WORKBENCH_PLUGINS}>
                    <WindowCommandBindings />
                    <SubWindowAppShell />
                    <SubWindowRuntime />
                    <ConversationNotificationRuntime />
                    <PopupHost />
                    <ToastHost />
                  </PluginSidebarRegistryProvider>
                </PluginWindowRuntime>
              </TabsProvider>
            </CommandProvider>
          </CommandContextKeyProvider>
        </CodeStyleProvider>
      </ThemeProvider>
    </ErrorBoundary>
  )
}

export default SubWindowApp
