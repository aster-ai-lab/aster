import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { TiptapEditorHTMLElement } from '@tiptap/core'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { QuickPanelProvider } from '@renderer/components/QuickPanel'

import ComposerSurfaceRuntime, { type ComposerSurfaceProps } from '../ComposerSurfaceRuntime'
import {
  ComposerToolDerivedStateProvider,
  ComposerToolRuntimeProvider,
  useComposerToolDispatch,
  useComposerToolLauncherVersion,
  useComposerToolState
} from '../ComposerToolRuntime'
import { agentFileExtensions, useComposerFileCapabilities } from '../variants/shared/useComposerFileCapabilities'

const translate = (key: string) => key
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: translate }) }))

globalThis.Range.prototype.getClientRects = () => Object.assign([], { item: () => null })
globalThis.Range.prototype.getBoundingClientRect = () => new globalThis.DOMRect()

afterEach(cleanup)

function HomeComposer({ onSendDraft }: Pick<ComposerSurfaceProps, 'onSendDraft'>) {
  const [text, setText] = useState('')
  const { files } = useComposerToolState()
  const { setFiles } = useComposerToolDispatch()
  useComposerToolLauncherVersion()
  const { supportedExts } = useComposerFileCapabilities(undefined)
  const extensions = supportedExts.length === 0 ? agentFileExtensions() : supportedExts

  return (
    <ComposerToolDerivedStateProvider couldAddImageFile extensions={extensions}>
      <ComposerSurfaceRuntime
        text={text}
        onTextChange={setText}
        tokens={[]}
        onTokensChange={() => undefined}
        managedTokenKinds={[]}
        placeholder="Message"
        sendMessageShortcut={['Enter']}
        sendDisabled={false}
        isLoading={false}
        onSendDraft={onSendDraft}
        onPause={() => undefined}
        supportedExts={extensions}
        setFiles={setFiles}
        filesCount={files.length}
        isExpanded={false}
        onExpandedChange={() => undefined}
        topContent={<button type="button">Review plan</button>}
        dockContent={<button type="button">Dock action</button>}
        quickPanelEnabled={false}
        enableDragDrop={false}
        enableSpellCheck={false}
        fontSize={14}
        narrowMode={false}
      />
    </ComposerToolDerivedStateProvider>
  )
}

describe('home composer lifecycle', () => {
  it('keeps the rich editor usable before a model is selected', async () => {
    const onSendDraft = vi.fn()
    render(
      <QuickPanelProvider>
        <ComposerToolRuntimeProvider actions={{ addNewTopic: () => undefined, onTextChange: () => undefined }}>
          <HomeComposer onSendDraft={onSendDraft} />
        </ComposerToolRuntimeProvider>
      </QuickPanelProvider>
    )
    const input = (await screen.findByRole('textbox', { name: 'Message' })) as TiptapEditorHTMLElement
    const editor = await waitFor(() => {
      if (!input.editor) throw new Error('Composer editor is not initialized')
      return input.editor
    })
    // jsdom does not implement native contenteditable insertion; use the real editor transaction.
    await act(async () => {
      input.focus()
      editor.commands.insertContent('draft')
    })
    expect(input).toHaveTextContent('draft')
    expect(screen.getByRole('button', { name: 'Review plan' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Dock action' })).toBeVisible()
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter', keyCode: 13 })
    expect(onSendDraft).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ text: 'draft' }))
  })
})
