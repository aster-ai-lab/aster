import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const rendererRoot = resolve(__dirname, '../../..')

/**
 * An agent avatar string holds an emoji, a bundled image, an inline or remote image,
 * or an authenticated catalog icon path. Only `components/Avatar` may decide which;
 * every other renderer passes the string to `AgentAvatar` or `agentAvatarElement`.
 * Each rule names the files it still tolerates. These lists may only shrink.
 */
const RULES: { name: string; pattern: RegExp; tolerated: string[] }[] = [
  {
    name: 'a local guess whether an avatar string is an image or an emoji',
    pattern: /(?:function|const)\s+\w*(?:isImageAvatar|isEmojiAvatar|isAvatarImage|avatarIsImage)\b/,
    // Member and person portraits, not agent avatars.
    tolerated: ['components/Sidebar/primitives.tsx', 'pages/messages/messageAvatars.tsx']
  },
  {
    name: 'an agent or assistant avatar drawn by EmojiIcon, which prints any string as text',
    pattern: /<EmojiIcon\b[^>]*?\bemoji=\{[^}]*(?:getAgentAvatar\w*|\.emoji\b|\.avatar\b|\bavatar\b|\bemoji\b)[^}]*\}/s,
    tolerated: ['components/Sidebar/primitives.tsx']
  },
  {
    name: 'an agent or assistant avatar placed directly in an image source',
    pattern: /<(?:img|AvatarImage)\b[^>]*?\bsrc=\{[^}]*(?:getAgentAvatar\w*|\.emoji\b|\bemoji\b)[^}]*\}/s,
    tolerated: []
  },
  {
    name: 'an agent or assistant avatar printed as text',
    pattern: />\s*\{\s*[\w?.]*(?:assistant|agent|preset|row|item)\??\.(?:emoji|avatar)\s*\}\s*</,
    tolerated: []
  },
  {
    name: 'an avatar string printed by CSS or textContent',
    pattern: /content:\s*attr\(\s*data-[\w-]*avatar|\.textContent\s*=\s*\w*avatar\b/i,
    tolerated: []
  }
]

function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    if (entry === '__tests__' || entry === 'node_modules') continue
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) sourceFiles(path, out)
    else if (/\.(?:tsx?|css)$/.test(entry)) out.push(path)
  }
  return out
}

describe('agent avatar boundary', () => {
  const files = sourceFiles(rendererRoot)
    .map((path) => relative(rendererRoot, path))
    .filter((path) => !path.startsWith('components/Avatar/'))

  for (const rule of RULES) {
    it(`has no ${rule.name} outside the avatar module`, () => {
      const hits = files.filter((path) => rule.pattern.test(readFileSync(join(rendererRoot, path), 'utf8')))
      expect(hits.filter((path) => !rule.tolerated.includes(path))).toEqual([])
      expect(rule.tolerated.filter((path) => !hits.includes(path))).toEqual([])
    })
  }

  it('catches each shape it forbids', () => {
    const samples = [
      'export function isImageAvatar(value: string) {',
      '<EmojiIcon emoji={getAgentAvatarFromConfiguration(agent.configuration)} size={24} />',
      '<EmojiIcon emoji={assistant.emoji} size={24} />',
      '<img src={row.emoji} alt="" />',
      '<span>{assistant.emoji}</span>',
      'mark.textContent = avatar',
      'content: attr(data-agent-avatar);'
    ]
    for (const sample of samples) {
      expect(RULES.some((rule) => rule.pattern.test(sample))).toBe(true)
    }
    expect(RULES.some((rule) => rule.pattern.test('<AgentAvatar avatar={assistant.emoji} size={24} />'))).toBe(false)
  })
})
