import { preferenceService } from '@data/PreferenceService'

export const CUSTOM_INSTRUCTION_MAX_CHARS = 8000

const KEY = 'app.custom_instructions' as const

export class CustomInstructionTooLargeError extends Error {
  constructor() {
    super('custom-instruction-too-large')
    this.name = 'CustomInstructionTooLargeError'
  }
}

export async function loadCustomInstructions(): Promise<string> {
  const value = await preferenceService.get(KEY)
  return typeof value === 'string' ? value : ''
}

export async function saveCustomInstructions(text: string): Promise<void> {
  if (text.length > CUSTOM_INSTRUCTION_MAX_CHARS) throw new CustomInstructionTooLargeError()
  await preferenceService.set(KEY, text)
}
