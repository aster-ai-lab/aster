import type { MarkdownSource } from '@a3s-lab/ui'
import { type FC, lazy, Suspense, useMemo } from 'react'
import type { Components } from 'streamdown'

import type { Citation } from '@renderer/types/message'

import ChatMarkdownRuntime from './ChatMarkdownRuntime'
import { OpenUiMessage } from './OpenUiMessage'
import { splitOpenUiFences } from './splitOpenUiFences'
import { scanStandaloneHtmlArtifact } from './standaloneHtmlArtifact'

export interface ChatMarkdownProps {
  block: MarkdownSource
  inlineHtmlPreviewMode?: InlineHtmlPreviewMode
  postProcess?: (text: string) => string
  className?: string
  components?: Partial<Components>
  trustedCitations?: readonly Citation[]
  linkifyFilePaths?: boolean
}

export type InlineHtmlPreviewMode = 'generating' | 'ready'

const StandaloneHtmlArtifactRenderer = lazy(() => import('./StandaloneHtmlArtifactRenderer'))

const ChatMarkdown: FC<ChatMarkdownProps> = (props) => {
  const { block, inlineHtmlPreviewMode } = props
  const aixSegments = useMemo(() => splitOpenUiFences(block.content), [block.content])
  const standaloneHtmlArtifact = useMemo(
    () => (inlineHtmlPreviewMode ? scanStandaloneHtmlArtifact(block.content, block.status === 'streaming') : undefined),
    [block.content, block.status, inlineHtmlPreviewMode]
  )

  if (aixSegments.some((segment) => segment.kind !== 'markdown')) {
    return (
      <>
        {aixSegments.map((segment) =>
          segment.kind === 'openui' ? (
            <OpenUiMessage
              key={segment.start}
              source={segment.source}
              streaming={block.status === 'streaming' || segment.incomplete === true}
            />
          ) : (
            <ChatMarkdown
              key={segment.start}
              {...props}
              block={{ ...block, id: `${block.id}:prose:${segment.start}`, content: segment.source }}
            />
          )
        )}
      </>
    )
  }

  if (standaloneHtmlArtifact && inlineHtmlPreviewMode) {
    return (
      <Suspense fallback={null}>
        <StandaloneHtmlArtifactRenderer
          artifact={standaloneHtmlArtifact}
          block={block}
          inlineHtmlPreviewMode={inlineHtmlPreviewMode}
        />
      </Suspense>
    )
  }

  return <ChatMarkdownRuntime {...props} />
}

export default ChatMarkdown
