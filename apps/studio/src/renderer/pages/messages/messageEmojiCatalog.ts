// Compact bilingual index of emoji-picker-element-data CLDR records.
// Annotations, tags, and shortcodes are already folded into `s`.
import compact from './messageEmojiCatalog.data.json'
import { messageEmojiGroupFromCldr, type MessageEmojiGroup } from './messageEmojiGroups'

type CompactEmoji = {
  e: string
  g: number
  n: string
  s: string
  z: string
}

export type MessageEmoji = {
  emoji: string
  group: MessageEmojiGroup
  label: { 'zh-CN': string; 'en-US': string }
  search: string
}

const fold = (value: string): string => value.toLowerCase().replace(/[\s_-]+/g, '')

const buildCatalog = (): MessageEmoji[] => {
  const unique: MessageEmoji[] = []
  for (const record of compact as CompactEmoji[]) {
    const group = messageEmojiGroupFromCldr(record.g)
    if (!group) continue
    unique.push({
      emoji: record.e,
      group,
      label: { 'zh-CN': record.z, 'en-US': record.n },
      search: record.s
    })
  }
  return unique
}

const catalog = buildCatalog()

const byEmoji = new Map(catalog.map((item) => [item.emoji, item]))

export const messageEmojisByGroup: Record<MessageEmojiGroup, MessageEmoji[]> = {
  smileys: [],
  people: [],
  animals: [],
  food: [],
  travel: [],
  activities: [],
  objects: [],
  symbols: [],
  flags: []
}

for (const item of catalog) messageEmojisByGroup[item.group].push(item)

export const searchMessageEmojis = (query: string): MessageEmoji[] => {
  const needle = fold(query)
  if (!needle) return []
  const raw = query.trim()
  return catalog.filter((item) => item.emoji.includes(raw) || item.search.includes(needle))
}

export const messageEmojiLabel = (emoji: string, language: string): string => {
  const item = byEmoji.get(emoji)
  if (!item) return emoji
  return language.toLowerCase().startsWith('zh') ? item.label['zh-CN'] : item.label['en-US']
}
