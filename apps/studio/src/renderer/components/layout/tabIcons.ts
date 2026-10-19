import {
  Code,
  FileText,
  Folder,
  Globe,
  LayoutGrid,
  MessageCircle,
  MessagesSquare,
  NotepadText,
  Puzzle,
  Rocket,
  ScanSearch,
  Settings,
  Sparkles
} from 'lucide-react'

import { PiIcon, ProjectsIcon, WikiIcon } from '@renderer/components/app/sidebarIcons'
import type { Tab } from '@renderer/hooks/tab'

export type IconComponent = React.FC<{ size?: number; strokeWidth?: number; className?: string }>

// ─── Route → Icon mapping ─────────────────────────────────────────────────────

export const ROUTE_ICONS: Record<string, IconComponent> = {
  '/app/chat': MessageCircle,
  '/app/browser': Globe,
  '/app/agents': ProjectsIcon,
  '/app/messages': MessagesSquare,
  '/app/mini-app': LayoutGrid,
  '/app/launchpad': Rocket,
  '/app/knowledge': WikiIcon,
  '/app/file-preview': ScanSearch,
  '/app/files': Folder,
  '/app/code': Code,
  '/app/notes': NotepadText,
  '/app/capabilities': PiIcon,
  '/app/plugins': Puzzle,
  '/app/documents': FileText,
  '/app/release-notes': Sparkles,
  '/settings': Settings
}

export function getTabIcon(tab: Tab): IconComponent {
  if (tab.type === 'webview') return Globe
  const pathname = new URL(tab.url, 'https://www.cherry-ai.com/').pathname
  const segments = pathname.split('/').filter(Boolean)
  const key = segments[0] === 'app' && segments.length >= 2 ? '/app/' + segments[1] : '/' + (segments[0] || '')
  return ROUTE_ICONS[key] || MessageCircle
}
