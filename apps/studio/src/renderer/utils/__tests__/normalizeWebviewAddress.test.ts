import { expect, it } from 'vitest'

import { normalizeWebviewAddress, quarkSearchQuery, workspaceBrowserAddress } from '../normalizeWebviewAddress'

it.each([
  ['example.com:8080/path', 'https://example.com:8080/path'],
  ['localhost:3000', 'http://localhost:3000/'],
  ['127.0.0.1:9000/x', 'http://127.0.0.1:9000/x'],
  ['[::1]:3000', 'http://[::1]:3000/'],
  ['  example.com  ', 'https://example.com/'],
  ['file:///tmp/page%20one.html', 'file:///tmp/page%20one.html'],
  ['file:///C:/project/index.html', 'file:///C:/project/index.html'],
  ['javascript:alert(1)', null],
  ['custom:page', null],
  ['', null]
])('normalizes the same address for browser and MiniApp: %s', (input, expected) => {
  expect(normalizeWebviewAddress(input)).toBe(expected)
})

it.each([
  ['example.com', 'https://example.com/'],
  ['localhost:3000/new', 'http://localhost:3000/new'],
  ['https://example.com/docs', 'https://example.com/docs'],
  ['javascript:alert(1)', null],
  ['', null],
  ['/private/var/pages/note.html', 'file:///private/var/pages/note.html'],
  [
    '/private/var/pages/什么是智能体.html',
    'file:///private/var/pages/%E4%BB%80%E4%B9%88%E6%98%AF%E6%99%BA%E8%83%BD%E4%BD%93.html'
  ],
  ['C:\\workspace\\note.html', 'file:///C:/workspace/note.html']
])('keeps a web address out of Quark search: %s', (input, expected) => {
  expect(workspaceBrowserAddress(input)).toBe(expected)
})

it.each([
  ['天气预报', '天气预报'],
  ['  rust tutorial  ', 'rust tutorial'],
  ['example.com', null],
  ['javascript:alert(1)', null],
  ['', null]
])('uses a search query for Quark suggestions: %s', (input, expected) => {
  expect(quarkSearchQuery(input)).toBe(expected)
})

it('searches when a pasted local path is not an HTML page', () => {
  const address = workspaceBrowserAddress('/private/var/pages/note.md')
  expect(address?.startsWith('https://quark.sm.cn/s?')).toBe(true)
})

it.each(['天气预报', 'rust tutorial', 'site:example.com rust'])('opens other text on Quark search: %s', (input) => {
  const address = workspaceBrowserAddress(input)
  expect(address?.startsWith('https://quark.sm.cn/s?')).toBe(true)
  expect(new URL(address ?? '').searchParams.get('q')).toBe(input)
})
