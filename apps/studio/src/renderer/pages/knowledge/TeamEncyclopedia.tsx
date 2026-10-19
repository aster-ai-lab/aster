import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { FileTreeSearchField } from '@renderer/components/FileTreeSearchField'
import { DriveApiError } from '@renderer/drive/driveTransport'
import {
  detachEncyclopedia,
  filesDriveHref,
  listEncyclopedia,
  postEncyclopedia,
  type EncyclopediaArticle
} from '@renderer/drive/encyclopediaClient'
import { useTabs } from '@renderer/hooks/tab'
import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'

interface TeamEncyclopediaProps {
  centerId: string
  corner?: ReactNode
}

/**
 * Compiled articles for this organization. The documents tab is the drive those articles come from.
 * Switching centerId drops the previous list before the next response arrives.
 */
export function TeamEncyclopedia({ centerId, corner }: TeamEncyclopediaProps) {
  const { t } = useTranslation()
  const [loadedFor, setLoadedFor] = useState(centerId)
  const [articles, setArticles] = useState<EncyclopediaArticle[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [editing, setEditing] = useState<EncyclopediaArticle | null>(null)

  if (loadedFor !== centerId) {
    setLoadedFor(centerId)
    setArticles([])
    setError(null)
    setLoading(true)
    setQuery('')
    setSelectedId(null)
    setEditing(null)
  }

  useEffect(() => {
    const controller = new AbortController()
    let active = true
    setLoading(true)
    void listEncyclopedia(controller.signal)
      .then((rows) => {
        if (!active) return
        setArticles(rows)
        setError(null)
      })
      .catch((reason: unknown) => {
        if (!active || controller.signal.aborted) return
        setError(encyclopediaErrorText(reason, t))
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
      controller.abort()
    }
  }, [centerId, t])

  const reload = () => {
    const controller = new AbortController()
    void listEncyclopedia(controller.signal).then((rows) => {
      if (loadedFor === centerId) setArticles(rows)
    })
  }

  const act = (articleId: string, action: 'publish' | 'return' | 'retry') => {
    void postEncyclopedia(articleId, action).then(() => reload())
  }

  const detach = (articleId: string, title: string, body: string) => {
    void detachEncyclopedia(articleId, title, body)
      .then(() => {
        setEditing(null)
        reload()
      })
      .catch((reason: unknown) => {
        setError(encyclopediaErrorText(reason, t))
      })
  }

  const needle = query.trim().toLowerCase()
  const matches = (article: EncyclopediaArticle) => !needle || article.title.toLowerCase().includes(needle)
  const pending = articles.filter((article) => article.status === 'pending' || article.status === 'draft')
  const published = articles.filter((article) => article.status === 'published')
  const shownPending = pending.filter(matches)
  const shownPublished = published.filter(matches)
  const selected = articles.find((article) => article.id === selectedId) ?? pending[0] ?? published[0] ?? null

  return (
    <section
      data-ui="knowledge.encyclopedia"
      data-center={centerId}
      aria-label={t('knowledge.hub.wiki')}
      className="flex min-h-0 flex-1 flex-col bg-white">
      <header className="shrink-0 bg-white">
        <div className="flex flex-col gap-4 px-6 pt-4 pb-4">
          <div className="flex items-center justify-between gap-4">
            <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
              {t('knowledge.notes.title')}
            </h1>
            <div className="flex shrink-0 items-center gap-2">{corner}</div>
          </div>
          <p className="m-0 text-[15px] text-muted-foreground">{t('knowledge.encyclopedia.subtitle')}</p>
        </div>
      </header>
      {error ? <p className="m-0 px-6 pb-2 text-[13px] text-[#b42318]">{error}</p> : null}
      {loading && articles.length === 0 && !error ? (
        <p className="m-0 px-6 py-16 text-center text-[13px] text-[#8a8a8a]">{t('knowledge.encyclopedia.loading')}</p>
      ) : null}
      {!loading && articles.length === 0 && !error ? (
        <div className="px-6 py-16 text-center">
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('knowledge.encyclopedia.empty')}</p>
          <p className="m-0 mt-1 text-[13px] text-[#8a8a8a]">{t('knowledge.encyclopedia.emptyDescription')}</p>
        </div>
      ) : null}
      {articles.length > 0 ? (
        <div className="flex min-h-0 flex-1">
          <aside className={`${pageSidebarWidthClassName} ${pageSidebarEdgeClassName} flex min-h-0 flex-col`}>
            <div className="px-3 pt-1 pb-2">
              <FileTreeSearchField
                value={query}
                onChange={setQuery}
                placeholder={t('knowledge.encyclopedia.search')}
                searchLabel={t('knowledge.encyclopedia.search')}
                refreshLabel={t('drive.refresh')}
                onRefresh={reload}
              />
            </div>
            <div className="min-h-0 flex-1 overflow-auto px-2 pb-3">
              {shownPending.length === 0 && shownPublished.length === 0 ? (
                <p className="px-2.5 py-6 text-[13px] text-[#8a8a8a]">{t('knowledge.encyclopedia.no_match')}</p>
              ) : null}
              {shownPending.length > 0 ? (
                <div className="pb-3">
                  <h2 className="m-0 px-2.5 py-2 text-[12px] font-medium text-[#8a8a8a]">
                    {t('knowledge.encyclopedia.queue')}
                  </h2>
                  <ul className="m-0 list-none p-0">
                    {shownPending.map((article) => (
                      <ArticleChoice
                        key={article.id}
                        article={article}
                        selected={selected?.id === article.id}
                        onSelect={(id) => {
                          setEditing(null)
                          setSelectedId(id)
                        }}
                      />
                    ))}
                  </ul>
                </div>
              ) : null}
              {shownPublished.length > 0 ? (
                <div>
                  <h2 className="m-0 px-2.5 py-2 text-[12px] font-medium text-[#8a8a8a]">
                    {t('knowledge.encyclopedia.published')}
                  </h2>
                  <ul className="m-0 list-none p-0">
                    {shownPublished.map((article) => (
                      <ArticleChoice
                        key={article.id}
                        article={article}
                        selected={selected?.id === article.id}
                        onSelect={(id) => {
                          setEditing(null)
                          setSelectedId(id)
                        }}
                      />
                    ))}
                  </ul>
                </div>
              ) : null}
            </div>
          </aside>
          <div className="min-w-0 flex-1 overflow-auto px-8 py-6">
            {selected && editing?.id === selected.id ? (
              <DetachForm
                article={editing}
                onCancel={() => setEditing(null)}
                onSave={(title, body) => detach(editing.id, title, body)}
              />
            ) : null}
            {selected && editing?.id !== selected.id ? (
              <ArticleReader article={selected} onAct={act} onDetach={setEditing} />
            ) : null}
          </div>
        </div>
      ) : null}
    </section>
  )
}

function encyclopediaErrorText(reason: unknown, t: (key: string) => string): string {
  if (reason instanceof DriveApiError && (reason.code === 'drive.unauthenticated' || reason.status === 401)) {
    return t('knowledge.encyclopedia.signedOut')
  }
  return reason instanceof Error && reason.message ? reason.message : t('knowledge.encyclopedia.loadFailed')
}

function ArticleChoice({
  article,
  selected,
  onSelect
}: {
  article: EncyclopediaArticle
  selected: boolean
  onSelect: (id: string) => void
}) {
  return (
    <li>
      <button
        type="button"
        data-ui="encyclopedia.article"
        data-status={article.status}
        aria-current={selected ? 'page' : undefined}
        className={`flex h-9 w-full items-center rounded-[8px] px-2.5 text-left text-[13px] ${selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'}`}
        onClick={() => onSelect(article.id)}>
        <span className="truncate">{article.title}</span>
      </button>
    </li>
  )
}

function ArticleReader({
  article,
  onAct,
  onDetach
}: {
  article: EncyclopediaArticle
  onAct: (id: string, action: 'publish' | 'return' | 'retry') => void
  onDetach: (article: EncyclopediaArticle) => void
}) {
  const { t } = useTranslation()
  const { openTab } = useTabs()
  const review = article.status === 'pending' || article.status === 'draft'
  const status =
    article.status === 'published'
      ? t('knowledge.encyclopedia.published')
      : article.status === 'draft'
        ? t('knowledge.encyclopedia.pending')
        : t('knowledge.encyclopedia.queue')
  return (
    <article>
      <div className="flex items-start justify-between gap-4">
        <h2 className="m-0 text-[20px] leading-tight font-medium text-[#171717]">{article.title}</h2>
        <span className="inline-flex h-5 shrink-0 items-center rounded-[4px] bg-[#f2f3f5] px-1.5 text-[11px] font-medium text-[#171717]">
          {status}
        </span>
      </div>
      <p className="m-0 mt-2 text-[12px] text-[#8a8a8a]">
        {article.audienceKind === 'org'
          ? t('knowledge.encyclopedia.audience_org')
          : t('knowledge.encyclopedia.audience_dept')}
      </p>
      <p className="m-0 mt-6 text-[15px] leading-7 whitespace-pre-wrap text-[#3f3f3f]">{article.body}</p>
      <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-2">
        {article.writerMemberId ? (
          <span data-ui="encyclopedia.writer" className="text-[12px] text-[#8a8a8a]">
            {t('knowledge.encyclopedia.writer')} {article.writerMemberId}
          </span>
        ) : null}
        {article.sessionLabel ? (
          <span data-ui="encyclopedia.session" className="text-[12px] text-[#8a8a8a]">
            {t('knowledge.encyclopedia.session')} {article.sessionLabel}
          </span>
        ) : null}
        {article.citations.map((citation) => (
          <a
            key={citation.fileId}
            data-ui="encyclopedia.citation"
            href={filesDriveHref(citation.fileId)}
            onClick={(event) => {
              event.preventDefault()
              openTab(filesDriveHref(citation.fileId))
            }}
            className="text-[13px] text-[#171717] underline">
            {t('knowledge.encyclopedia.citation')} {citation.fileId}
          </a>
        ))}
      </div>
      <div className="mt-5 flex flex-wrap items-center gap-2">
        {article.canDetach ? (
          <button
            type="button"
            data-ui="encyclopedia.detach"
            className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa]"
            onClick={() => onDetach(article)}>
            {t('knowledge.encyclopedia.edit')}
          </button>
        ) : null}
        {review ? (
          <>
            <button
              type="button"
              className="h-8 rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white"
              onClick={() => onAct(article.id, 'publish')}>
              {t('knowledge.encyclopedia.accept')}
            </button>
            <button
              type="button"
              className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa]"
              onClick={() => onAct(article.id, 'return')}>
              {t('knowledge.encyclopedia.return')}
            </button>
            <button
              type="button"
              className="h-[30px] px-2 text-[13px] text-[#8a8a8a] hover:text-[#171717]"
              onClick={() => onAct(article.id, 'retry')}>
              {t('knowledge.encyclopedia.retry')}
            </button>
          </>
        ) : null}
      </div>
    </article>
  )
}

