import { memo } from 'react'

import { logoInitials } from '@renderer/components/account/entityImage'
import type { ToBotProfile } from '@renderer/im/tobotProfile'
import { cn } from '@renderer/utils/style'

import { ToBossMark } from './ToBossMark'
import { ToBotMark } from './ToBotMark'

const TEXT_BACKGROUNDS = ['#5B8DEF', '#2F9E8F', '#E08A3C', '#7A5AF8', '#E5484D', '#3F7A4A', '#0E7490', '#C45B4A']

function hash(text: string): number {
  let value = 0
  for (const char of text) value = (value * 33 + char.charCodeAt(0)) >>> 0
  return value
}

function digitsOf(value: string): string {
  return value.replace(/\D/g, '')
}

/** A bare phone, including spaces and a +86 prefix. A name that merely contains digits is not one. */
function phoneLike(value: string): boolean {
  const body = value.replace(/[\s\-()]/g, '').replace(/^\+/, '')
  return digitsOf(value).length >= 7 && digitsOf(value).length <= 20 && digitsOf(value) === body
}

/**
 * Letters for a person who has not set a photo.
 * A nickname uses the same initials as a team wordmark. A phone uses its last four digits.
 */
export function avatarText(name: string, phone = ''): string {
  const nick = name.trim()
  if (nick && !phoneLike(nick)) return logoInitials(nick)
  const fromPhone = digitsOf(phone || nick)
  if (fromPhone) return fromPhone.slice(-4)
  return nick ? logoInitials(nick) : ''
}

export function avatarColor(key: string): string {
  return TEXT_BACKGROUNDS[hash(key.trim() || '?') % TEXT_BACKGROUNDS.length]
}

function isImageAvatar(avatar: string | undefined): avatar is string {
  if (!avatar) return false
  return (
    avatar.startsWith('data:image/') ||
    avatar.startsWith('blob:') ||
    avatar.startsWith('file:') ||
    avatar.startsWith('http://') ||
    avatar.startsWith('https://')
  )
}

function letterFontPx(text: string, size: number): number {
  const ratio = text.length >= 4 ? 0.28 : text.length > 1 ? 0.38 : 0.46
  return Math.max(8, Math.round(size * ratio))
}

export function PersonMark({
  name,
  phone = '',
  avatar,
  colorKey,
  rounded = true,
  size = 36
}: {
  name: string
  phone?: string
  avatar?: string
  colorKey?: string
  rounded?: boolean
  size?: number
}) {
  const label = name.trim() || phone.trim()
  const radius = rounded ? 'rounded-[8px]' : ''
  if (isImageAvatar(avatar)) {
    return (
      <span
        data-ui="messages.person-avatar"
        data-kind="image"
        role="img"
        aria-label={label || undefined}
        className={cn('block size-full overflow-hidden', radius)}>
        <img src={avatar} alt="" draggable={false} className="size-full object-cover" />
      </span>
    )
  }
  const text = avatarText(name, phone)
  return (
    <span
      data-ui="messages.person-avatar"
      data-kind="text"
      role="img"
      aria-label={label || undefined}
      className={cn('flex size-full items-center justify-center overflow-hidden bg-[#e5e5e5]', radius)}
      style={text ? { backgroundColor: avatarColor(colorKey || name || phone) } : undefined}>
      {text ? (
        <span className="leading-none font-medium text-white" style={{ fontSize: letterFontPx(text, size) }}>
          {text}
        </span>
      ) : null}
    </span>
  )
}

export function PersonAvatar({
  name,
  phone = '',
  avatar,
  colorKey,
  online = false,
  size = 36,
  className
}: {
  name: string
  phone?: string
  avatar?: string
  colorKey?: string
  online?: boolean
  size?: number
  className?: string
}) {
  return (
    <span
      className={cn('relative inline-flex shrink-0', className)}
      style={className ? undefined : { width: size, height: size }}>
      <PersonMark name={name} phone={phone} avatar={avatar} colorKey={colorKey} size={size} />
      {online ? (
        <span
          data-ui="messages.presence"
          aria-hidden
          className="absolute right-0 bottom-0 size-2.5 rounded-full border-2 border-white bg-[#1C9A4C]"
        />
      ) : null}
    </span>
  )
}

export type GroupFace =
  | { kind: 'person'; name: string; phone?: string; avatar?: string; colorKey?: string }
  | { kind: 'bot'; bot?: 'tobot' | 'toboss'; label: string; profile?: ToBotProfile }

/** WeChat rows: a short row is centered, and nine faces is the cap. */
function wechatRows(count: number): number[] {
  if (count <= 1) return [1]
  if (count === 2) return [2]
  if (count === 3) return [1, 2]
  if (count === 4) return [2, 2]
  if (count === 5) return [2, 3]
  if (count === 6) return [3, 3]
  if (count === 7) return [1, 3, 3]
  if (count === 8) return [2, 3, 3]
  return [3, 3, 3]
}

type GroupTile = { x: number; y: number; size: number }

/** Square tiles in a rounded square. A short row is centered. Tiles never overlap. */
function groupTiles(count: number, box: number): GroupTile[] {
  const total = Math.min(Math.max(count, 1), 9)
  if (total === 1) return [{ x: 0, y: 0, size: box }]
  const rows = wechatRows(total)
  const columns = Math.max(...rows)
  const gutter = Math.max(1, Math.round(box / 18))
  const tile = Math.max(1, Math.floor((box - gutter * (columns + 1)) / columns))
  const block = rows.length * tile + (rows.length - 1) * gutter
  const y0 = Math.floor((box - block) / 2)
  const tiles: GroupTile[] = []
  rows.forEach((rowCount, row) => {
    const rowWidth = rowCount * tile + (rowCount - 1) * gutter
    const x0 = Math.floor((box - rowWidth) / 2)
    const y = y0 + row * (tile + gutter)
    for (let column = 0; column < rowCount; column += 1) {
      tiles.push({ x: x0 + column * (tile + gutter), y, size: tile })
    }
  })
  return tiles
}

