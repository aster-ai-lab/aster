import { type CSSProperties, useEffect, useRef, useState } from 'react'

import './bot-avatar.css'
import {
  type A3SBotAvatarActivity,
  type A3SBotAvatarGaze,
  type A3SBotAvatarMotionInput,
  a3sBotAvatarEyePaths,
  attachA3SBotAvatarMotion
} from './bot-avatar-character'

export type {
  A3SBotAvatarActivity,
  A3SBotAvatarGaze,
  A3SBotAvatarIdleDirection,
  A3SBotAvatarIdleGlance,
  A3SBotAvatarIdlePose,
  A3SBotAvatarIdleSequence,
  A3SBotAvatarIdleStep
} from './bot-avatar-character'
export {
  advanceA3SBotAvatarIdleSequence,
  calculateA3SBotAvatarGaze,
  nextA3SBotAvatarBlinkDelay,
  nextA3SBotAvatarIdleGlance,
  nextA3SBotAvatarThinkingGlance
} from './bot-avatar-character'

export interface A3SBotAvatarProps {
  activity?: A3SBotAvatarActivity
  /** Disable idle/blink work for older transcript rows to keep long lists cheap. */
  animated?: boolean
  className?: string
  color?: string
  fill?: string
  flowingFill?: boolean
  /** Fixed pupil offset; wins over idleGaze and pointer tracking. */
  gaze?: A3SBotAvatarGaze
  idleGaze?: boolean
  label?: string
  tracking?: boolean
  /** Stable hook for the surface that owns this face, for example messages.toboss-avatar. */
  ui?: string
  /** Optional visual identity layered onto the shared animated face. */
  variant?: A3SBotAvatarVariant
  /** Edge length. The document-expert face scales from this, including the eyes. */
  size?: number
}

export type A3SBotAvatarVariant = 'default' | 'scientist' | 'boss'

interface A3SBotAvatarStyle extends CSSProperties {
  '--a3s-bot-avatar-color'?: string
  '--a3s-bot-avatar-fill'?: string
  '--a3s-bot-avatar-size'?: string
}

const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)'

function initiallyPrefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia(REDUCED_MOTION_QUERY).matches
  )
}

export function A3SBotAvatar({
  activity = 'idle',
  animated = true,
  className,
  color,
  fill,
  flowingFill = false,
  gaze,
  idleGaze = true,
  label,
  size,
  tracking = false,
  ui,
  variant = 'default'
}: A3SBotAvatarProps) {
  const motionRef = useRef<HTMLSpanElement>(null)
  const motionInput = useRef<A3SBotAvatarMotionInput>({
    activity,
    gaze: gaze ?? null,
    idleGaze,
    tracking
  })
  const [reducedMotion, setReducedMotion] = useState(initiallyPrefersReducedMotion)
  motionInput.current = {
    activity,
    gaze: gaze ?? null,
    idleGaze,
    tracking
  }

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const preference = window.matchMedia(REDUCED_MOTION_QUERY)
    const updatePreference = () => setReducedMotion(preference.matches)
    updatePreference()
    preference.addEventListener('change', updatePreference)
    return () => preference.removeEventListener('change', updatePreference)
  }, [])

  useEffect(() => {
    const host = motionRef.current
    if (!host || reducedMotion || !animated) return
    return attachA3SBotAvatarMotion(host, () => motionInput.current)
  }, [animated, reducedMotion])

  const eyes = a3sBotAvatarEyePaths(activity)
  const style: A3SBotAvatarStyle | undefined =
    color || fill || size != null
      ? {
          ...(size != null ? { '--a3s-bot-avatar-size': `${size}px` } : {}),
          ...(color || fill
            ? {
                '--a3s-bot-avatar-color': color,
                '--a3s-bot-avatar-fill': fill
              }
            : {})
        }
      : undefined
  return (
    <div
      aria-hidden={label ? undefined : 'true'}
      aria-label={label}
      className={['a3s-bot-avatar', className].filter(Boolean).join(' ')}
      data-a3s-bot-avatar="true"
      data-ui={ui}
      data-animation-enabled={animated ? 'true' : 'false'}
      data-avatar-shape="blob"
      data-bot-activity={activity}
      data-flowing-fill={flowingFill ? 'true' : undefined}
      data-gaze-mode={reducedMotion || (!tracking && !idleGaze) ? 'static' : tracking ? 'pointer' : 'idle'}
      data-pointer-tracking={tracking ? 'true' : 'false'}
      data-avatar-variant={variant}
      role={label ? 'img' : undefined}
      style={style}>
      <span className="a3s-bot-avatar__motion" ref={motionRef}>
        <span className="a3s-bot-avatar__face">
          <span aria-hidden="true" className="a3s-bot-avatar__blob" />
          {variant === 'boss' ? (
            <>
              <svg aria-hidden="true" className="a3s-bot-avatar__crown" focusable="false" viewBox="0 0 64 28">
                <path d="M4 24 L4 15 L16 19 L32 5 L48 19 L60 15 L60 24 Z" fill="currentColor" />
              </svg>
              <span aria-hidden="true" className="a3s-bot-avatar__tie" />
            </>
          ) : null}
          <span className="a3s-bot-avatar__eyes">
            <svg aria-hidden="true" className="a3s-bot-avatar__eye-mark" focusable="false" viewBox="0 0 64 32">
              <path className="a3s-bot-avatar__eye" d={eyes.left} data-a3s-bot-avatar-eye="left" fill="currentColor" />
              <path
                className="a3s-bot-avatar__eye"
                d={eyes.right}
                data-a3s-bot-avatar-eye="right"
                fill="currentColor"
              />
            </svg>
            {variant === 'scientist' ? (
              <span aria-hidden="true" className="a3s-bot-avatar__glasses">
                <span className="a3s-bot-avatar__glasses-lens a3s-bot-avatar__glasses-lens--left" />
                <span className="a3s-bot-avatar__glasses-bridge" />
                <span className="a3s-bot-avatar__glasses-lens a3s-bot-avatar__glasses-lens--right" />
              </span>
            ) : null}
          </span>
        </span>
      </span>
    </div>
  )
}
