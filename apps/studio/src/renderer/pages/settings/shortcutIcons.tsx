import type { LucideIcon } from 'lucide-react'
import {
  AppWindow,
  ArrowLeft,
  ArrowRight,
  Camera,
  Copy,
  Cpu,
  Eraser,
  Eye,
  LayoutGrid,
  List,
  MessageSquare,
  MessageSquarePlus,
  MessagesSquare,
  MousePointer2,
  PanelLeft,
  PanelRight,
  Pencil,
  PencilLine,
  Printer,
  RotateCcw,
  Search,
  Settings,
  Shrink,
  Sparkles,
  SquareX,
  TextCursorInput,
  TextSearch,
  TextSelect,
  Trash2,
  X,
  Zap,
  ZoomIn,
  ZoomOut
} from 'lucide-react'

import type { ShortcutSettingsGroup } from '@renderer/hooks/command'
import type { CommandId } from '@shared/utils/command'

const SHORTCUT_ICON_CLASS = 'size-4 shrink-0'

export type ShortcutFilterGroup = 'all' | ShortcutSettingsGroup

const GROUP_ICONS: Record<ShortcutFilterGroup, LucideIcon> = {
  all: LayoutGrid,
  general: AppWindow,
  chat: MessageSquare,
  topic: MessagesSquare,
  assistant: Sparkles
}

const COMMAND_ICONS = {
  'app.fullscreen.exit': Shrink,
  'app.search': Search,
  'app.print': Printer,
  'app.sidebar.toggle': PanelLeft,
  'app.settings.open': Settings,
  'app.window.show': Eye,
  'app.window.close': X,
  'app.zoom.in': ZoomIn,
  'app.zoom.out': ZoomOut,
  'app.zoom.reset': RotateCcw,
  'chat.input.focus': TextCursorInput,
  'chat.context.toggle_new': Eraser,
  'chat.message.copy_last': Copy,
  'chat.message.edit_last_user': Pencil,
  'chat.message.search': TextSearch,
  'chat.model.select': Cpu,
  'quick_assistant.toggle': Zap,
  'selection.capture_text': TextSelect,
  'selection.toggle': MousePointer2,
  'screenshot.capture': Camera,
  'topic.clear_messages': Trash2,
  'topic.create': MessageSquarePlus,
  'topic.rename': PencilLine,
  'topic.sidebar.toggle': PanelRight,
  'workbench.palette': List,
  'tab.close': SquareX,
  'tab.next': ArrowRight,
  'tab.prev': ArrowLeft
} satisfies Record<CommandId, LucideIcon>

export function ShortcutGroupIcon({ group }: { group: ShortcutFilterGroup }) {
  const Icon = GROUP_ICONS[group]
  return <Icon className={SHORTCUT_ICON_CLASS} strokeWidth={1.7} aria-hidden data-shortcut-group={group} />
}

export function ShortcutCommandIcon({ command }: { command: CommandId }) {
  const Icon = COMMAND_ICONS[command]
  return <Icon className={SHORTCUT_ICON_CLASS} strokeWidth={1.7} aria-hidden data-shortcut-icon={command} />
}

export function ShortcutFilterLabel({ group, label }: { group: ShortcutFilterGroup; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <ShortcutGroupIcon group={group} />
      {label}
    </span>
  )
}
