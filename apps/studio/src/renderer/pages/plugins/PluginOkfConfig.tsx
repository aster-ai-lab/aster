import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { ChevronLeft, Settings } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import Scrollbar from '@renderer/components/Scrollbar'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { displayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings/appWorkspace'
import { toast } from '@renderer/services/toast'
import { readStudioHomePath } from '@shared/studioRuntime'

import { listPluginOkfPages, readPluginOkfPage, savePluginOkfPage, type PluginOkfPage } from './pluginOkf'

export function PluginOkfConfig({ slug, name, onBack }: { slug: string; name: string; onBack: () => void }) {
  const { t } = useTranslation()
  const [workspaceStored] = usePreference('app.workspace.path')
  const workspaceWiki = useMemo(() => {
    const root = displayedWorkspacePath(
      typeof workspaceStored === 'string' ? workspaceStored : '',
      readStudioHomePath()
    )
    return root ? `${root}/wiki` : undefined
  }, [workspaceStored])
  const [pages, setPages] = useState<PluginOkfPage[]>([])
  const [status, setStatus] = useState<'loading' | 'ready' | 'missing' | 'error'>('loading')
  const [editor, setEditor] = useState<{ path: string; title: string; markdown: string } | null>(null)
  const [opening, setOpening] = useState(false)

  useEffect(() => {
    let cancelled = false
    setStatus('loading')
    void listPluginOkfPages(slug)
      .then((next) => {
        if (cancelled) return
        setPages(next)
        setStatus('ready')
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setStatus(error instanceof MemberOrgError && error.message === 'missing_plugin' ? 'missing' : 'error')
      })
    return () => {
      cancelled = true
    }
  }, [slug])

  const openPage = async (page: PluginOkfPage) => {
    if (opening) return
    setOpening(true)
    try {
      const loaded = await readPluginOkfPage(slug, page.path)
      setEditor(loaded)
    } catch {
      toast.error(t('plugins.okf.load_failed'))
    } finally {
      setOpening(false)
    }
  }

  const save = async (markdown: string) => {
    if (!editor) return
    try {
      await savePluginOkfPage({ slug, path: editor.path, markdown, workspaceWiki })
      setEditor((current) => (current ? { ...current, markdown } : current))
      const next = await listPluginOkfPages(slug)
      setPages(next)
      toast.success(t('plugins.okf.saved'))
    } catch {
      toast.error(t('plugins.okf.save_failed'))
    }
  }

  return (
    <div data-ui="plugins.okf.page" className="flex h-full min-h-0 min-w-0 flex-col bg-white">
      <div className="flex shrink-0 items-center gap-2 px-6 pt-4 pb-3">
        <button
          type="button"
          onClick={onBack}
          className="flex h-[30px] shrink-0 items-center gap-1 rounded-[8px] px-2 text-[13px] text-[#171717] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10">
          <ChevronLeft className="size-4" strokeWidth={1.7} aria-hidden />
          {t('plugins.okf.back')}
        </button>
        <h1 className="m-0 min-w-0 truncate text-[20px] leading-tight font-semibold text-[#171717]">
          {t('plugins.okf.title', { name })}
        </h1>
      </div>
      <Scrollbar className="min-h-0 min-w-0 flex-1">
        <div className="px-6 pb-6">
          {status === 'loading' ? (
            <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">{t('common.loading')}</p>
          ) : status === 'missing' ? (
            <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">{t('plugins.okf.missing')}</p>
          ) : status === 'error' ? (
            <p role="alert" className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
              {t('plugins.okf.load_failed')}
            </p>
          ) : pages.length === 0 ? (
            <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">{t('plugins.okf.empty')}</p>
          ) : (
            <ul className="m-0 flex list-none flex-col gap-2 p-0">
              {pages.map((page) => (
                <li key={page.path}>
                  <button
                    type="button"
                    onClick={() => void openPage(page)}
                    className="flex w-full min-w-0 items-center justify-between gap-3 rounded-[10px] border border-[#ececec] bg-white px-3 py-2.5 text-left outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                    <span className="min-w-0">
                      <span className="block truncate text-[14px] font-medium text-[#171717]">{page.title}</span>
                      <span className="mt-0.5 block truncate text-[12px] text-[#8a8a8a]">{page.path}</span>
                    </span>
                    <span className="flex h-7 shrink-0 items-center gap-1 rounded-[8px] border border-[#e5e5e5] bg-white px-2 text-[12px] text-[#171717]">
                      <Settings className="size-3.5" strokeWidth={1.7} aria-hidden />
                      {t('plugins.okf.edit')}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Scrollbar>
      <Dialog
        open={editor !== null}
        onOpenChange={(open) => {
          if (!open) setEditor(null)
        }}>
        <DialogContent
          data-ui="plugins.okf.editor"
          aria-describedby={undefined}
          className="flex h-[min(760px,calc(100dvh-2rem))] w-[min(920px,calc(100vw-1.5rem))] max-w-none flex-col gap-3 overflow-hidden p-4 sm:max-w-none">
          <DialogTitle className="truncate text-[16px]">{editor?.title ?? ''}</DialogTitle>
          {editor ? <OkfEditor markdown={editor.markdown} onSave={save} /> : null}
        </DialogContent>
      </Dialog>
    </div>
  )
}

function OkfEditor({ markdown, onSave }: { markdown: string; onSave: (markdown: string) => Promise<void> }) {
  const { t } = useTranslation()
  const content = useMemo(() => ({ type: 'markdown' as const, markdown }), [markdown])
  const [editors, setEditors] = useState<typeof import('@a3s-lab/office/react') | null>(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let cancelled = false
    void import('@a3s-lab/office/styles.css').catch(() => undefined)
    void import('@a3s-lab/office/react')
      .then((mod) => {
        if (!cancelled) setEditors(mod)
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  if (failed) {
    return <p className="m-0 text-[13px] text-[#8a8a8a]">{t('plugins.okf.unavailable')}</p>
  }
  if (!editors) return <div className="min-h-0 flex-1" />
  const Editor = editors.MarkdownEditor
  return (
    <div className="flex min-h-0 w-full min-w-0 flex-1 flex-col">
      <Editor
        content={content}
        preview={false}
        presentation="note"
        theme="light"
        onChange={() => undefined}
        onSave={(next) => onSave(next)}
      />
    </div>
  )
}
