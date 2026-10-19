import { SearchInput } from '@a3s-lab/ui'
import { BookOpen, CodeXml, Home, Library, Star } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'
import { cn } from '@renderer/utils/style'

import {
  CLOUD_KNOWLEDGE_CATEGORIES,
  type CloudKnowledgeBundle,
  type CloudKnowledgeCategory,
  visibleCloudBundles
} from './cloudKnowledgeCatalog'
import type { CloudDownloadProgress } from './installCloudKnowledge'

const CATEGORY_ICONS = {
  home: Home,
  engineering: CodeXml,
  product: BookOpen,
  research: Library
} as const satisfies Record<CloudKnowledgeCategory, typeof Home>

const BANNERS: Record<string, string> = {
  'product-handbook': 'bg-[#f4f6f8]',
  'engineering-notes': 'bg-[#f3f5f4]',
  'research-briefs': 'bg-[#f6f5f3]'
}

export function CloudKnowledgeHome({
  query,
  onQueryChange,
  installed,
  installFailed,
  removeFailed,
  download,
  onInstall,
  onOpen,
  onRemove
}: {
  query: string
  onQueryChange: (value: string) => void
  installed: ReadonlySet<string>
  installFailed: boolean
  removeFailed: boolean
  download: (CloudDownloadProgress & { id: string }) | null
  onInstall: (bundle: CloudKnowledgeBundle) => void
  onOpen: (id: string) => void
  onRemove: (id: string) => void
}) {
  const { t } = useTranslation()
  const [category, setCategory] = useState<CloudKnowledgeCategory>('home')
  const searching = query.trim().length > 0
  const needle = query.trim().toLocaleLowerCase()
  const shelf = visibleCloudBundles(category)
  const entries = searching
    ? shelf.filter((bundle) => `${t(bundle.titleKey)} ${t(bundle.descriptionKey)}`.toLocaleLowerCase().includes(needle))
    : shelf

  return (
    <>
      <nav
        aria-label={t('knowledge.cloud.categories')}
        className={cn(
          'flex flex-col gap-0.5 bg-white px-3 pt-1 pb-4',
          pageSidebarWidthClassName,
          pageSidebarEdgeClassName
        )}>
        <SearchInput
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          onClear={() => onQueryChange('')}
          clearLabel={t('common.clear')}
          placeholder={t('knowledge.cloud.search')}
          size="sm"
          containerClassName="mb-2 h-[30px] w-full shrink-0 border-0 bg-[#f5f5f5] shadow-none"
          className="placeholder:text-[#8a8a8a]"
        />
        {CLOUD_KNOWLEDGE_CATEGORIES.map((id) => {
          const Icon = CATEGORY_ICONS[id]
          const selected = id === category
          return (
            <button
              key={id}
              type="button"
              aria-current={selected ? 'page' : undefined}
              onClick={() => setCategory(id)}
              className={cn(
                'flex h-9 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
                'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                selected
                  ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                  : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
              )}>
              <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
              <span className="truncate">{t(`knowledge.cloud.category.${id}`)}</span>
            </button>
          )
        })}
      </nav>
      <div className="min-h-0 min-w-0 flex-1 overflow-y-auto bg-white">
        <div className="px-6 pt-4 pb-6">
          {installFailed ? (
            <p role="status" className="m-0 mb-4 text-[13px] text-[#8a8a8a]">
              {t('knowledge.cloud.install_failed')}
            </p>
          ) : null}
          {removeFailed ? (
            <p role="status" className="m-0 mb-4 text-[13px] text-[#8a8a8a]">
              {t('knowledge.cloud.remove_failed')}
            </p>
          ) : null}
          {entries.length === 0 ? (
            <p className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">{t('knowledge.cloud.empty')}</p>
          ) : (
            <div className="grid grid-cols-2 gap-4 min-[1080px]:grid-cols-4">
              {entries.map((bundle) => {
                const title = t(bundle.titleKey)
                const done = installed.has(bundle.id)
                const current = download?.id === bundle.id ? download : null
                const percent = current && current.total > 0 ? Math.round((current.completed / current.total) * 100) : 0
                return (
                  <article
                    key={bundle.id}
                    className="group relative h-[236px] overflow-hidden rounded-[14px] border border-[#ececec] shadow-[0_8px_22px_rgba(20,40,80,0.04)]">
                    <div
                      data-knowledge-cover="true"
                      className={cn('absolute inset-0', BANNERS[bundle.id] ?? 'bg-[#f5f5f5]')}
                    />
                    <div className="relative flex h-full flex-col justify-between p-4">
                      <BookOpen className="size-5 text-[#171717]" strokeWidth={1.7} aria-hidden />
                      <div>
                        <h3 className="m-0 truncate text-[14px] font-medium text-[#1a1a1a]">{title}</h3>
                        <p className="m-0 mt-1.5 line-clamp-2 text-[12px] leading-5 text-[#8b8b8b]">
                          {t(bundle.descriptionKey)}
                        </p>
                        <div className="mt-3 flex items-center justify-between gap-2">
                          <span className="flex items-center gap-1 text-[13px] text-[#171717]">
                            <Star className="size-3.5 fill-[#171717] text-[#171717]" strokeWidth={1.6} aria-hidden />
                            {bundle.rating.toFixed(1)}
                          </span>
                          {current ? (
                            <div
                              role="progressbar"
                              aria-valuemin={0}
                              aria-valuemax={100}
                              aria-valuenow={percent}
                              aria-valuetext={current.name || t('knowledge.cloud.preparing')}
                              aria-label={`${t('knowledge.cloud.downloading')} ${title}`}
                              className="relative h-[30px] w-[108px] overflow-hidden rounded-[8px] border border-[#e5e5e5] bg-white">
                              <div
                                className="absolute inset-y-0 left-0 bg-[#e4e4e4]"
                                style={{ width: `${percent}%` }}
                              />
                              <span className="relative z-10 flex h-full items-center justify-center text-[12px] text-[#171717]">
                                {percent === 0 ? t('knowledge.cloud.preparing') : `${percent}%`}
                              </span>
                            </div>
                          ) : (
                            <span className="pointer-events-none flex items-center gap-2 opacity-0 transition-opacity group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100">
                              {done ? (
                                <>
                                  <button
                                    type="button"
                                    aria-label={`${t('knowledge.cloud.preview')} ${title}`}
                                    onClick={() => onOpen(bundle.id)}
                                    className="h-[30px] shrink-0 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                                    {t('knowledge.cloud.preview')}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`${t('knowledge.cloud.remove')} ${title}`}
                                    onClick={() => onRemove(bundle.id)}
                                    className="h-[30px] shrink-0 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                                    {t('knowledge.cloud.remove')}
                                  </button>
                                </>
                              ) : (
                                <button
                                  type="button"
                                  aria-label={`${t('knowledge.cloud.download')} ${title}`}
                                  onClick={() => {
                                    void onInstall(bundle)
                                  }}
                                  className="h-[30px] shrink-0 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                                  {t('knowledge.cloud.download')}
                                </button>
                              )}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  </article>
                )
              })}
            </div>
          )}
        </div>
      </div>
    </>
  )
}
