import type { StudioContributionCondition, StudioContributionContextKey } from '@shared/types/studioContribution'

export type WorkbenchContributionContext = Readonly<
  Record<StudioContributionContextKey, boolean> & { theme: 'light' | 'dark' | null; locale: string | null }
>

/** Visibility observes host facts; it grants no resource or execution authority. */
export function matchesWorkbenchCondition(
  condition: StudioContributionCondition | undefined,
  context: WorkbenchContributionContext
): boolean {
  if (!condition) return true
  return (
    (condition.all ?? []).every((key) => context[key] === true) &&
    (condition.none ?? []).every((key) => context[key] === false) &&
    (!condition.theme || condition.theme === context.theme) &&
    (!condition.locale || condition.locale.toLowerCase() === context.locale?.toLowerCase())
  )
}
