import * as z from 'zod'

import { aixComposerModeSchema } from './aixComposerMode'
import {
  STUDIO_COMPOSER_SCOPES,
  STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
  STUDIO_UI_POINTS,
  studioContributionConditionSchema,
  studioExtensionPointContractsSchema
} from './studioContribution'
import { STUDIO_EXTENSION_POINT_CONTRACTS } from './studioExtensionPoints.generated'

const id = z.string().regex(/^[a-z][a-z0-9-]{0,127}$/)
const slug = z.string().regex(/^[a-z][a-z0-9-]{0,63}$/)
const text = (limit: number) => z.string().refine((value) => [...value].length <= limit)
const order = z.number().int().min(-2147483648).max(2147483647)
const commandSchema = z
  .object({
    id,
    title: text(64).refine((value) => value.trim().length > 0),
    icon: id.optional(),
    enablement: studioContributionConditionSchema.optional()
  })
  .strict()
  .readonly()
const menuSchema = z
  .object({
    location: z.enum(['view.title', 'view.context']),
    command: id,
    order,
    when: studioContributionConditionSchema.optional()
  })
  .strict()
  .readonly()
const viewSchema = z
  .object({
    id,
    title: text(64),
    description: text(240).default(''),
    point: z.enum(STUDIO_UI_POINTS),
    contractVersion: z.literal(STUDIO_EXTENSION_POINT_CONTRACTS.page.contractVersion).optional(),
    mode: z.enum(STUDIO_EXTENSION_POINT_CONTRACTS.page.modes).optional(),
    scope: z.enum(STUDIO_COMPOSER_SCOPES).optional(),
    when: studioContributionConditionSchema.optional(),
    commands: z.array(commandSchema).max(32).readonly().optional(),
    menus: z.array(menuSchema).max(64).readonly().optional(),
    presentation: z.enum(['webview', 'tree']).optional(),
    selector: z
      .string()
      .min(1)
      .max(128)
      .regex(/^(?!\.{1,2}$)(?!\/)(?:[A-Za-z0-9_.*?-]+\/)*[A-Za-z0-9_.*?-]+$/)
      .optional(),
    priority: z.enum(['default', 'option']).optional(),
    icon: id,
    order,
    container: id.nullable()
  })
  .strict()
const containerSchema = z
  .object({
    id,
    title: text(64).refine((value) => value.trim().length > 0),
    icon: id,
    order,
    location: z.enum(['activitybar', 'panel']),
    when: studioContributionConditionSchema.optional()
  })
  .strict()
