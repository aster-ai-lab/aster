import { MenuItem, MenuList } from '@a3s-lab/ui'
import { useNavigate, useSearch } from '@tanstack/react-router'
import type { FC } from 'react'
import { useTranslation } from 'react-i18next'

import Scrollbar from '@renderer/components/Scrollbar'
import {
  settingsSubmenuItemClassName,
  settingsSubmenuItemLabelClassName,
  settingsSubmenuListClassName
} from '@renderer/pages/settings/settingsStyles'
import { getChannelTypeIcon } from '@renderer/utils/agentSession'

import ChannelDetail from './ChannelDetail'
import { AVAILABLE_CHANNELS } from './channelTypes'

const ChannelsSettings: FC = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { section } = useSearch({ from: '/settings/channels' })
  const selected = AVAILABLE_CHANNELS.find((channel) => channel.type === section) ?? AVAILABLE_CHANNELS[0]

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1">
      <nav
        aria-label={t('settings.channels.title')}
        className="flex h-full min-h-0 w-[176px] min-w-[176px] shrink-0 flex-col border-r border-[#efefef] bg-white pt-2">
        <Scrollbar className="min-h-0 flex-1">
          <MenuList className={settingsSubmenuListClassName}>
            {AVAILABLE_CHANNELS.map((channel) => {
              const icon = getChannelTypeIcon(channel.type)
              const active = channel.type === selected.type
              return (
                <MenuItem
                  key={channel.type}
                  label={t(channel.titleKey)}
                  active={active}
                  aria-current={active ? 'page' : undefined}
                  onClick={() => void navigate({ to: '/settings/channels', search: { section: channel.type } })}
                  icon={
                    icon ? <img src={icon} alt="" className="size-4 rounded object-contain" aria-hidden /> : undefined
                  }
                  className={settingsSubmenuItemClassName}
                  labelClassName={settingsSubmenuItemLabelClassName}
                />
              )
            })}
          </MenuList>
        </Scrollbar>
      </nav>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <ChannelDetail key={selected.type} channelDef={selected} />
      </div>
    </div>
  )
}

export default ChannelsSettings
