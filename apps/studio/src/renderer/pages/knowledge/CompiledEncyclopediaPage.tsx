import { MarkdownEditor } from '@a3s-lab/office/react'
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { driveApi } from '@renderer/drive/driveTransport'
import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'
import { isCloudWorkspace } from '@renderer/pages/documents/cloudWorkspace'
import { listLocalEntries, localEntryStatus, readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'
import { defaultAppWorkspacePath, knowledgeCorpusAllowed } from '@renderer/pages/settings/GeneralSettings/appWorkspace'
import { cn } from '@renderer/utils/style'
import { readStudioHomePath } from '@shared/studioRuntime'

import { compileWorkspaceOkf, searchWorkspaceOkf } from './compileLocalOkf'
import { personalVaultPath } from './knowledgeVault'
import { isOkfConcept, sourceStem } from './okfConcept'
import { WikiRetrievalStatus } from './WikiRetrievalStatus'

interface CompiledPage {
  id: string
  title: string
  markdown: string
}

const MAX_BYTES = 256 * 1024

async function readText(root: string, path: string): Promise<string | null> {
  try {
    const bytes = await readLocalDocumentBytes(root, path, undefined, MAX_BYTES)
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

/** Markdown files under the default workspace `wiki/local`. Compiled pages stay at `wiki/`. */
async function readLocalLibrary(root: string): Promise<CompiledPage[]> {
  const pages: CompiledPage[] = []
  const visit = async (directory: string, depth: number) => {
    if (pages.length >= 200 || depth > 6) return
    let listing: Awaited<ReturnType<typeof listLocalEntries>>
    try {
      listing = await listLocalEntries(root, directory)
    } catch (error) {
      if (localEntryStatus(error) === 404) return
      throw error
    }
    for (const entry of listing.entries) {
      if (entry.name.startsWith('.')) continue
      if (entry.kind === 'directory') {
        await visit(entry.path, depth + 1)
        continue
      }
      if (!entry.name.endsWith('.md')) continue
      const markdown = await readText(root, entry.path)
      if (!markdown) continue
      pages.push({ id: entry.path, title: sourceStem(entry.name), markdown })
    }
  }
  await visit(personalVaultPath(root), 0)
  pages.sort((left, right) => left.title.localeCompare(right.title, 'zh'))
  return pages
}

async function loadCloud(): Promise<CompiledPage[]> {
  const articles = await driveApi.listEncyclopedia()
  return articles.flatMap((article) => {
    if (!isOkfConcept(article.body)) return []
    return [{ id: article.id, title: article.title, markdown: article.body }]
  })
}

export function CompiledEncyclopediaPage({
  corner,
  root,
  embedded = false
}: { corner?: ReactNode; root?: string; embedded?: boolean } = {}) {
  const { t } = useTranslation()
  const homePath = readStudioHomePath()
  const cloud = Boolean(root && isCloudWorkspace(root))
  const workspaceRoot = cloud ? root! : defaultAppWorkspacePath(homePath)
  const canCompile = !cloud && knowledgeCorpusAllowed(workspaceRoot, readStudioHomePath())
  const [pages, setPages] = useState<CompiledPage[]>([])
  const [index, setIndex] = useState('')
  const [phase, setPhase] = useState<'loading' | 'ready' | 'error'>('loading')
  const [openedId, setOpenedId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [searched, setSearched] = useState<CompiledPage[] | null>(null)
  const [searchPhase, setSearchPhase] = useState<'idle' | 'not-ready' | 'published'>('idle')
  const [reloadKey, setReloadKey] = useState(0)

  const reload = useCallback(() => setReloadKey((current) => current + 1), [])
  const refresh = useCallback(() => {
    const run = async () => {
      if (canCompile && workspaceRoot) await compileWorkspaceOkf(workspaceRoot)
      reload()
    }
    void run().catch(() => setPhase('error'))
  }, [canCompile, reload, workspaceRoot])

  useEffect(() => {
    let live = true
    setPhase('loading')
    setOpenedId(null)
    const load = async () => {
      if (cloud) return { index: '', pages: await loadCloud() }
      if (!workspaceRoot) return { index: '', pages: [] }
      return { index: '', pages: await readLocalLibrary(workspaceRoot) }
    }
    void load()
      .then((next) => {
        if (!live) return
        setIndex(next.index)
        setPages(next.pages)
        setPhase('ready')
      })
      .catch(() => {
        if (!live) return
        setIndex('')
        setPages([])
        setPhase('error')
      })
    return () => {
      live = false
    }
  }, [cloud, reloadKey, workspaceRoot])

  useEffect(() => {
    const needle = query.trim()
    if (!needle) {
      setSearched(null)
      setSearchPhase('idle')
      return
    }
    let live = true
    const run = async () => {
      if (cloud) {
        const found = await driveApi.searchEncyclopedia(needle)
        if (!live) return
        setSearchPhase(found.phase)
        setSearched(
          found.phase === 'not-ready'
            ? []
            : found.hits.map((hit) => ({
                id: hit.id,
                title: hit.heading || hit.folder || hit.body.slice(0, 24),
                markdown: hit.body
              }))
        )
        return
      }
      const found = workspaceRoot ? await searchWorkspaceOkf(workspaceRoot, needle) : null
      if (!live) return
      if (!found) {
        setSearchPhase('not-ready')
        setSearched([])
        return
      }
      setSearchPhase(found.phase)
      setSearched(
        found.phase === 'not-ready'
          ? []
          : found.hits.map((hit) => ({ id: hit.name, title: hit.name.replace(/\.md$/, ''), markdown: hit.passage }))
      )
    }
    void run().catch(() => {
      if (!live) return
      setSearchPhase('not-ready')
      setSearched([])
    })
    return () => {
      live = false
    }
  }, [cloud, query, workspaceRoot])

  const visible = useMemo(() => {
    if (query.trim()) return searched ?? []
    return pages
  }, [pages, query, searched])
  const opened = pages.find((page) => page.id === openedId) ?? null
  const landing = !opened && phase === 'ready' && index && searchPhase !== 'not-ready' && !query.trim() ? index : ''

  return (
    <div className="flex h-full min-h-0 flex-col bg-white">
      {embedded ? null : (
        <div className="shrink-0 bg-white px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
                {t('knowledge.notes.title')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  aria-label={t(canCompile ? 'knowledge.compiled.refresh' : 'knowledge.notes.refresh')}
                  onClick={refresh}
                  className="h-[30px] rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717]">
                  {t(canCompile ? 'knowledge.compiled.refresh' : 'knowledge.notes.refresh')}
                </button>
                {corner}
              </div>
            </div>
            <p className="m-0 text-[15px] text-muted-foreground">
              {t(cloud ? 'knowledge.compiled.subtitle.shared' : 'knowledge.compiled.subtitle')}
            </p>
            {!cloud && !canCompile ? (
              <p className="m-0 text-[13px] text-muted-foreground">{t('knowledge.compiled.workspace_required')}</p>
            ) : null}
            <WikiRetrievalStatus />
          </div>
        </div>
      )}
      <div className="flex min-h-0 flex-1">
        <aside
          className={cn(
            'flex h-full min-h-0 flex-col overflow-hidden bg-white px-3 pt-1 pb-4',
            pageSidebarWidthClassName,
            pageSidebarEdgeClassName
          )}>
          <label className="mb-2 flex h-[30px] items-center rounded-[8px] bg-[#f5f5f5] px-2">
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('knowledge.compiled.search')}
              aria-label={t('knowledge.compiled.search')}
              className="h-full w-full bg-transparent text-[13px] outline-none"
            />
          </label>
          <nav
            aria-label={t('knowledge.compiled.list')}
            className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
            {visible.map((page) => (
              <button
                key={page.id}
                type="button"
                aria-current={page.id === openedId ? 'page' : undefined}
                onClick={() => setOpenedId(page.id)}
                className={cn(
                  'h-9 truncate rounded-[8px] px-2.5 text-left text-[13px] hover:bg-[#f7f7f7]',
                  page.id === openedId ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f]'
                )}>
                {page.title}
              </button>
            ))}
          </nav>
        </aside>
        <section className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-white px-6 pt-1">
          {phase === 'loading' ? (
            <p role="status" className="m-0 text-[14px] text-[#8a8a8a]">
              {t('knowledge.compiled.loading')}
            </p>
          ) : null}
          {phase === 'error' ? (
            <p role="alert" className="m-0 text-[14px] text-[#b42318]">
              {t('knowledge.compiled.failed')}
            </p>
          ) : null}
          {phase === 'ready' && !landing && !opened ? (
            <p role="status" className="m-0 text-[14px] text-[#8a8a8a]">
              {searchPhase === 'not-ready'
                ? t('knowledge.compiled.notReady')
                : query.trim() && visible.length === 0
                  ? t('knowledge.compiled.noMatch')
                  : pages.length > 0
                    ? t('knowledge.compiled.pick')
                    : cloud
                      ? t('knowledge.compiled.empty')
                      : t('knowledge.notes.empty')}
            </p>
          ) : null}
          {opened || landing ? (
            <article aria-readonly="true" className="flex min-h-0 flex-1 flex-col overflow-hidden">
              {opened ? <h2 className="m-0 mb-3 text-[18px] font-semibold text-[#171717]">{opened.title}</h2> : null}
              <div className="min-h-0 flex-1">
                <MarkdownEditor
                  content={{ type: 'markdown', markdown: opened ? opened.markdown : landing }}
                  presentation="note"
                  preview
                />
              </div>
            </article>
          ) : null}
        </section>
      </div>
    </div>
  )
}
