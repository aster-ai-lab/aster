import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { TiptapEditorHTMLElement } from '@tiptap/core'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { QuickPanelProvider } from '@renderer/components/QuickPanel'

import { COMPOSER_INPUT_MAX_LENGTH, serializeComposerDocument } from '../composerDraft'
import ComposerSurfaceRuntime, { type ComposerSurfaceProps } from '../ComposerSurfaceRuntime'
import { COMPOSER_TOKEN_NODE_NAME } from '../ComposerTokenNode'
import { ComposerToolRuntimeProvider } from '../ComposerToolRuntime'
import { createPromptVariableContent, createPromptVariableToken } from '../promptVariables'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

// jsdom has no Range layout; these contracts observe edits, not geometry.
globalThis.Range.prototype.getClientRects = () => Object.assign([], { item: () => null })
globalThis.Range.prototype.getBoundingClientRect = () => new globalThis.DOMRect()

afterEach(cleanup)

function Harness({ onSendDraft }: Pick<ComposerSurfaceProps, 'onSendDraft'>) {
  const [text, setText] = useState('${city}')
  return (
    <QuickPanelProvider>
      <ComposerToolRuntimeProvider actions={{ addNewTopic: () => undefined, onTextChange: setText }}>
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
          supportedExts={[]}
          setFiles={() => undefined}
          filesCount={0}
          isExpanded={false}
          onExpandedChange={() => undefined}
          quickPanelEnabled={false}
          enableDragDrop={false}
          enableSpellCheck={false}
          fontSize={14}
          narrowMode={false}
        />
      </ComposerToolRuntimeProvider>
    </QuickPanelProvider>
  )
}

async function setup(content = createPromptVariableContent('${city}'), selection = 1) {
  const onSendDraft = vi.fn()
  render(<Harness onSendDraft={onSendDraft} />)
  const input = (await screen.findByRole('textbox', { name: 'Message' })) as TiptapEditorHTMLElement
  const editor = await waitFor(() => {
    if (!input.editor) throw new Error('Composer editor is not initialized')
    return input.editor
  })
  await act(async () => {
    editor.commands.setContent(content)
    editor.commands.setNodeSelection(selection)
  })
  fireEvent.compositionStart(input)
  expect(editor.view.composing).toBe(true)
  return { input, editor, onSendDraft }
}

async function send(input: HTMLElement) {
  // Send after WebKit's candidate-confirmation grace period, without a real sleep.
  const clock = vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 1000)
  try {
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter', keyCode: 13 })
    })
  } finally {
    clock.mockRestore()
  }
}

describe('real composer prompt-variable composition', () => {
  it('commits the final value, ends composition, and lets a later Enter send', async () => {
    const { input, editor, onSendDraft } = await setup()
    await act(async () => {
      fireEvent.compositionUpdate(input, { data: 'shanghai' })
      fireEvent.compositionEnd(input, { data: '上海' })
    })

    expect(editor.view.composing).toBe(false)
    await send(input)
    expect(onSendDraft).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ text: '上海' }))
  })

  it.each([false, true])(
    'preserves the old value when composition is canceled (empty update: %s)',
    async (emptyUpdate) => {
      const { input, editor, onSendDraft } = await setup()
      await act(async () => {
        fireEvent.compositionUpdate(input, { data: 'shanghai' })
        if (emptyUpdate) fireEvent.compositionUpdate(input, { data: '' })
        fireEvent.compositionEnd(input, { data: '' })
      })

      expect(editor.state.doc.firstChild?.firstChild?.attrs.promptText).toBe('${city}')
      expect(editor.view.composing).toBe(false)
      await send(input)
      expect(onSendDraft).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ text: '${city}' }))
    }
  )

  it('uses the last composition update only when the end event has no data', async () => {
    const { input, editor, onSendDraft } = await setup()
    await act(async () => {
      fireEvent.compositionUpdate(input, { data: '上海' })
      fireEvent(input, new Event('compositionend', { bubbles: true }))
    })

    expect(editor.view.composing).toBe(false)
    await send(input)
    expect(onSendDraft).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ text: '上海' }))
  })

  it('rejects a composition at the input limit, ends composition, and lets a later Enter send', async () => {
    const text = 'x'.repeat(COMPOSER_INPUT_MAX_LENGTH)
    const { input, editor, onSendDraft } = await setup(
      {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            content: [
              { type: 'text', text },
              { type: COMPOSER_TOKEN_NODE_NAME, attrs: createPromptVariableToken('city', '', 0) }
            ]
          }
        ]
      },
      COMPOSER_INPUT_MAX_LENGTH + 1
    )
    await act(async () => {
      fireEvent.compositionUpdate(input, { data: 'shanghai' })
      fireEvent.compositionEnd(input, { data: '上海' })
    })

    expect(editor.view.composing).toBe(false)
    expect(serializeComposerDocument(editor).text).toBe(text)
    expect(editor.state.doc.firstChild?.lastChild?.attrs.promptText).toBe('')
    await send(input)
    expect(onSendDraft).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ text }))
  })
})
