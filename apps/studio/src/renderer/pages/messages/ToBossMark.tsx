import { A3SBotAvatar } from '@renderer/components/bot/A3SBotAvatar'

/** Fixed ToBoss face. Shape, color, and eyes stay on this variant. */
export function ToBossMark({
  size = 36,
  label,
  animated = false
}: {
  size?: number
  label?: string
  animated?: boolean
}) {
  return (
    <A3SBotAvatar
      animated={animated}
      className="shrink-0"
      label={label}
      size={size}
      ui="messages.toboss-avatar"
      variant="boss"
    />
  )
}
