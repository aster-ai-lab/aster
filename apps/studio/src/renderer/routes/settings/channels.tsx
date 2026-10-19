import { createFileRoute } from '@tanstack/react-router'

import ChannelsSettings from '@renderer/pages/settings/ChannelsSettings/ChannelsSettings'
import { AVAILABLE_CHANNELS } from '@renderer/pages/settings/ChannelsSettings/channelTypes'

const CHANNEL_SECTIONS = AVAILABLE_CHANNELS.map((channel) => channel.type)

export const Route = createFileRoute('/settings/channels')({
  component: ChannelsSettings,
  validateSearch: (search: Record<string, unknown>): { section: (typeof CHANNEL_SECTIONS)[number] } => {
    const section = search.section
    if (typeof section === 'string' && CHANNEL_SECTIONS.includes(section as (typeof CHANNEL_SECTIONS)[number])) {
      return { section: section as (typeof CHANNEL_SECTIONS)[number] }
    }
    return { section: 'feishu' }
  }
})
