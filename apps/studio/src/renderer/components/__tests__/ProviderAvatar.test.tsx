// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import type { CSSProperties } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

// Resolve only `openai` to a recognizable stand-in icon; everything else is unknown.
vi.mock('@a3s-lab/ui/icons', () => {
  const BrandIcon = ({ style, variant }: { style?: CSSProperties; variant?: string }) => (
    <span data-testid="brand-icon" data-variant={variant} style={style} />
  )
  return {
    getProviderIconAssetMetrics: ({ kind, iconId }: { kind: string; iconId: string }) => ({
      canvasScale: kind === 'model' ? 24 / 16 : iconId === 'anthropic' ? 1 : 120 / 65,
      kind: iconId === 'anthropic' ? 'tile' : 'mark'
    }),
    resolveProviderIconRef: (id: string) =>
      id === 'openai' ? { kind: 'provider', key: id, meta: { id, colorPrimary: '#000' } } : undefined,
    useIcon: (ref: unknown) => (ref ? BrandIcon : undefined)
  }
})

import { ProviderAvatarPrimitive } from '../ProviderAvatar'

afterEach(() => {
  cleanup()
})

/** 真 ui 包的 avatar 族以 data-slot 标记（setup 桩用 data-testid）。 */
const getByDataSlot = (slot: string) => {
  const el = document.querySelector(`[data-slot="${slot}"]`)
  if (!el) throw new Error(`no element with data-slot="${slot}"`)
  return el as HTMLElement
}
const queryByDataSlot = (slot: string) => document.querySelector(`[data-slot="${slot}"]`) as HTMLElement | null

describe('ProviderAvatarPrimitive', () => {
  it('uses the built-in provider icon when no custom logo is stored', () => {
    render(<ProviderAvatarPrimitive providerId="openai" providerName="OpenAI" />)

    expect(screen.getByTestId('brand-icon')).toBeInTheDocument()
    expect(screen.queryByText('O')).not.toBeInTheDocument()
  })

  it('renders image logo avatars with object-cover cropping', () => {
    const logo = 'file:///tmp/wide-provider-logo.png'

    render(<ProviderAvatarPrimitive providerId="custom" providerName="Custom" logo={logo} />)

    expect(document.querySelector('img')).toHaveClass('object-cover')
    expect(document.querySelector('img')).toHaveAttribute('src', logo)
    // 真 Radix 语义：图片 load 后 fallback 不渲染（mock 时代两者并存）。
    expect(screen.queryByText('C')).not.toBeInTheDocument()
  })

  it('resolves an `icon:<id>` logo to the built-in brand icon', () => {
    render(<ProviderAvatarPrimitive providerId="custom" providerName="Custom" logo="icon:openai" />)

    expect(screen.getByTestId('brand-icon')).toBeInTheDocument()
    expect(screen.getByTestId('brand-icon')).not.toHaveAttribute('data-variant')
    // The raw reference must not leak through as an image source.
    expect(document.querySelector('img')).toBeNull()
  })

  it('fills the avatar container with built-in icons', () => {
    render(
      <ProviderAvatarPrimitive
        providerId="custom"
        providerName="Custom"
        logo="icon:openai"
        style={{ width: '52px', height: '52px' }}
      />
    )

    expect(screen.getByTestId('brand-icon')).toHaveStyle({ width: '100%', height: '100%' })
  })

  it('applies a caller-provided display style to built-in icons', () => {
    render(
      <ProviderAvatarPrimitive
        providerId="custom"
        providerName="Custom"
        logo="icon:openai"
        iconStyle={{ width: '71.42857142857143%', height: '71.42857142857143%', borderRadius: '5px' }}
      />
    )

    expect(screen.getByTestId('brand-icon')).toHaveStyle({
      width: '71.42857142857143%',
      height: '71.42857142857143%',
      borderRadius: '5px'
    })
  })

  it('separates compact artwork size from the declared avatar box', () => {
    render(
      <ProviderAvatarPrimitive
        providerId="openai"
        providerName="OpenAI"
        size={18}
        artworkSize={14}
        displayContext="sidebar"
      />
    )

    const normalizedArtworkSize = `${(120 / 65) * (14 / 18) * 100}%`
    expect(screen.getByTestId('brand-icon')).toHaveStyle({
      width: normalizedArtworkSize,
      height: normalizedArtworkSize
    })
    expect(getByDataSlot('avatar')).toHaveStyle({ width: '18px', height: '18px' })
    expect(getByDataSlot('avatar-fallback')).toHaveClass('bg-transparent')
    expect(getByDataSlot('avatar-fallback')).not.toHaveClass('bg-background')
  })

  it('keeps the provider-list surface behind built-in icons', () => {
    render(
      <ProviderAvatarPrimitive providerId="openai" providerName="OpenAI" size={32} displayContext="provider-list" />
    )

    expect(getByDataSlot('avatar-fallback')).toHaveClass('bg-background')
  })

  it('contains compact image logos on an outlined image surface', () => {
    const logo = 'file:///tmp/provider-logo.png'

    render(
      <ProviderAvatarPrimitive
        providerId="custom"
        providerName="Custom"
        logo={logo}
        size={18}
        artworkSize={14}
        displayContext="sidebar"
      />
    )

    expect(document.querySelector('img')).toHaveClass(
      'object-contain',
      'rounded-[3px]',
      'outline-black/10',
      'dark:outline-white/10'
    )
    expect(document.querySelector('img')).toHaveStyle({ width: '14px', height: '14px' })
    expect(getByDataSlot('avatar')).toHaveStyle({ width: '18px', height: '18px' })
  })

  it('falls back to the name initial when an `icon:<id>` reference is unknown', () => {
    render(<ProviderAvatarPrimitive providerId="custom" providerName="Zeta" logo="icon:does-not-exist" />)

    expect(screen.queryByTestId('brand-icon')).not.toBeInTheDocument()
    expect(screen.getByText('Z')).toBeInTheDocument()
    expect(document.querySelector('img')).toBeNull()
  })
})