function GroupFaceMark({ face, size }: { face: GroupFace; size: number }) {
  if (face.kind === 'bot') {
    return face.bot === 'toboss' ? (
      <ToBossMark size={size} label={face.label} />
    ) : (
      <ToBotMark size={size} label={face.label} profile={face.profile} />
    )
  }
  return (
    <PersonMark
      name={face.name}
      phone={face.phone}
      avatar={face.avatar}
      colorKey={face.colorKey}
      rounded={false}
      size={size}
    />
  )
}

function faceIdentity(face: GroupFace): string {
  if (face.kind === 'bot') return `bot:${face.bot ?? ''}:${face.label}`
  return `person:${face.colorKey ?? ''}:${face.name}:${face.phone ?? ''}:${face.avatar ?? ''}`
}

export const GroupAvatar = memo(function GroupAvatar({
  faces,
  size = 36
}: {
  faces: readonly GroupFace[]
  size?: number
}) {
  const shown = (faces.length > 0 ? faces : [{ kind: 'person' as const, name: '' }]).slice(0, 9)
  const tiles = groupTiles(shown.length, size)
  return (
    <span
      data-ui="messages.group-avatar"
      data-faces={String(shown.length)}
      className="relative shrink-0 overflow-hidden rounded-[8px] bg-[#e5e5e5]"
      style={{ width: size, height: size }}>
      {shown.map((face, index) => {
        const tile = tiles[index]
        return (
          <span
            key={faceIdentity(face)}
            data-ui="messages.group-face"
            className="absolute overflow-hidden"
            style={{ left: tile.x, top: tile.y, width: tile.size, height: tile.size }}>
            <span className="absolute inset-0">
              <GroupFaceMark face={face} size={tile.size} />
            </span>
          </span>
        )
      })}
    </span>
  )
}, facesUnchanged)

function facesUnchanged(
  prev: { faces: readonly GroupFace[]; size?: number },
  next: { faces: readonly GroupFace[]; size?: number }
): boolean {
  if ((prev.size ?? 36) !== (next.size ?? 36) || prev.faces.length !== next.faces.length) return false
  return prev.faces.every((face, index) => faceIdentity(face) === faceIdentity(next.faces[index]))
}

export function AccountAvatar({ kind }: { kind: 'article' | 'wallet' | 'team' | 'badge' | 'bell' | 'transfer' }) {
  if (kind === 'transfer') {
    return (
      <span
        data-ui="messages.account-avatar"
        data-account={kind}
        className="flex size-9 shrink-0 items-center justify-center rounded-[8px] bg-[#5BB866]">
        <svg viewBox="0 0 24 24" className="size-[22px]" aria-hidden="true">
          <path
            d="M4 7.25A2.25 2.25 0 0 1 6.25 5h3.4c.5 0 .98.2 1.33.55l.97.95h5.8A2.25 2.25 0 0 1 20 8.75v8A2.25 2.25 0 0 1 17.75 19H6.25A2.25 2.25 0 0 1 4 16.75v-9.5Z"
            fill="#fff"
          />
          <path
            d="M8.5 12.75h6.25M12.5 10.25l2.5 2.5-2.5 2.5"
            fill="none"
            stroke="#5BB866"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </span>
    )
  }
  return (
    <span
      data-ui="messages.account-avatar"
      data-account={kind}
      className="flex size-9 shrink-0 items-center justify-center rounded-[8px] bg-[#F4F5F7] text-[#171717]">
      <svg
        viewBox="0 0 24 24"
        className="size-5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        aria-hidden="true">
        {kind === 'article' ? (
          <>
            <rect x="5" y="4" width="14" height="16" rx="2" />
            <path d="M8 8h8M8 12h8M8 16h5" />
          </>
        ) : null}
        {kind === 'wallet' ? (
          <>
            <rect x="3.5" y="6" width="17" height="12" rx="2" />
            <path d="M3.5 10h17" />
            <circle cx="16.5" cy="14" r="0.8" fill="currentColor" stroke="none" />
          </>
        ) : null}
        {kind === 'team' ? (
          <>
            <circle cx="9" cy="9" r="2.2" />
            <circle cx="15.5" cy="9.5" r="1.8" />
            <path d="M4.8 17.5c.6-2.4 2.4-3.6 4.2-3.6s3.6 1.2 4.2 3.6" />
            <path d="M14 14.2c1.4-.3 2.8.4 3.6 2.3" />
          </>
        ) : null}
        {kind === 'badge' ? (
          <>
            <path d="M12 3.5 18.5 6v5.2c0 3.4-2.4 6.4-6.5 8.3-4.1-1.9-6.5-4.9-6.5-8.3V6L12 3.5Z" />
            <path d="m9 11.5 2 2 4-4" />
          </>
        ) : null}
        {kind === 'bell' ? (
          <>
            <path d="M6.5 16.5h11l-1.2-2.1V10a4.3 4.3 0 0 0-8.6 0v4.4L6.5 16.5Z" />
            <path d="M10 16.5a2 2 0 0 0 4 0" />
          </>
        ) : null}
      </svg>
    </span>
  )
}
