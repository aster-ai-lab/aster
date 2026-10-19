import { checkEntityImageSize, prepareEntityImageBytes } from '@renderer/utils/image'

const LOGO_COLORS = ['#3370FF', '#14A9A0', '#7A5AF8', '#F5822A', '#E5484D', '#2F9E44', '#D4A017', '#0E7490']

const CJK = /\p{Script=Han}|\p{Script=Hiragana}|\p{Script=Katakana}|\p{Script=Hangul}/u

function graphemes(text: string): string[] {
  if (typeof Intl !== 'undefined' && 'Segmenter' in Intl) {
    return [...new Intl.Segmenter('zh', { granularity: 'grapheme' }).segment(text)].map((part) => part.segment)
  }
  return [...text]
}

/** 团队文字 Logo 的字：中日韩取前两字，拉丁文取前两个词的首字母。 */
export function logoInitials(name: string): string {
  const trimmed = name.trim()
  if (!trimmed) return ''
  const chars = graphemes(trimmed)
  if (CJK.test(chars[0])) return chars.slice(0, 2).join('')
  const words = trimmed.split(/[\s\-_.]+/).filter(Boolean)
  const initials = words.length > 1 ? words.slice(0, 2).map((word) => graphemes(word)[0]) : chars.slice(0, 2)
  return initials.join('').toUpperCase()
}

function hash(text: string): number {
  let value = 0
  for (const char of text) value = (value * 31 + char.codePointAt(0)!) >>> 0
  return value
}

function escapeXml(text: string): string {
  return text.replace(/[<>&'"]/g, (char) => `&#${char.charCodeAt(0)};`)
}

function base64Utf8(text: string): string {
  let binary = ''
  for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte)
  return btoa(binary)
}

/** 根据团队名称生成的方形文字 Logo（SVG data URL）。同名总是同色。 */
export function textLogo(name: string): string {
  const initials = logoInitials(name)
  if (!initials) return ''
  const color = LOGO_COLORS[hash(name.trim()) % LOGO_COLORS.length]
  const size = graphemes(initials).length > 1 ? 46 : 60
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">` +
    `<rect width="128" height="128" rx="28" fill="${color}"/>` +
    `<text x="64" y="64" dy=".35em" text-anchor="middle" fill="#fff" font-size="${size}" font-weight="600" ` +
    `font-family="-apple-system,BlinkMacSystemFont,'PingFang SC','Microsoft YaHei',sans-serif">${escapeXml(initials)}</text>` +
    `</svg>`
  return `data:image/svg+xml;base64,${base64Utf8(svg)}`
}

/** 头像 / Logo 上传：校验大小后裁成 128×128 方图，返回可直接提交的 data URL。 */
export async function imageFileToDataUrl(file: File): Promise<string> {
  const tooLarge = checkEntityImageSize(file)
  if (tooLarge) throw new Error(tooLarge)
  const bytes = await prepareEntityImageBytes(file)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return `data:image/webp;base64,${btoa(binary)}`
}
