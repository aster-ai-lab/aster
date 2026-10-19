import * as z from 'zod'

import { STUDIO_CONTRIBUTION_CONTEXT_KEYS } from './studioContribution'
import { STUDIO_EXTENSION_POINT_CONTRACTS } from './studioExtensionPoints.generated'

const id = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[a-z][a-z0-9-]*$/)

const controlId = z.string().regex(/^[a-z][a-z0-9_]{0,63}$/)

const composerModeControlSchema = z
  .object({
    id: controlId,
    kind: z.enum(['number', 'select', 'toggle', 'text']),
    label: z.string().refine((value) => value.trim().length > 0 && [...value].length <= 32),
    min: z.number().int().min(1).max(100).optional(),
    max: z.number().int().min(1).max(8000).optional(),
    options: z.array(z.string().min(1).max(200)).max(128).optional(),
    optionLabels: z.array(z.string().min(1).max(32)).max(128).optional(),
    initial: z.union([z.string().max(200), z.boolean()]).optional()
  })
  .strict()

const composerModeTaskSchema = z
  .object({
    id,
    title: z.string().refine((value) => value.trim().length > 0 && [...value].length <= 32),
    prompt: z.string().refine((value) => value.trim().length > 0 && [...value].length <= 240)
  })
  .strict()

function withoutUndefined<T extends Record<string, unknown>>(value: T): T {
  const next: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(value)) {
    if (item !== undefined) next[key] = item
  }
  return next as T
}

export const aixComposerModeSchema = z
  .object({
    id,
    contractVersion: z.literal(STUDIO_EXTENSION_POINT_CONTRACTS.composerMode.contractVersion),
    title: z.string().refine((value) => value.trim().length > 0 && [...value].length <= 64),
    description: z.string().refine((value) => [...value].length <= 240),
    icon: id,
    agent: id,
    editor: id.optional(),
    keptNotice: z.enum(['draft', 'editor']).optional(),
    scope: z.enum(STUDIO_EXTENSION_POINT_CONTRACTS.composerMode.scopes),
    context: z
      .array(z.enum(STUDIO_CONTRIBUTION_CONTEXT_KEYS))
      .max(6)
      .refine((keys) => new Set(keys).size === keys.length)
      .readonly(),
    order: z.number().int().min(-2147483648).max(2147483647),
    controls: z.array(composerModeControlSchema).max(8).optional(),
    tasks: z.array(composerModeTaskSchema).max(8).optional(),
    prompt: z
      .string()
      .refine((value) => value.trim().length > 0 && [...value].length <= 80)
      .optional()
  })
  .strict()
  .transform((mode) => {
    const next = withoutUndefined(mode as unknown as Record<string, unknown>)
    if (Array.isArray(next.controls)) {
      next.controls = next.controls.map((control) => withoutUndefined(control as Record<string, unknown>))
    }
    if (Array.isArray(next.tasks)) {
      next.tasks = next.tasks.map((task) => withoutUndefined(task as Record<string, unknown>))
    }
    return next as typeof mode
  })
  .readonly()

export type AixComposerMode = z.infer<typeof aixComposerModeSchema>
