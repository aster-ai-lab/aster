import type { LucideProps } from 'lucide-react'
import {
  Building2,
  Calendar,
  Code,
  FileText,
  Folder,
  LayoutGrid,
  Mail,
  MessageSquare,
  MessagesSquare,
  Network,
  NotepadText,
  Puzzle
} from 'lucide-react'
import type { ReactNode } from 'react'

import type { SidebarAppId } from '@renderer/utils/sidebar'

type SidebarIconComponent = (props: LucideProps) => ReactNode

const CodeMateIcon: SidebarIconComponent = (props) => <Code {...props} viewBox="-2 -2 28 28" />

/** Two rounded squares, the rear one up and to the right. Outline until the rail selects it. */
export const ProjectsIcon: SidebarIconComponent = ({ size = 24, strokeWidth = 1.6, ...props }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" {...props}>
    <rect
      x="8"
      y="3.25"
      width="12.5"
      height="12.5"
      rx="2.75"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
    />
    <rect
      x="3.5"
      y="8.25"
      width="12.5"
      height="12.5"
      rx="2.75"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
    />
  </svg>
)

/** Open book. Outline until the rail selects it. */
export const WikiIcon: SidebarIconComponent = ({ size = 24, strokeWidth = 1.6, ...props }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" {...props}>
    <path
      d="M4 5.5A3.5 3.5 0 0 1 7.5 2H12v18H7.5A3.5 3.5 0 0 0 4 23Z"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
    />
    <path
      d="M20 5.5A3.5 3.5 0 0 0 16.5 2H12v18h4.5A3.5 3.5 0 0 1 20 23Z"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
    />
  </svg>
)

/** Four-point star. Outline until the rail selects it. */
export const PiIcon: SidebarIconComponent = ({ size = 24, strokeWidth = 1.6, ...props }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" {...props}>
    <path
      d="M12 2.6 14.1 8.7 20.4 9.3 15.5 13.6 17 19.8 12 16.5 7 19.8 8.5 13.6 3.6 9.3 9.9 8.7Z"
      fill="none"
      stroke="currentColor"
      strokeLinejoin="round"
      strokeWidth={strokeWidth}
    />
  </svg>
)

/**
 * Icon component for each built-in sidebar app. Keyed by the `SidebarAppId` union so the
 * compiler enforces full coverage — adding a new sidebar app id without an icon
 * here is a type error. Kept in the component layer because the values are React
 * components; the navigation data and logic live in `@renderer/utils/sidebar`.
 */
export const SIDEBAR_ICON_COMPONENTS = {
  assistants: MessageSquare,
  agents: ProjectsIcon,
  messages: MessagesSquare,
  mail: Mail,
  schedule: Calendar,
  org: Network,
  platform: Building2,
  mini_app: LayoutGrid,
  knowledge: WikiIcon,
  files: Folder,
  code_tools: CodeMateIcon,
  notes: NotepadText,
  capabilities: PiIcon,
  plugins: Puzzle,
  documents: FileText
} satisfies Record<SidebarAppId, SidebarIconComponent>
