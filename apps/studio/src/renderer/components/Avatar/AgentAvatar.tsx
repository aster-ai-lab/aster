import { type CSSProperties, useState } from 'react'

import { useCatalogIconSrc } from '@renderer/hooks/useCatalogIconSrc'
import { DEFAULT_AGENT_AVATAR } from '@renderer/utils/agent'
import { cn } from '@renderer/utils/style'

import { agentAvatarInitial, readAgentAvatar } from './agentAvatarValue'

export interface AgentAvatarProps {
  avatar?: string | null
  /** The agent name; its first letter draws when the avatar cannot. */
  name?: string
  size: number
  /** `tile` draws the mark on its fill; `plain` draws a glyph bare, for an inline text row. */
  glyphStyle?: 'tile' | 'plain'
  /** `background` paints an image as a CSS background, for editor surfaces where an `<img>` draws broken. */
  imagePaint?: 'img' | 'background'
  className?: string
  'data-ui'?: string
}

/** DESIGN.md marks: 4px at 16px, 8px at 32px, 10px at 40px. */
export function agentAvatarRadius(size: number): number {
  return Math.max(3, Math.round(size / 4))
}

const TILE = 'bg-[#f2f3f5] shadow-[inset_0_0_0_0.5px_rgba(0,0,0,0.06)]'

/** The one renderer for an agent avatar string. It never prints an address as text. */
export function AgentAvatar({
  avatar,
  name,
  size,
  glyphStyle = 'tile',
  imagePaint = 'img',
  className,
  'data-ui': dataUi
}: AgentAvatarProps) {
  const value = readAgentAvatar(avatar)
  const catalogSrc = useCatalogIconSrc(value?.kind === 'catalog-icon' ? value.path : undefined)
  const [failedSrc, setFailedSrc] = useState<string | null>(null)
  const src = value?.kind === 'image' ? value.src : catalogSrc
  const box: CSSProperties = { width: size, height: size, borderRadius: agentAvatarRadius(size) }
  const frame = 'relative flex shrink-0 items-center justify-center overflow-hidden select-none'

  if (src && imagePaint === 'background') {
    return (
      <span
        aria-hidden
        data-ui={dataUi}
        data-avatar="image"
        className={cn('inline-block shrink-0 bg-cover bg-center', TILE, className)}
        style={{ ...box, backgroundImage: `url("${src.replace(/["\\\n\r]/g, '')}")` }}
      />
    )
  }

  if (src && src !== failedSrc) {
    return (
      <span aria-hidden data-ui={dataUi} data-avatar="image" className={cn(frame, TILE, className)} style={box}>
        <img src={src} alt="" draggable={false} className="size-full object-cover" onError={() => setFailedSrc(src)} />
      </span>
    )
  }

  const glyph = value?.kind === 'glyph' ? value.glyph : undefined
  const initial = glyph ? '' : agentAvatarInitial(name)
  if (initial) {
    return (
      <span
        aria-hidden
        data-ui={dataUi}
        data-avatar="initial"
        className={cn(frame, 'leading-none font-medium text-[#3f3f3f]', TILE, className)}
        style={{ ...box, fontSize: Math.max(9, Math.round(size * 0.4)) }}>
        {initial}
      </span>
    )
  }

  const shown = glyph ?? DEFAULT_AGENT_AVATAR
  const plain = glyphStyle === 'plain'
  return (
    <span
      aria-hidden
      data-ui={dataUi}
      data-avatar="glyph"
      className={cn(frame, 'leading-none', !plain && TILE, className)}
      style={{ ...box, fontSize: Math.round(size * (plain ? 0.8 : 0.56)) }}>
      {shown}
    </span>
  )
}
