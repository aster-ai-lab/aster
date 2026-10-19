import {
  AlarmClock,
  Calendar,
  CalendarDays,
  ClipboardList,
  ClipboardPlus,
  Clapperboard,
  Image,
  Languages,
  Lightbulb,
  MessageCircle,
  Moon,
  Newspaper,
  type LucideIcon
} from 'lucide-react'

import type { WorkScheduleDraft } from './workSchedule'

export type WorkScheduleTemplate = {
  id: string
  icon: LucideIcon
  schedule: Partial<WorkScheduleDraft>
}

/** 图中的 12 个自动化模版。名称和说明走文案，这里只放打开编辑器时的时间。 */
export const WORK_SCHEDULE_TEMPLATES: WorkScheduleTemplate[] = [
  { id: 'ai_news', icon: Newspaper, schedule: { mode: 'period', period: 'daily', time: '09:00' } },
  { id: 'words', icon: Languages, schedule: { mode: 'period', period: 'daily', time: '08:00' } },
  { id: 'bedtime', icon: Moon, schedule: { mode: 'period', period: 'daily', time: '20:30' } },
  { id: 'report', icon: ClipboardList, schedule: { mode: 'period', period: 'weekly', time: '18:00', weekdays: [5] } },
  { id: 'movie', icon: Clapperboard, schedule: { mode: 'period', period: 'weekly', time: '19:00', weekdays: [6] } },
  { id: 'history', icon: Calendar, schedule: { mode: 'period', period: 'daily', time: '08:30' } },
  { id: 'why', icon: Lightbulb, schedule: { mode: 'period', period: 'daily', time: '12:00' } },
  { id: 'family', icon: AlarmClock, schedule: { mode: 'period', period: 'weekly', time: '10:00', weekdays: [0] } },
  { id: 'checkup', icon: ClipboardPlus, schedule: { mode: 'once', onceAt: '2026-04-08T07:00' } },
  { id: 'interview', icon: MessageCircle, schedule: { mode: 'interval', hours: 2 } },
  { id: 'meeting', icon: CalendarDays, schedule: { mode: 'period', period: 'daily', time: '09:00' } },
  { id: 'wallpaper', icon: Image, schedule: { mode: 'period', period: 'daily', time: '07:30' } }
]
