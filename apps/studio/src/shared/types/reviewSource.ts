import * as z from 'zod'

export const REVIEW_TEXT_BYTES = 256 * 1024

/** Explicit review material, separate from the task's instructions. */
export const reviewTextSourceSchema = z.strictObject({
  kind: z.literal('text'),
  text: z
    .string()
    .refine(
      (text) =>
        text.trim().length > 0 && !text.includes('\0') && new TextEncoder().encode(text).length <= REVIEW_TEXT_BYTES
    )
})

export type ReviewTextSource = z.infer<typeof reviewTextSourceSchema>

export const reviewProgressSchema = z
  .strictObject({
    index: z.number().int().positive(),
    total: z.number().int().positive(),
    quote: z.string().max(512)
  })
  .refine((value) => value.index <= value.total)

export type ReviewProgress = z.infer<typeof reviewProgressSchema>
