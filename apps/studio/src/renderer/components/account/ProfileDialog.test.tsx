import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ProfileDialog } from './AccountDialogs'
import { parseAvatarSpec } from './composedAvatar'

describe('ProfileDialog avatar composer', () => {
  it('saves a composed avatar from the profile dialog', async () => {
    const user = userEvent.setup()
    const save = vi.fn(async (profile: { displayName: string; avatar: string }) => ({
      displayName: profile.displayName,
      phone: '',
      avatar: profile.avatar
    }))
    render(
      <ProfileDialog
        open
        account={{ displayName: '林', phone: '' }}
        onOpenChange={() => undefined}
        onSaved={() => undefined}
        onSessionDenied={() => undefined}
        save={save}
      />
    )

    await user.click(await screen.findByRole('button', { name: '生成头像' }))
    expect(screen.getByRole('radiogroup', { name: '背景' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '肤色' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '发型' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '发色' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '眼睛' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '嘴巴' })).toBeInTheDocument()
    expect(screen.getByRole('radiogroup', { name: '配饰' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '马尾' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '圆睁' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '浅笑' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: '耳机' })).toBeInTheDocument()
    expect(document.querySelector('[data-ui="account.avatar-preview"]')).toBeTruthy()
    const options = document.querySelector('[data-ui="account.avatar-options"]')
    expect(options).toHaveClass('overflow-y-auto')
    expect(screen.getByRole('button', { name: '保存' }).compareDocumentPosition(options!)).toBe(
      Node.DOCUMENT_POSITION_PRECEDING
    )

    const bob = screen.getByRole('radio', { name: '波波头' })
    const short = screen.getByRole('radio', { name: '短发' })
    const target = bob.getAttribute('aria-checked') === 'true' ? short : bob
    await user.click(target)

    const portrait = screen.getByRole('img', { name: '林' })
    const src = portrait.getAttribute('src') ?? ''
    expect(src.startsWith('data:image/svg+xml;base64,')).toBe(true)
    expect(src.length).toBeLessThan(400_000)
    expect(parseAvatarSpec(src)?.hair).toBe(target === bob ? 1 : 0)

    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(save).toHaveBeenCalledWith({ displayName: '林', avatar: src })
  })
})
