import { type ComponentProps, use } from 'react'
import { createPortal } from 'react-dom'

import ConversationComposerStage from '@renderer/components/composer/ConversationComposerStage'
import { WorkbenchComposerHostContext } from '@renderer/pages/agents/workbenchComposerHost'

import { useRightPanelComposerElevated, useRightPanelPresentationMaximized } from '../panes/Shell'

export type ConversationStageCenterProps = ComponentProps<typeof ConversationComposerStage>

export default function ConversationStageCenter(props: ConversationStageCenterProps) {
  const mainVisible = props.mainVisible ?? props.placement === 'docked'
  const paneMaximized = useRightPanelPresentationMaximized()
  const paneElevatesComposer = useRightPanelComposerElevated()
  const composerHost = use(WorkbenchComposerHostContext)
  const portaledComposer = composerHost && props.composer ? createPortal(props.composer, composerHost) : null

  return (
    <>
      {portaledComposer}
      <div className="flex h-full min-h-0 flex-1 flex-col justify-between">
        <ConversationComposerStage
          {...props}
          composer={portaledComposer ? null : props.composer}
          mainVisible={mainVisible && !paneMaximized}
          composerElevated={props.composerElevated || paneElevatesComposer}
        />
      </div>
    </>
  )
}
