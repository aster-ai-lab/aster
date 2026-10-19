import { isEqual } from 'es-toolkit/compat'
import * as z from 'zod'

import { STUDIO_EXTENSION_POINT_CONTRACTS } from './studioExtensionPoints.generated'

export const studioExtensionPointContractsSchema = z.custom<typeof STUDIO_EXTENSION_POINT_CONTRACTS>(
  (value) => isEqual(value, STUDIO_EXTENSION_POINT_CONTRACTS),
  'Incompatible Studio extension point capability directory'
)

export const STUDIO_CONTRIBUTION_CONTEXT_KEYS = [
  'workspace',
  'document',
  'conversation',
  'run',
  'turn',
  'harness'
] as const
export const studioContributionConditionSchema = z
  .object({
    all: z.array(z.enum(STUDIO_CONTRIBUTION_CONTEXT_KEYS)).max(6).readonly().optional(),
    none: z.array(z.enum(STUDIO_CONTRIBUTION_CONTEXT_KEYS)).max(6).readonly().optional(),
    theme: z.enum(['light', 'dark']).optional(),
    locale: z
      .string()
      .min(1)
      .max(64)
      .regex(/^[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*$/)
      .optional()
  })
  .strict()
  .superRefine((condition, ctx) => {
    const all = condition.all ?? []
    const none = condition.none ?? []
    if (
      new Set(all).size !== all.length ||
      new Set(none).size !== none.length ||
      all.some((key) => none.includes(key)) ||
      (!all.length && !none.length && !condition.theme && !condition.locale)
    )
      ctx.addIssue({ code: 'custom', message: 'Invalid contribution condition' })
  })
  .readonly()

export type StudioContributionCondition = z.infer<typeof studioContributionConditionSchema>
export type StudioContributionContextKey = (typeof STUDIO_CONTRIBUTION_CONTEXT_KEYS)[number]

/** Native/renderer protocol version; package ACL schema versions evolve separately. */
export const STUDIO_CONTRIBUTION_PROTOCOL_VERSION = 1
export const STUDIO_COMPOSER_SCOPES = ['window', 'workspace', 'session', 'run', 'turn'] as const

export const STUDIO_EXTENSION_POINT_IDS = {
  activity: STUDIO_EXTENSION_POINT_CONTRACTS.activity.id,
  page: STUDIO_EXTENSION_POINT_CONTRACTS.page.id,
  sidebar: STUDIO_EXTENSION_POINT_CONTRACTS.sidebar.id,
  panel: STUDIO_EXTENSION_POINT_CONTRACTS.panel.id,
  workspace: STUDIO_EXTENSION_POINT_CONTRACTS.workspace.id,
  applet: STUDIO_EXTENSION_POINT_CONTRACTS.applet.id,
  composer: STUDIO_EXTENSION_POINT_CONTRACTS.composer.id,
  composerMode: STUDIO_EXTENSION_POINT_CONTRACTS.composerMode.id,
  harness: STUDIO_EXTENSION_POINT_CONTRACTS.harness.id,
  command: STUDIO_EXTENSION_POINT_CONTRACTS.command.id,
  menu: STUDIO_EXTENSION_POINT_CONTRACTS.menu.id,
  keybinding: STUDIO_EXTENSION_POINT_CONTRACTS.keybinding.id,
  customEditor: STUDIO_EXTENSION_POINT_CONTRACTS.customEditor.id,
  home: STUDIO_EXTENSION_POINT_CONTRACTS.home.id
} as const

export type StudioExtensionPointId = (typeof STUDIO_EXTENSION_POINT_IDS)[keyof typeof STUDIO_EXTENSION_POINT_IDS]
export type StudioCompositionMode =
  (typeof STUDIO_EXTENSION_POINT_CONTRACTS)[keyof typeof STUDIO_EXTENSION_POINT_CONTRACTS]['modes'][number]
export const STUDIO_UI_POINTS = [
  'studio.editor',
  'studio.sidebar',
  'studio.panel',
  'studio.workspace',
  'studio.applets',
  'studio.composer.actions',
  'studio.editor.custom'
] as const
