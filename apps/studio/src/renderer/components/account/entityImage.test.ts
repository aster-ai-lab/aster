import { describe, expect, it } from 'vitest'

import { logoInitials, textLogo } from './entityImage'

function decode(dataUrl: string): string {
  const binary = atob(dataUrl.split(',')[1])
  return new TextDecoder().decode(Uint8Array.from(binary, (char) => char.charCodeAt(0)))
}

describe('team text logo', () => {
  it('takes two CJK characters or two Latin initials', () => {
    expect(logoInitials('  通友科技 ')).toBe('通友')
    expect(logoInitials('acme labs')).toBe('AL')
    expect(logoInitials('kense')).toBe('KE')
    expect(logoInitials('   ')).toBe('')
  })

  it('renders an escaped SVG data URL that the OS accepts and keeps one color per name', () => {
    const logo = textLogo('通友科技')
    expect(logo.startsWith('data:image/svg+xml;base64,')).toBe(true)
    expect(decode(logo)).toContain('>通友</text>')
    expect(textLogo('通友科技')).toBe(logo)
    expect(decode(textLogo('<a&b>'))).toContain('&#60;A</text>')
    expect(textLogo('')).toBe('')
  })
})
