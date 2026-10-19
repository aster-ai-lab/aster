import { describe, expect, it } from 'vitest'

import { mailBodyText, normalizeMailHref, quoteMailBody, sanitizeMailHtml } from '../mailDocument'

describe('mailDocument', () => {
  it('quotes plain text and keeps a tag in the original as text', () => {
    const quoted = quoteMailBody('crates.io', 'Hello\n<script>alert(1)</script>')
    expect(quoted).toContain('<blockquote>')
    expect(quoted).toContain('crates.io')
    expect(quoted).toContain('Hello')
    expect(quoted).not.toContain('<script>')
    expect(quoted).toContain('&lt;script&gt;')
  })

  it('drops a script and a javascript link, and keeps an https link', () => {
    const clean = sanitizeMailHtml(
      '<p>Hi</p><script>alert(1)</script><a href="javascript:alert(1)">bad</a><a href="https://example.com">ok</a>'
    )
    expect(clean).toContain('Hi')
    expect(clean).toContain('bad')
    expect(clean).toContain('https://example.com')
    expect(clean).toContain('rel="noopener noreferrer"')
    expect(clean).not.toContain('script')
    expect(clean).not.toContain('javascript')
  })

  it('keeps headings and drops an image data url', () => {
    const clean = sanitizeMailHtml('<h2>Title</h2><h3>Sub</h3><img src="data:text/plain,hi"><p>ok</p>')
    expect(clean).toContain('<h2>Title</h2>')
    expect(clean).toContain('<h3>Sub</h3>')
    expect(clean).toContain('ok')
    expect(clean).not.toContain('data:')
    expect(clean).not.toContain('<img')
  })

  it('keeps a signature block and drops attributes on it', () => {
    const clean = sanitizeMailHtml(
      '<mail-signature style="color:red" onclick="alert(1)"><p>Roy</p></mail-signature><div><p>x</p></div>'
    )
    expect(clean).toContain('<mail-signature>')
    expect(clean).toContain('Roy')
    expect(clean).toContain('<p>x</p>')
    expect(clean).not.toContain('<div')
    expect(clean).not.toContain('style')
    expect(clean).not.toContain('onclick')
  })

  it('accepts http and mailto links and rejects a script url', () => {
    expect(normalizeMailHref('example.com/path')).toBe('https://example.com/path')
    expect(normalizeMailHref('mailto:roy@mail.towork')).toBe('mailto:roy@mail.towork')
    expect(normalizeMailHref('javascript:alert(1)')).toBeNull()
    expect(normalizeMailHref('https://')).toBeNull()
    expect(mailBodyText('<p></p>')).toBe('')
    expect(mailBodyText('<p>later</p>')).toBe('later')
    expect(mailBodyText('<p>你好 <strong>正文</strong></p><script>alert(1)</script>')).toBe('你好 正文')
    expect(mailBodyText('<style>p{color:red}</style><p>later</p>')).toBe('later')
    expect(mailBodyText('<p>hi</p><script>alert(1)')).toBe('hi')
  })
})
