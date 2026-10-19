import { Dices } from 'lucide-react'
import { type KeyboardEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import {
  AVATAR_ACCESSORIES,
  AVATAR_BACKGROUNDS,
  AVATAR_EYE_STYLES,
  AVATAR_HAIR_COLORS,
  AVATAR_HAIR_STYLES,
  AVATAR_MOUTH_STYLES,
  AVATAR_SKINS,
  avatarDataUrl,
  randomAvatarSpec,
  type AvatarSpec
} from './composedAvatar'

const THUMB_CLASS =
  'size-11 overflow-hidden rounded-[8px] bg-white outline-none focus-visible:ring-2 focus-visible:ring-black/10'

const PART_CROP = {
  hair: 'origin-[50%_18%] scale-[1.85]',
  eyes: 'origin-[50%_62%] scale-[2.65]',
  mouth: 'origin-[50%_78%] scale-[2.9]',
  accessory: 'origin-[50%_58%] scale-[1.35]'
} as const

function selectedRing(selected: boolean): string {
  return selected ? 'ring-2 ring-[#171717] ring-offset-2 ring-offset-[#fafafa]' : 'ring-1 ring-[#e5e5e5]'
}

function moveRadio(event: KeyboardEvent<HTMLDivElement>) {
  if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return
  const radios = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]')]
  const index = radios.indexOf(document.activeElement as HTMLButtonElement)
  if (index < 0) return
  event.preventDefault()
  const step = event.key === 'ArrowRight' ? 1 : radios.length - 1
  const next = radios[(index + step) % radios.length]
  next.focus()
  next.click()
}

function OptionGroup({ label, children, muted = false }: { label: string; children: ReactNode; muted?: boolean }) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={cn('grid gap-1.5', muted && 'opacity-40')}
      onKeyDown={moveRadio}>
      <span className="text-[12px] leading-4 text-[#8a8a8a]">{label}</span>
      <div className="flex flex-wrap gap-2">{children}</div>
    </div>
  )
}

function ColorChoice({
  color,
  label,
  selected,
  disabled,
  onSelect
}: {
  color: string
  label: string
  selected: boolean
  disabled?: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      aria-label={label}
      disabled={disabled}
      onClick={onSelect}
      className={cn(
        'size-8 rounded-[8px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40',
        selectedRing(selected)
      )}
      style={{ backgroundColor: color }}
    />
  )
}

function PartChoice({
  spec,
  label,
  crop,
  selected,
  disabled,
  onSelect
}: {
  spec: AvatarSpec
  label: string
  crop: string
  selected: boolean
  disabled?: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      aria-label={label}
      disabled={disabled}
      onClick={onSelect}
      className={cn(THUMB_CLASS, selectedRing(selected), 'disabled:opacity-40')}>
      <img src={avatarDataUrl(spec)} alt="" className={cn('size-full object-cover', crop)} />
    </button>
  )
}

/** Picks one option in each avatar layer. The parent stores the resulting image. */
export function AvatarComposer({
  spec,
  disabled,
  onChange
}: {
  spec: AvatarSpec
  disabled?: boolean
  onChange: (spec: AvatarSpec) => void
}) {
  const { t } = useTranslation()
  const choose = (patch: Partial<AvatarSpec>) => onChange({ ...spec, ...patch })
  const swatchLabel = (group: string, index: number) => `${group} ${index + 1}`
  const bald = AVATAR_HAIR_STYLES[spec.hair] === 'none'

  return (
    <div
      data-ui="account.avatar-composer"
      className="grid gap-3 rounded-[12px] border border-[#e5e5e5] bg-[#fafafa] p-3">
      <div className="sticky top-0 z-10 -mx-3 flex items-center gap-3 bg-[#fafafa] px-3 py-1">
        <img
          data-ui="account.avatar-preview"
          src={avatarDataUrl(spec)}
          alt=""
          className="size-16 shrink-0 rounded-[12px] bg-white"
        />
        <button
          type="button"
          disabled={disabled}
          onClick={() => onChange(randomAvatarSpec())}
          className="inline-flex h-9 items-center gap-1.5 rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          <Dices className="size-3.5" strokeWidth={1.7} aria-hidden />
          {t('sidebar.center.avatar.random')}
        </button>
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-3">
        <OptionGroup label={t('sidebar.center.avatar.background')}>
          {AVATAR_BACKGROUNDS.map((color, index) => (
            <ColorChoice
              key={color}
              color={color}
              label={swatchLabel(t('sidebar.center.avatar.background'), index)}
              selected={spec.background === index}
              disabled={disabled}
              onSelect={() => choose({ background: index })}
            />
          ))}
        </OptionGroup>
        <OptionGroup label={t('sidebar.center.avatar.skin')}>
          {AVATAR_SKINS.map((color, index) => (
            <ColorChoice
              key={color}
              color={color}
              label={swatchLabel(t('sidebar.center.avatar.skin'), index)}
              selected={spec.skin === index}
              disabled={disabled}
              onSelect={() => choose({ skin: index })}
            />
          ))}
        </OptionGroup>
      </div>
      <OptionGroup label={t('sidebar.center.avatar.hair')}>
        {AVATAR_HAIR_STYLES.map((style, index) => (
          <PartChoice
            key={style}
            spec={{ ...spec, hair: index }}
            crop={PART_CROP.hair}
            label={t(`sidebar.center.avatar.hair.${style}`)}
            selected={spec.hair === index}
            disabled={disabled}
            onSelect={() => choose({ hair: index })}
          />
        ))}
      </OptionGroup>
      <OptionGroup label={t('sidebar.center.avatar.hair.color')} muted={bald}>
        {AVATAR_HAIR_COLORS.map((color, index) => (
          <ColorChoice
            key={color}
            color={color}
            label={swatchLabel(t('sidebar.center.avatar.hair.color'), index)}
            selected={spec.hairColor === index}
            disabled={disabled}
            onSelect={() => choose({ hairColor: index })}
          />
        ))}
      </OptionGroup>
      <OptionGroup label={t('sidebar.center.avatar.eyes')}>
        {AVATAR_EYE_STYLES.map((style, index) => (
          <PartChoice
            key={style}
            spec={{ ...spec, eyes: index }}
            crop={PART_CROP.eyes}
            label={t(`sidebar.center.avatar.eyes.${style}`)}
            selected={spec.eyes === index}
            disabled={disabled}
            onSelect={() => choose({ eyes: index })}
          />
        ))}
      </OptionGroup>
      <OptionGroup label={t('sidebar.center.avatar.mouth')}>
        {AVATAR_MOUTH_STYLES.map((style, index) => (
          <PartChoice
            key={style}
            spec={{ ...spec, mouth: index }}
            crop={PART_CROP.mouth}
            label={t(`sidebar.center.avatar.mouth.${style}`)}
            selected={spec.mouth === index}
            disabled={disabled}
            onSelect={() => choose({ mouth: index })}
          />
        ))}
      </OptionGroup>
      <OptionGroup label={t('sidebar.center.avatar.accessory')}>
        {AVATAR_ACCESSORIES.map((style, index) => (
          <PartChoice
            key={style}
            spec={{ ...spec, accessory: index }}
            crop={PART_CROP.accessory}
            label={t(`sidebar.center.avatar.accessory.${style}`)}
            selected={spec.accessory === index}
            disabled={disabled}
            onSelect={() => choose({ accessory: index })}
          />
        ))}
      </OptionGroup>
    </div>
  )
}
