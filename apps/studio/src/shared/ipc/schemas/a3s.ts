import * as z from 'zod'

import { defineRoute } from '../define'

/**
 * A3S IPC schemas — provider-specific sign-in, balance and logout operations.
 *
 * The OAuth engine is shared with Codex and Grok, while the renderer-facing
 * contract stays here because A3S supplies hosts and receives API keys.
 */

/** The A3S account profile, or null when the profile endpoint has nothing. */
const a3sProfileSchema = z.object({
  displayName: z.string().nullable(),
  username: z.string().nullable(),
  email: z.string().nullable(),
  group: z.string().nullable()
})

/** Balance plus optional profile, returned to the settings panel. */
const a3sBalanceSchema = z.object({
  balance: z.number(),
  profile: a3sProfileSchema.nullable()
})

export type A3sProfile = z.infer<typeof a3sProfileSchema>
export type A3sBalance = z.infer<typeof a3sBalanceSchema>

const apiHostInput = z.object({ apiHost: z.string() })

export const a3sRequestSchemas = {
  'a3s.sign_in': defineRoute({
    input: z.object({ requestId: z.string().min(1), oauthServer: z.string(), apiHost: z.string().optional() }),
    output: z.object({ apiKeys: z.string().min(1) })
  }),
  'a3s.get_balance': defineRoute({ input: apiHostInput, output: a3sBalanceSchema }),
  'a3s.logout': defineRoute({ input: apiHostInput, output: z.void() })
}
