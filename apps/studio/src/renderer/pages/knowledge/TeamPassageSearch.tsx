import { Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { filesDriveHref, searchTeamPassages, type TeamPassageHit } from '@renderer/drive/encyclopediaClient'
import { useTabs } from '@renderer/hooks/tab'

type SearchState =
  | { phase: 'idle' }
  | { phase: 'loading' }
  | { phase: 'done'; hits: TeamPassageHit[] }
  | { phase: 'failed' }

/** Team encyclopedia passage search. The server filters by the member's departments. Mount with `key={centerId}`. */
export function TeamPassageSearch() {
  const { t } = useTranslation()
  const { openTab } = useTabs()
  const [query, setQuery] = useState('')
  const [state, setState] = useState<SearchState>({ phase: 'idle' })
  const inflight = useRef<AbortController | null>(null)

  useEffect(() => () => inflight.current?.abort(), [])

  const close = () => {
    inflight.current?.abort()
    setState({ phase: 'idle' })
  }

  const submit = () => {
    const asked = query.trim()
    if (!asked) return
    inflight.current?.abort()
    const controller = new AbortController()
    inflight.current = controller
    setState({ phase: 'loading' })
    void searchTeamPassages(asked, controller.signal)
      .then((hits) => {
        if (!controller.signal.aborted) setState({ phase: 'done', hits })
      })
      .catch(() => {
        if (!controller.signal.aborted) setState({ phase: 'failed' })
      })
  }

  return (
    <div className="relative" data-ui="knowledge.team-search">
      <form
        role="search"
        className="flex h-[30px] w-[260px] items-center gap-1.5 rounded-[8px] bg-[#f5f5f5] px-2.5"
        onSubmit={(event) => {
          event.preventDefault()
          submit()
        }}>
        <Search className="size-3.5 shrink-0 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />
        <input
          type="search"
          data-ui="knowledge.team-search.input"
          aria-label={t('knowledge.encyclopedia.search_passages')}
          placeholder={t('knowledge.encyclopedia.search_passages')}
          value={query}
          spellCheck={false}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape') close()
          }}
          className="min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] [&::-webkit-search-cancel-button]:hidden"
        />
      </form>
      {state.phase === 'idle' ? null : (
        <div
          data-ui="knowledge.team-search.results"
          data-phase={state.phase}
          className="absolute top-[36px] right-0 z-30 max-h-[60vh] w-[420px] overflow-auto rounded-[12px] border border-[#e5e5e5] bg-white p-2 shadow-[0_8px_24px_rgba(0,0,0,0.08)]">
          <div className="flex items-center justify-between px-2 pb-1">
            <span className="text-[12px] text-[#8a8a8a]">{t('knowledge.encyclopedia.passages_heading')}</span>
            <button
              type="button"
              aria-label={t('common.close')}
              className="inline-flex size-6 items-center justify-center rounded-[6px] text-[#8a8a8a] hover:bg-[#f5f5f5]"
              onClick={close}>
              <X className="size-3.5" aria-hidden />
            </button>
          </div>
          {state.phase === 'loading' ? (
            <p className="m-0 px-2 py-4 text-[13px] text-[#8a8a8a]" role="status">
              {t('knowledge.encyclopedia.loading')}
            </p>
          ) : null}
          {state.phase === 'failed' ? (
            <p className="m-0 px-2 py-4 text-[13px] text-[#b42318]" role="alert">
              {t('knowledge.encyclopedia.passages_failed')}
            </p>
          ) : null}
          {state.phase === 'done' && state.hits.length === 0 ? (
            <p data-ui="knowledge.team-search.empty" className="m-0 px-2 py-4 text-[13px] text-[#8a8a8a]">
              {t('knowledge.encyclopedia.passages_empty')}
            </p>
          ) : null}
          {state.phase === 'done' && state.hits.length > 0 ? (
            <ul className="m-0 list-none p-0">
              {state.hits.map((hit) => (
                <li
                  key={hit.passageId}
                  data-ui="knowledge.team-search.hit"
                  className="rounded-[8px] px-2 py-2 hover:bg-[#f7f7f7]">
                  <div className="flex items-center justify-between gap-3">
                    <span className="truncate text-[13px] font-medium text-[#171717]">
                      {hit.heading || hit.folderName}
                    </span>
                    {hit.fileId ? (
                      <a
                        data-ui="knowledge.team-search.open"
                        href={filesDriveHref(hit.fileId)}
                        onClick={(event) => {
                          event.preventDefault()
                          openTab(event.currentTarget.getAttribute('href') ?? '')
                        }}
                        className="shrink-0 text-[12px] text-[#171717] underline">
                        {t('knowledge.encyclopedia.open_source')}
                      </a>
                    ) : null}
                  </div>
                  <p className="m-0 mt-1 line-clamp-2 text-[12px] leading-5 text-[#3f3f3f]">{hit.body}</p>
                  {hit.path.length > 0 ? (
                    <p className="m-0 mt-1 text-[11px] text-[#8a8a8a]">
                      {t('knowledge.encyclopedia.passage_path', { path: hit.path.join(' → ') })}
                    </p>
                  ) : null}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      )}
    </div>
  )
}
