import * as z from 'zod'

export const SlashCommandSchema = z.strictObject({
  command: z.string(),
  description: z.string().optional(),
  /** `conversation`: the command acts on an existing conversation and does nothing in a new one. */
  availability: z.enum(['always', 'conversation']).optional(),
  /** Studio already exposes this control in its own UI (for example the model picker). */
  uiControlled: z.boolean().optional()
})

export type SlashCommand = z.infer<typeof SlashCommandSchema>

/** Commands worth offering before a conversation exists, in a composer that has its own controls. */
export function newConversationSlashCommands<T extends SlashCommand>(commands: readonly T[]): T[] {
  return commands.filter((command) => command.availability !== 'conversation' && !command.uiControlled)
}