const packageSchema = z
  .object({
    slug,
    packageId: z.string().regex(/^use\/[a-z][a-z0-9_.-]*\/[a-z][a-z0-9_.-]*$/),
    packageDigest: z
      .string()
      .regex(/^sha256:[a-f0-9]{64}$/)
      .optional(),
    generation: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    revision: z.string().regex(/^sha256:[a-f0-9]{64}$/),
    containers: z.array(containerSchema).max(32),
    views: z.array(viewSchema).max(64),
    composerModes: z.array(aixComposerModeSchema).max(32).readonly().optional(),
    commands: z
      .array(
        z
          .object({
            id,
            title: text(64).refine((value) => value.trim().length > 0),
            category: text(64)
              .refine((value) => value.trim().length > 0)
              .optional(),
            icon: id.optional(),
            enablement: studioContributionConditionSchema.optional()
          })
          .strict()
          .readonly()
      )
      .max(32)
      .readonly()
      .optional(),
    menus: z
      .array(
        z
          .object({
            location: z.enum(['commandPalette', 'editor/context', 'editor/title']),
            command: id,
            order,
            when: studioContributionConditionSchema.optional()
          })
          .strict()
          .readonly()
      )
      .max(64)
      .readonly()
      .optional(),
    home: z
      .array(
        z
          .object({
            id,
            shelf: z.enum(['write', 'create', 'process']),
            title: text(64).refine((value) => value.trim().length > 0),
            description: text(240),
            order,
            action: z.enum(['writing', 'applet', 'file', 'unavailable']),
            seed: text(240).optional(),
            tag: text(32).optional(),
            writingType: z.enum(['notice', 'report', 'request', 'minutes']).optional(),
            applet: z.enum(['plan', 'pdf-tools']).optional(),
            fileKind: z.enum(['pdf-to-word', 'word-to-pdf']).optional(),
            format: z.enum(['pdf', 'word']).optional(),
            cover: z.enum(['ppt', 'site', 'video', 'image']).optional()
          })
          .strict()
          .readonly()
      )
      .max(32)
      .readonly()
      .optional(),
    harness: z
      .array(
        z
          .object({
            id,
            component: z.string().regex(/^[a-z][a-z0-9-]{0,63}$/)
          })
          .strict()
          .readonly()
      )
      .max(32)
      .readonly()
      .optional(),
    keybindings: z
      .array(
        z
          .object({
            command: id,
            key: z.string().min(3).max(64),
            mac: z.string().min(3).max(64).optional(),
            when: studioContributionConditionSchema.optional()
          })
          .strict()
          .readonly()
      )
      .max(64)
      .readonly()
      .optional()
  })
  .strict()
  .superRefine((item, ctx) => {
    const containers = new Set(item.containers.map((container) => container.id))
    if (
      containers.size !== item.containers.length ||
      new Set(item.views.map((view) => view.id)).size !== item.views.length ||
      new Set((item.composerModes ?? []).map((mode) => mode.id)).size !== (item.composerModes ?? []).length ||
      (item.composerModes ?? []).some(
        (mode) =>
          mode.editor !== undefined &&
          !item.views.some(
            (view) =>
              view.id === mode.editor &&
              view.container === null &&
              view.presentation !== 'tree' &&
              view.point !== 'studio.composer.actions'
          )
      ) ||
      item.views.some((view) => {
        const commands = view.commands ?? []
        const menus = view.menus ?? []
        if (
          new Set(commands.map((command) => command.id)).size !== commands.length ||
          new Set(menus.map((menu) => `${menu.location}/${menu.command}`)).size !== menus.length ||
          menus.some((menu) => !commands.some((command) => command.id === menu.command)) ||
          ((commands.length > 0 || menus.length > 0) && view.point === 'studio.composer.actions')
        )
          return true
        if (view.scope !== undefined && view.point !== 'studio.composer.actions') return true
        if (view.point === 'studio.editor.custom') {
          if (!view.selector || view.container !== null || view.presentation === 'tree') return true
        } else if (view.selector !== undefined || view.priority !== undefined) return true
        if (view.presentation === 'tree' && view.container === null) return true
        const container = item.containers.find((container) => container.id === view.container)
        if (view.container === null) return view.point === 'studio.sidebar' || view.point === 'studio.panel'
        return !container || view.point !== (container.location === 'activitybar' ? 'studio.sidebar' : 'studio.panel')
      }) ||
      item.containers.some((container) => !item.views.some((view) => view.container === container.id)) ||
      (item.menus ?? []).some((menu) => !(item.commands ?? []).some((command) => command.id === menu.command)) ||
      (item.keybindings ?? []).some(
        (binding) => !(item.commands ?? []).some((command) => command.id === binding.command)
      )
    ) {
      ctx.addIssue({ code: 'custom', message: 'Invalid AIX view ownership' })
    }
  })

export const aixWorkbenchCatalogueSchema = z
  .object({
    protocolVersion: z.literal(STUDIO_CONTRIBUTION_PROTOCOL_VERSION),
    extensionPoints: studioExtensionPointContractsSchema.optional(),
    revision: z.string().regex(/^(sha256:[a-f0-9]{64})?$/),
    packages: z.array(packageSchema).max(128),
    diagnostics: z
      .array(
        z
          .object({
            slug: slug.nullable(),
            reason: z.enum(['package-unavailable', 'package-content-refused'])
          })
          .strict()
      )
      .max(128)
      .optional()
  })
  .strict()
  .superRefine((catalogue, ctx) => {
    if (
      new Set(catalogue.packages.map((item) => item.slug)).size !== catalogue.packages.length ||
      new Set(catalogue.packages.map((item) => item.packageId)).size !== catalogue.packages.length
    ) {
      ctx.addIssue({ code: 'custom', message: 'Duplicate AIX package identity' })
    }
  })

export type AixWorkbenchCatalogue = z.infer<typeof aixWorkbenchCatalogueSchema>
export type AixWorkbenchPackage = AixWorkbenchCatalogue['packages'][number]
export type AixWorkbenchView = AixWorkbenchPackage['views'][number]

export const AIX_VIEW_SESSION_PROTOCOL_VERSION = 3
export const AIX_VIEW_ADMISSION_LIFETIME_MS = 30_000
export const AIX_VIEW_CANCELLATION_TIMEOUT_MS = 5000
export interface AixViewAdmission {
  readonly requestId: string
  readonly activationId: string
  readonly instanceId: string
  readonly expiresAt: number
}
const admission = {
  protocolVersion: z.literal(AIX_VIEW_SESSION_PROTOCOL_VERSION),
  requestId: z.string().regex(/^[a-f0-9]{64}$/),
  activationId: z.string().regex(/^[a-f0-9]{64}$/),
  instanceId: z.string().regex(/^[a-f0-9]{64}$/),
  expiresAt: z.number().int().positive().max(Number.MAX_SAFE_INTEGER)
}
export const aixViewSessionSchema = z
  .object({ ...admission, token: z.string().regex(/^[a-f0-9]{64}$/), path: z.string() })
  .strict()
export const aixViewCancellationSchema = z.object({ ...admission, closed: z.literal(true) }).strict()
