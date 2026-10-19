import { act, fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'

import { openPaperloom } from '../paperloom/paperloomApi'
import { WorkbenchHomeActiveContext } from '../workbenchComposerHost'
import { WorkbenchTaskWorkspace } from '../WorkbenchTaskWorkspace'
import { WorkbenchWorkspaceTabsProvider } from '../workbenchWorkspaceTabs'

vi.mock('@renderer/components/chat/panes/Shell', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@renderer/components/chat/panes/Shell')>()),
  RightPanelHeaderControls: () => null,
  useRightPanelState: () => ({}),
  useRightPanelActions: () => ({ canOpen: () => true, tryOpen: () => true, close: vi.fn() })
}))
vi.mock('../paperloom/PaperloomPanel', () => ({
  PaperloomPanel: ({ question, source }: { question: string; source?: { root: string; paths: readonly string[] } }) => {
    const [draft, setDraft] = useState(question)
    return (
      <>
        <input aria-label="paper-draft" value={draft} onChange={(event) => setDraft(event.target.value)} />
        <output aria-label="paper-source">{JSON.stringify(source)}</output>
      </>
    )
  }
}))

function Workspace({
  session = 'task-one',
  home = false,
  root = '/workspace/task-one'
}: {
  session?: string
  home?: boolean
  root?: string
}) {
  return (
    <WorkbenchHomeActiveContext value={home}>
      <WorkbenchWorkspaceTabsProvider>
        <WorkbenchTaskWorkspace
          agentSessionId={session}
          cwd={root}
          renderFiles={() => <input aria-label="file-draft" defaultValue="file state" />}
          renderBrowser={() => null}
        />
      </WorkbenchWorkspaceTabsProvider>
    </WorkbenchHomeActiveContext>
  )
}

describe('paper workspace ownership', () => {
  it('preserves both paper and file state across tab changes and clears papers on task changes', () => {
    const view = render(<Workspace />)
    fireEvent.click(screen.getByRole('button', { name: '文件' }))
    fireEvent.change(screen.getByLabelText('file-draft'), { target: { value: 'unsaved file' } })
    act(() => expect(openPaperloom('source question')).toBe(true))
    fireEvent.change(screen.getByLabelText('paper-draft'), { target: { value: 'unsent question' } })
    fireEvent.click(screen.getByRole('tab', { name: '文件' }))
    expect(screen.getByLabelText('file-draft')).toHaveValue('unsaved file')
    expect(screen.getByLabelText('paper-draft')).not.toBeVisible()
    fireEvent.click(screen.getByRole('tab', { name: '论文精读' }))
    expect(screen.getByLabelText('paper-draft')).toHaveValue('unsent question')
    view.rerender(<Workspace session="task-two" />)
    expect(screen.queryByLabelText('paper-draft')).toBeNull()
  })

  it('does not consume the home composer event from a hidden task workspace', () => {
    render(<Workspace home />)
    act(() => expect(openPaperloom('home question')).toBe(false))
    expect(screen.queryByLabelText('paper-draft')).toBeNull()
  })
  it('pins source selection to the current task root and withdraws it on a root change', () => {
    const view = render(<Workspace />)
    act(() =>
      expect(openPaperloom('read this', { root: '/another-workspace', paths: ['/workspace/task-one/paper.pdf'] })).toBe(
        true
      )
    )
    expect(JSON.parse(screen.getByLabelText('paper-source').textContent ?? '{}')).toEqual({
      root: '/workspace/task-one',
      paths: ['/workspace/task-one/paper.pdf']
    })
    view.rerender(<Workspace root="/workspace/task-two" />)
    expect(screen.queryByLabelText('paper-source')).toBeNull()
  })
})
