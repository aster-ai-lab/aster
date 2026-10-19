import { describe, expect, it } from 'vitest'

import { previewKind, previewMime, type DocumentPreviewKind } from '../documentPreview'
import { monacoLanguage } from '../monacoLanguage'

const cases: Array<[string, DocumentPreviewKind]> = [
  ['隐私政策.docx', 'document'],
  ['Notes.DOC', 'document'],
  ['规格.docm', 'document'],
  ['信头.dotx', 'document'],
  ['信.odt', 'document'],
  ['备忘.rtf', 'document'],
  ['README.md', 'markdown'],
  ['README.markdown', 'markdown'],
  ['表.csv', 'spreadsheet'],
  ['表.tsv', 'spreadsheet'],
  ['表.xlsx', 'spreadsheet'],
  ['表.xls', 'spreadsheet'],
  ['表.ods', 'spreadsheet'],
  ['幻灯.pptx', 'presentation'],
  ['幻灯.ppt', 'presentation'],
  ['幻灯.odp', 'presentation'],
  ['手册.pdf', 'pdf'],
  ['封面.PNG', 'image'],
  ['封面.jpg', 'image'],
  ['封面.jpeg', 'image'],
  ['封面.gif', 'image'],
  ['封面.webp', 'image'],
  ['封面.svg', 'image'],
  ['语音.mp3', 'audio'],
  ['语音.wav', 'audio'],
  ['语音.ogg', 'audio'],
  ['剪辑.mp4', 'video'],
  ['剪辑.webm', 'video'],
  ['剪辑.mov', 'video'],
  ['纪要.txt', 'text'],
  ['配置.json', 'text'],
  ['页面.html', 'text'],
  ['页面.htm', 'text'],
  ['App.ts', 'text'],
  ['App.tsx', 'text'],
  ['app.js', 'text'],
  ['App.jsx', 'text'],
  ['theme.css', 'text'],
  ['feed.xml', 'text'],
  ['config.yaml', 'text'],
  ['config.yml', 'text'],
  ['main.rs', 'text'],
  ['main.py', 'text'],
  ['main.go', 'text'],
  ['001_init.sql', 'text'],
  ['Cargo.toml', 'text'],
  ['Cargo.lock', 'text'],
  ['图纸.dwg', 'unsupported'],
  ['书.epub', 'unsupported'],
  ['表.numbers', 'unsupported'],
  ['导图.xmind', 'unsupported'],
  ['README', 'text'],
  ['Dockerfile', 'text'],
  ['archive.tar.gz', 'unsupported'],
  ['.gitignore', 'text'],
  ['notes.', 'unsupported'],
  ['bundle.zip', 'unsupported'],
  ['main.kt', 'text'],
  ['config.toml', 'text']
]

describe('previewKind', () => {
  it.each(cases)('%s opens as %s', (fileName, kind) => {
    expect(previewKind(fileName)).toBe(kind)
  })

  it('keeps html and txt on the text surface and csv on the spreadsheet', () => {
    expect(previewKind('notes.txt')).toBe('text')
    expect(previewKind('page.html')).toBe('text')
    expect(previewKind('sheet.csv')).toBe('spreadsheet')
    expect(previewKind('readme.md')).toBe('markdown')
  })

  it('picks a Monaco language and leaves unknown text as plaintext', () => {
    expect(monacoLanguage('main.rs')).toBe('rust')
    expect(monacoLanguage('widget.cpp')).toBe('c')
    expect(monacoLanguage('App.tsx')).toBe('typescript')
    expect(monacoLanguage('page.html')).toBe('html')
    expect(monacoLanguage('config.yaml')).toBe('yaml')
    expect(monacoLanguage('api.proto')).toBe('proto')
    expect(monacoLanguage('Token.sol')).toBe('sol')
    expect(monacoLanguage('Dockerfile')).toBe('dockerfile')
    expect(monacoLanguage('Gemfile')).toBe('ruby')
    expect(monacoLanguage('说明.acl')).toBe('plaintext')
    expect(monacoLanguage('README')).toBe('plaintext')
    expect(monacoLanguage('.gitignore')).toBe('plaintext')
  })

  it('maps image, audio, and video names to a playable mime', () => {
    expect(previewMime('封面.png')).toBe('image/png')
    expect(previewMime('封面.jpeg')).toBe('image/jpeg')
    expect(previewMime('封面.svg')).toBe('image/svg+xml')
    expect(previewMime('语音.mp3')).toBe('audio/mpeg')
    expect(previewMime('剪辑.mp4')).toBe('video/mp4')
    expect(previewMime('纪要.txt')).toBeNull()
  })
})
