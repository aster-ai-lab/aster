export const WRITING_CATEGORIES = ['home', 'notice', 'summary', 'request', 'minutes'] as const

export type WritingCategory = (typeof WRITING_CATEGORIES)[number]

export type WritingShelf = Exclude<WritingCategory, 'home'>

export interface WritingEntry {
  id: string
  titleKey: string
  descriptionKey: string
  promptKey: string
  category: WritingShelf
  featured: boolean
}

export const WRITING_ENTRIES: readonly WritingEntry[] = [
  {
    id: 'notice-meeting',
    titleKey: 'workbench.writing.card.notice_meeting.title',
    descriptionKey: 'workbench.writing.card.notice_meeting.description',
    promptKey: 'workbench.feature.write.blessing',
    category: 'notice',
    featured: true
  },
  {
    id: 'summary-quarter',
    titleKey: 'workbench.writing.card.summary_quarter.title',
    descriptionKey: 'workbench.writing.card.summary_quarter.description',
    promptKey: 'workbench.feature.write.career',
    category: 'summary',
    featured: true
  },
  {
    id: 'request-funding',
    titleKey: 'workbench.writing.card.request_funding.title',
    descriptionKey: 'workbench.writing.card.request_funding.description',
    promptKey: 'workbench.feature.write.plan',
    category: 'request',
    featured: true
  },
  {
    id: 'minutes-director',
    titleKey: 'workbench.writing.card.minutes_director.title',
    descriptionKey: 'workbench.writing.card.minutes_director.description',
    promptKey: 'workbench.feature.write.review',
    category: 'minutes',
    featured: true
  },
  {
    id: 'notice-brief',
    titleKey: 'workbench.writing.card.notice_brief.title',
    descriptionKey: 'workbench.writing.card.notice_brief.description',
    promptKey: 'workbench.writing.card.notice_brief.prompt',
    category: 'notice',
    featured: false
  },
  {
    id: 'summary-year',
    titleKey: 'workbench.writing.card.summary_year.title',
    descriptionKey: 'workbench.writing.card.summary_year.description',
    promptKey: 'workbench.writing.card.summary_year.prompt',
    category: 'summary',
    featured: false
  },
  {
    id: 'request-matter',
    titleKey: 'workbench.writing.card.request_matter.title',
    descriptionKey: 'workbench.writing.card.request_matter.description',
    promptKey: 'workbench.writing.card.request_matter.prompt',
    category: 'request',
    featured: false
  },
  {
    id: 'minutes-topic',
    titleKey: 'workbench.writing.card.minutes_topic.title',
    descriptionKey: 'workbench.writing.card.minutes_topic.description',
    promptKey: 'workbench.writing.card.minutes_topic.prompt',
    category: 'minutes',
    featured: false
  }
]

export function visibleWritingEntries(category: WritingCategory, expanded: boolean): readonly WritingEntry[] {
  if (category === 'home') {
    return expanded ? WRITING_ENTRIES : WRITING_ENTRIES.filter((entry) => entry.featured)
  }
  return WRITING_ENTRIES.filter((entry) => entry.category === category)
}

export function hasMoreWriting(category: WritingCategory, expanded: boolean): boolean {
  return category === 'home' && !expanded && WRITING_ENTRIES.some((entry) => !entry.featured)
}
