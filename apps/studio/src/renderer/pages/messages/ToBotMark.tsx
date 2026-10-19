import type { ToBotProfile } from '@renderer/im/tobotProfile'
import { useToBotProfile } from '@renderer/im/tobotProfile'

import { TOBOT_COLOR_HEX, tobotEyeInk, tobotEyes, tobotMouth, tobotShapePath } from './tobotAvatar'

export function ToBotMark({ profile, size = 36, label }: { profile?: ToBotProfile; size?: number; label?: string }) {
  const stored = useToBotProfile()
  const face = profile ?? stored
  const eyes = tobotEyes(face.expression, face.eyes)
  const mouth = tobotMouth(face.expression)
  const ink = tobotEyeInk(face.color)
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-ui="messages.tobot-avatar"
      data-a3s-bot-avatar="true"
      data-shape={face.shape}
      data-expression={face.expression}
      data-color={face.color}
      data-eyes={face.eyes}
      className="shrink-0">
      <path d={tobotShapePath(face.shape)} fill={TOBOT_COLOR_HEX[face.color]} />
      {eyes.map((eye, index) => (
        <ellipse
          key={index}
          cx={eye.x}
          cy={eye.y}
          rx={eye.rx}
          ry={eye.ry}
          fill={ink}
          transform={eye.rot ? `rotate(${eye.rot} ${eye.x} ${eye.y})` : undefined}
        />
      ))}
      {mouth ? <path d={mouth} fill="none" stroke={ink} strokeWidth="2.2" strokeLinecap="round" /> : null}
    </svg>
  )
}
