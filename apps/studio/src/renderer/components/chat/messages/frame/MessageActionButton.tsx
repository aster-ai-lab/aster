import type { ComponentProps, ReactNode } from 'react'

import './message-action-menu.css'

import ActionIconButton from '@renderer/components/ActionIconButton'
import { classNames } from '@renderer/utils/style'

type MessageActionButtonProps = Omit<ComponentProps<typeof ActionIconButton>, 'icon'> & {
  children?: ReactNode
  softHoverBg?: boolean
}

export const MessageActionButton = ({
  active,
  children,
  className,
  softHoverBg,
  type,
  ...props
}: MessageActionButtonProps) => {
  return (
    <ActionIconButton
      type={type ?? 'button'}
      active={active}
      icon={children}
      className={classNames(
        'message-action-button flex size-6 items-center justify-center rounded-[6px] border-0 bg-transparent p-0 text-[#3f3f3f] transition-colors duration-150 ease-out motion-reduce:transition-none',
        '[&_svg]:size-3.5 [&_svg]:stroke-[1.7]',
        'enabled:cursor-pointer enabled:hover:bg-[#f2f3f5] enabled:hover:text-[#171717]',
        'disabled:cursor-not-allowed disabled:opacity-40',
        active && 'is-active bg-[#f2f3f5] text-[#171717]',
        className
      )}
      data-message-action-hover={softHoverBg ? 'soft' : undefined}
      {...props}
    />
  )
}
