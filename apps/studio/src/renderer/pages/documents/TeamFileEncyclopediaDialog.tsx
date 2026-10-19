import { Dialog, DialogContent, DialogTitle } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { driveApi, type FileEncyclopedia } from '@renderer/drive/driveTransport'

export function TeamFileEncyclopediaDialog({
  file,
  onClose
}: {
  file: { id: string; name: string } | null
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [page, setPage] = useState<FileEncyclopedia | null>(null)
  const [phase, setPhase] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    if (!file) return
    let live = true
    const load = () => {
      driveApi.fileEncyclopedia(file.id).then(
        (next) => {
          if (!live) return
          setPage(next)
          setPhase('ready')
        },
        () => {
          if (!live) return
          setPhase('error')
        }
      )
    }
    setPhase('loading')
    setPage(null)
    load()
    return () => {
      live = false
    }
  }, [file])

  const article = page?.article ?? null
  return (
    <Dialog open={file !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent size="lg" data-ui="documents.encyclopedia.dialog">
        <DialogTitle>{t('documents.encyclopedia.title')}</DialogTitle>
        {file ? (
          <div className="flex max-h-[70vh] flex-col gap-4 overflow-y-auto px-1 pb-2">
            <p className="m-0 text-[13px] text-[#6b6b6b]">
              {t('documents.encyclopedia.source_file')}
              {file.name}
            </p>
            {phase === 'loading' && !article ? (
              <p className="m-0 text-[13px] text-[#8a8a8a]" role="status">
                {t('documents.encyclopedia.loading')}
              </p>
            ) : null}
            {phase === 'error' ? (
              <p className="m-0 text-[13px] text-[#b42318]" role="alert">
                {t('documents.encyclopedia.failed')}
              </p>
            ) : null}
            {phase === 'ready' && !article ? (
              <p className="m-0 text-[13px] text-[#8a8a8a]" role="status">
                {t('documents.encyclopedia.empty')}
              </p>
            ) : null}
            {article ? (
              <>
                <div>
                  <h2 className="m-0 text-[18px] font-semibold text-[#171717]">{article.title}</h2>
                  <p className="m-0 mt-3 text-[14px] leading-6 whitespace-pre-wrap text-[#1a1a1a]">{article.body}</p>
                </div>
                <div>
                  <h3 className="m-0 text-[13px] font-medium text-[#171717]">{t('documents.encyclopedia.source')}</h3>
                  {article.sources.length === 0 ? (
                    <p className="m-0 mt-2 text-[13px] text-[#8a8a8a]">{file.name}</p>
                  ) : (
                    <ul className="m-0 mt-2 list-none space-y-3 p-0">
                      {article.sources.map((source, index) => (
                        <li key={`${source.fileId}-${index}`} className="rounded-[8px] bg-[#f6f7f8] px-3 py-2">
                          <p className="m-0 text-[13px] font-medium text-[#171717]">
                            {source.fileName || file.name}
                            {source.heading ? ` · ${source.heading.replace(/^#+\s*/, '')}` : ''}
                          </p>
                          {source.excerpt ? (
                            <p className="m-0 mt-1 text-[13px] leading-5 text-[#4a4a4a]">{source.excerpt}</p>
                          ) : null}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </>
            ) : null}
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}
