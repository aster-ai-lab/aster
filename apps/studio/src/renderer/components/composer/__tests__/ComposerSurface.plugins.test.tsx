import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useCallback, useRef, useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import { QuickPanelProvider } from '@renderer/components/QuickPanel'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type { PluginComposerDraft } from '@renderer/types/workbenchPlugin'

import ComposerSurface, { type ComposerSurfaceActions } from '../ComposerSurface'
import { ComposerToolRuntimeProvider } from '../ComposerToolRuntime'
import type { ComposerDraftToken } from '../tokens'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

// jsdom has no Range layout; these contracts observe edits, not geometry.
globalThis.Range.prototype.getClientRects = () => Object.assign([], { item: () => null })
globalThis.Range.prototype.getBoundingClientRect = () => new globalThis.DOMRect()

const hosts: WorkbenchPluginHost[] = []
afterEach(async () => {
  cleanup()
  await Promise.all(hosts.splice(0).map((host) => host.close()))
})

type Mutation = 'text' | 'scope' | 'editable' | 'tokens'

function Harness({ mutation = 'text' }: { mutation?: Mutation }) {
  const [text, setText] = useState('draft')
  const [draftScope, setDraftScope] = useState('first')
  const [editable, setEditable] = useState(true)
  const [tokens, setTokens] = useState<readonly ComposerDraftToken[]>([])
  const [ready, setReady] = useState(false)
  const actions = useRef<ComposerSurfaceActions | undefined>(undefined)
  const onActionsChange = useCallback((next: ComposerSurfaceActions) => {
    actions.current = next
    setReady(true)
  }, [])
  const mutate = (changed: boolean) => {
    if (mutation === 'text') setText(changed ? 'new draft' : 'draft')
    if (mutation === 'scope') setDraftScope(changed ? 'second' : 'first')
    if (mutation === 'editable') setEditable(!changed)
    if (mutation === 'tokens')
      setTokens(changed ? [{ id: 'folder', kind: 'folder', label: 'Project', payload: { path: '/project' } }] : [])
  }
  return (
    <QuickPanelProvider>
      <ComposerToolRuntimeProvider
        actions={{
          addNewTopic: () => undefined,
          onTextChange: (next) => setText(typeof next === 'function' ? next(text) : next)
        }}>
        <output aria-label="Current draft">{text}</output>
        <button type="button" onClick={() => mutate(true)}>
          Change
        </button>
        <button type="button" onClick={() => mutate(false)}>
          Restore
        </button>
        <button
          type="button"
          disabled={!ready}
          onClick={() => {
            actions.current?.onTextChange('new draft')
            actions.current?.onTextChange('draft')
          }}>
          Batched input
        </button>
        <ComposerSurface
          draftScope={draftScope}
          text={text}
          onTextChange={setText}
          tokens={tokens}
          onTokensChange={setTokens}
          managedTokenKinds={[]}
          placeholder="Message"
          sendDisabled
          hideSend
          isLoading={false}
          onSendDraft={() => undefined}
          onPause={() => undefined}
          supportedExts={[]}
          setFiles={() => undefined}
          filesCount={0}
          isExpanded={false}
          onExpandedChange={() => undefined}
          quickPanelEnabled={false}
          enableDragDrop={false}
          enableSpellCheck={false}
          editable={editable}
          fontSize={14}
          narrowMode={false}
          onActionsChange={onActionsChange}
        />
      </ComposerToolRuntimeProvider>
    </QuickPanelProvider>
  )
}

