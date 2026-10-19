import { createContext, type ReactNode, use, useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import type { WorkbenchContributionContext, WorkbenchPluginScope } from '@renderer/services/plugins'
import { STUDIO_CONTRIBUTION_CONTEXT_KEYS } from '@shared/types/studioContribution'

type Appearance = Pick<WorkbenchContributionContext, 'theme' | 'locale'>
const AppearanceContext = createContext<Appearance>({ theme: null, locale: null })

export function PluginContributionContextProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation()
  const read = useCallback(
    (): Appearance => ({
      theme: globalThis.document.documentElement.classList.contains('dark') ? 'dark' : 'light',
      locale: i18n.resolvedLanguage || i18n.language || null
    }),
    [i18n]
  )
  const [appearance, setAppearance] = useState(read)
  useEffect(() => {
    const update = () => {
      const next = read()
      setAppearance((previous) => (next.theme === previous.theme && next.locale === previous.locale ? previous : next))
    }
    update()
    const observer = new globalThis.MutationObserver(update)
    observer.observe(globalThis.document.documentElement, { attributes: true, attributeFilter: ['class'] })
    i18n.on('languageChanged', update)
    return () => {
      observer.disconnect()
      i18n.off('languageChanged', update)
    }
  }, [i18n, read])
  return <AppearanceContext value={appearance}>{children}</AppearanceContext>
}

export function usePluginContributionContext(scope: WorkbenchPluginScope | null): WorkbenchContributionContext {
  const appearance = use(AppearanceContext)
  const active = !!scope?.isOpen()
  return useMemo(
    () =>
      ({
        ...appearance,
        ...Object.fromEntries(STUDIO_CONTRIBUTION_CONTEXT_KEYS.map((key) => [key, active && !!scope?.services[key]]))
      }) as WorkbenchContributionContext,
    [appearance, scope, active]
  )
}
