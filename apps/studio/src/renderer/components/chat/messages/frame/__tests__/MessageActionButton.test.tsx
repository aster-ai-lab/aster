import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { render, screen } from '@testing-library/react'
import type { ComponentProps, ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'

const actionChrome = readFileSync(
  join(process.cwd(), 'src/renderer/components/chat/messages/frame/message-action-menu.css'),
  'utf8'
)
const mainStyles = readFileSync(join(process.cwd(), 'src/renderer/assets/styles/index.css'), 'utf8')

vi.mock('@renderer/components/ActionIconButton', () => ({
  default: ({
    active,
    className,
    icon,
    type,
    ...props
  }: ComponentProps<'button'> & { active?: boolean; icon: ReactNode }) => (
    <button
      type={type ?? 'button'}
      className={className}
      data-active={active || undefined}
      data-testid="action-icon-button"
      {...props}>
      {icon}
    </button>
  )
}))

const { MessageActionButton } = await import('../MessageActionButton')

describe('MessageActionButton', () => {
  it('renders through the shared ActionIconButton implementation', () => {
    render(
      <MessageActionButton active className="message-action-button" aria-label="Copy">
        <span>Copy icon</span>
      </MessageActionButton>
    )

    const button = screen.getByTestId('action-icon-button')

    expect(button).toHaveAccessibleName('Copy')
    expect(button).toHaveAttribute('data-active', 'true')
    expect(button).toHaveClass(
      'message-action-button',
      'size-6',
      'rounded-[6px]',
      'enabled:hover:bg-[#f2f3f5]',
      'is-active'
    )
    expect(button).toHaveTextContent('Copy icon')
  })

  it('locks DESIGN.md metrics for the toolbar and its menu', () => {
    expect(actionChrome).toContain('.message-action-button')
    expect(actionChrome).toContain('width: 24px;')
    expect(actionChrome).toContain('height: 24px;')
    expect(actionChrome).toContain('border-radius: 6px;')
    expect(actionChrome).toContain('width: 14px;')
    expect(actionChrome).toContain('stroke-width: 1.7;')
    expect(actionChrome).toContain('background: #f2f3f5;')
    expect(actionChrome).toContain('height: 36px;')
    expect(actionChrome).toContain('font-size: 13px;')
    expect(actionChrome).toContain('background: #f7f7f7;')
    expect(actionChrome).toContain('box-shadow: 0 12px 32px rgba(0, 0, 0, 0.12);')
    expect(actionChrome).toContain('width: 16px;')
    expect(mainStyles).toContain('components/chat/messages/frame/message-action-menu.css')
  })
})
