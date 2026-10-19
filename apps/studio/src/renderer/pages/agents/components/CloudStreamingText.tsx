import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

/** Streaming text display for cloud agent SSE events. */
export function CloudStreamingText({
  isStreaming,
  text,
  onStreamData
}: {
  isStreaming: boolean
  text: string
  onStreamData: ((delta: string) => void) | undefined
}) {
  const { t } = useTranslation()
  const containerRef = useRef<HTMLDivElement>(null)
  const [streaming, setStreaming] = useState(false)

  useEffect(() => {
    setStreaming(Boolean(onStreamData))
  }, [onStreamData])

  useEffect(() => {
    if (containerRef.current && streaming) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight
    }
  }, [text, streaming])

  return (
    <div ref={containerRef} className="min-h-[2rem] whitespace-pre-wrap text-sm">
      {text}
      {isStreaming && (
        <span className="ml-0.5 inline-block h-4 w-1 animate-pulse bg-current align-text-bottom" />
      )}
      {streaming && !text && (
        <span className="text-muted-foreground">{t('agents.cloudStream.streaming')}</span>
      )}
    </div>
  )
}
