/** CLDR groups shown in the message composer. Skin-tone components (group 2) are omitted. */
export const MESSAGE_EMOJI_GROUPS = [
  'smileys',
  'people',
  'animals',
  'food',
  'travel',
  'activities',
  'objects',
  'symbols',
  'flags'
] as const

export type MessageEmojiGroup = (typeof MESSAGE_EMOJI_GROUPS)[number]

export const MESSAGE_EMOJI_GROUP_LABEL: Record<MessageEmojiGroup, string> = {
  smileys: 'emoji_picker.categories.smileys_emotion',
  people: 'emoji_picker.categories.people_body',
  animals: 'emoji_picker.categories.animals_nature',
  food: 'emoji_picker.categories.food_drink',
  travel: 'emoji_picker.categories.travel_places',
  activities: 'emoji_picker.categories.activities',
  objects: 'emoji_picker.categories.objects',
  symbols: 'emoji_picker.categories.symbols',
  flags: 'emoji_picker.categories.flags'
}

const CLDR_GROUP: Record<number, MessageEmojiGroup | undefined> = {
  0: 'smileys',
  1: 'people',
  3: 'animals',
  4: 'food',
  5: 'travel',
  6: 'activities',
  7: 'objects',
  8: 'symbols',
  9: 'flags'
}

export const messageEmojiGroupFromCldr = (group: number): MessageEmojiGroup | undefined => CLDR_GROUP[group]