function DetachForm({
  article,
  onCancel,
  onSave
}: {
  article: EncyclopediaArticle
  onCancel: () => void
  onSave: (title: string, body: string) => void
}) {
  const { t } = useTranslation()
  const [title, setTitle] = useState(article.title)
  const [body, setBody] = useState(article.body)
  return (
    <form
      data-ui="encyclopedia.detach-form"
      className="flex max-w-3xl flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault()
        onSave(title.trim(), body)
      }}>
      <label className="text-[13px] text-[#3f3f3f]">
        {t('knowledge.hub.wiki_title')}
        <input
          data-ui="encyclopedia.detach-title"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          className="mt-1 block h-9 w-full rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10"
        />
      </label>
      <label className="text-[13px] text-[#3f3f3f]">
        {t('knowledge.encyclopedia.body')}
        <textarea
          data-ui="encyclopedia.detach-body"
          value={body}
          onChange={(event) => setBody(event.target.value)}
          className="mt-1 block min-h-40 w-full rounded-[8px] bg-[#f5f5f5] px-3 py-2 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10"
        />
      </label>
      <div className="flex justify-end gap-2">
        <button
          type="button"
          className="h-9 rounded-[8px] border border-[#e5e5e5] bg-white px-3.5 text-[13px] text-[#171717] hover:bg-[#fafafa]"
          onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <button type="submit" className="h-9 rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white">
          {t('knowledge.encyclopedia.save')}
        </button>
      </div>
    </form>
  )
}
