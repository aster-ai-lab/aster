import { render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { DefaultAvatar } from '../DefaultAvatar'

describe('DefaultAvatar', () => {
  afterEach(() => {
    delete document.documentElement.dataset.tenant
  })

  it('uses the aster transparent star rather than its desktop icon plate', () => {
    document.documentElement.dataset.tenant = 'aster'
    render(<DefaultAvatar label="AsterDream" />)
    expect(screen.getByRole('img', { name: 'AsterDream' }).querySelector('img')).toHaveAttribute(
      'src',
      '/tenant-logo.png'
    )
  })

  it('uses the app icon for every name', () => {
    document.documentElement.dataset.tenant = 'toyou'
    const { container } = render(<DefaultAvatar label="通友" />)
    const avatar = screen.getByRole('img', { name: '通友' })
    expect(avatar).toHaveAttribute('data-ui', 'default-avatar')
    expect(avatar.querySelector('img')).toHaveAttribute('src', '/tenant-icon.png')
    expect(container.querySelector('svg')).toBeNull()

    const again = render(<DefaultAvatar label="Ada" />)
    expect(again.container.querySelector('[data-ui="default-avatar"] img')).toHaveAttribute('src', '/tenant-icon.png')
  })

  it('still shows the app icon when the name is empty', () => {
    document.documentElement.dataset.tenant = 'toyou'
    const { container } = render(<DefaultAvatar />)
    expect(container.querySelector('[data-ui="default-avatar"] img')).toHaveAttribute('src', '/tenant-icon.png')
  })
})