async function setup(mutation?: Mutation) {
  let finish!: (text: string) => void
  const pending = new Promise<string>((resolve) => {
    finish = resolve
  })
  const snapshots: PluginComposerDraft[] = []
  const host = new WorkbenchPluginHost('composer-revision-test')
  hosts.push(host)
  await host.setPlugins([
    {
      id: 'transform',
      generation: 1,
      scopes: ['window'],
      contributions: [
        {
          id: 'replace',
          slot: 'composer',
          labelKey: 'common.add',
          invoke: (_context, _text, draft) => {
            snapshots.push(draft)
            return pending
          }
        }
      ]
    }
  ])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  render(
    <PluginScopeProvider scope={scope}>
      <Harness mutation={mutation} />
    </PluginScopeProvider>
  )
  await waitFor(() => expect(screen.getByRole('button', { name: 'Batched input' })).toBeEnabled())
  await waitFor(() => expect(screen.queryByRole('textbox', { name: 'Message' })).toBeNull(), {
    timeout: 10_000
  })
  expect(screen.getAllByText('draft')).toHaveLength(2)
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'common.add' }))
  return {
    user,
    snapshots,
    finish: async () => {
      await act(async () => {
        finish('plugin edit')
        await pending
      })
    }
  }
}

describe('actual Composer plugin draft revisions', () => {
  it('accepts an unchanged draft through the real rich editor', async () => {
    const { finish } = await setup()
    await finish()
    expect(screen.getByLabelText('Current draft')).toHaveTextContent('plugin edit')
  })

  it.each<Mutation>(['text', 'scope', 'editable', 'tokens'])(
    'refuses a delayed edit after %s changes and returns',
    async (mutation) => {
      const { user, finish } = await setup(mutation)
      await user.click(screen.getByRole('button', { name: 'Change' }))
      await user.click(screen.getByRole('button', { name: 'Restore' }))
      expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
      await finish()
      expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
    }
  )

  it('refuses a delayed edit after two rich-editor input changes in one React batch', async () => {
    const { user, finish } = await setup()
    await user.click(screen.getByRole('button', { name: 'Batched input' }))
    expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
    await finish()
    expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
  })

  it('allows a fresh invocation after rejecting an obsolete revision', async () => {
    const { user, finish, snapshots } = await setup()
    const first = snapshots[0]
    await user.click(screen.getByRole('button', { name: 'Change' }))
    await user.click(screen.getByRole('button', { name: 'Restore' }))
    await finish()
    expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
    await user.click(screen.getByRole('button', { name: 'common.add' }))
    await waitFor(() => expect(screen.getByLabelText('Current draft')).toHaveTextContent('plugin edit'))
    expect(snapshots[1].id).toBe(first.id)
    expect(snapshots[1].revision).toBeGreaterThan(first.revision)
  })

  it('does not let a plugin rewrite the captured draft revision', async () => {
    const { finish, snapshots } = await setup()
    expect(snapshots[0].text).toBe('draft')
    expect(Reflect.set(snapshots[0], 'revision', -1)).toBe(false)
    expect(Reflect.set(snapshots[0], 'id', 'foreign')).toBe(false)
    await finish()
    expect(screen.getByLabelText('Current draft')).toHaveTextContent('plugin edit')
  })

  it('consumes a revision even when the accepted first edit keeps the same text', async () => {
    let first!: (text: string) => void
    let second!: (text: string) => void
    const a = new Promise<string>((resolve) => {
      first = resolve
    })
    const b = new Promise<string>((resolve) => {
      second = resolve
    })
    const host = new WorkbenchPluginHost('competing-composer-test')
    hosts.push(host)
    await host.setPlugins([
      {
        id: 'transform',
        generation: 1,
        scopes: ['window'],
        contributions: [
          { id: 'first', slot: 'composer', labelKey: 'common.add', invoke: () => a },
          { id: 'second', slot: 'composer', labelKey: 'common.delete', invoke: () => b }
        ]
      }
    ])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    render(
      <PluginScopeProvider scope={scope}>
        <Harness />
      </PluginScopeProvider>
    )
    await waitFor(() => expect(screen.queryByRole('textbox', { name: 'Message' })).toBeNull(), {
      timeout: 10_000
    })
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'common.add' }))
    await user.click(screen.getByRole('button', { name: 'common.delete' }))
    await act(async () => {
      first('draft')
      await a
    })
    await act(async () => {
      second('stale edit')
      await b
    })
    expect(screen.getByLabelText('Current draft')).toHaveTextContent(/^draft$/)
  })
})
