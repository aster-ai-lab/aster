import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import type { ComponentProps, ReactElement } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { resetHomeComposerModeForTests, selectHomeComposerMode } from '../homeComposerMode'
import {
  readPendingProject,
  selectWorkbenchProject,
  useWorkbenchProjectSelection,
  WorkbenchProjectTray as TrayView,
  type WorkbenchProjectSelectionArgs,
  writePendingProject
} from '../WorkbenchProjectTray'
import { selectedWorkspacePaths, WorkbenchWorkspaceFilePicker } from '../WorkbenchWorkspaceFiles'

function WorkbenchProjectTray({
  goal,
  onClearGoal,
  locked,
  goalPaused,
  goalClearable,
  goalSnapshot,
  files,
  ...args
}: WorkbenchProjectSelectionArgs & {
  goal: string | null
  onClearGoal: () => void
  goalPaused?: boolean
  goalClearable?: boolean
  goalSnapshot?: ComponentProps<typeof TrayView>['goalSnapshot']
  files?: ComponentProps<typeof TrayView>['files']
}) {
  const selection = useWorkbenchProjectSelection({ ...args, locked })
  return (
    <TrayView
      selection={selection}
      locked={locked}
      goal={goal}
      goalPaused={goalPaused}
      goalClearable={goalClearable}
      goalSnapshot={goalSnapshot}
      files={files}
      onClearGoal={onClearGoal}
    />
  )
}

const projects = [
  { id: 'p1', name: 'alpha', path: '/work/alpha', type: 'user' },
  { id: 'sys', name: 'scratch', path: '/tmp/scratch', type: 'system' }
]

vi.mock('@renderer/data/hooks/useDataApi', () => ({
  useQuery: () => ({ data: projects })
}))

vi.mock('@renderer/components/resourceCatalog/selectors/WorkspaceSelector', () => ({
  WorkspaceSelector: ({
    trigger,
    value,
    onChange
  }: {
    trigger: ReactElement
    value: string | null
    onChange: (id: string | null) => void
  }) => (
    <div data-selected={value ?? ''}>
      {trigger}
      <button type="button" onClick={() => onChange('p1')}>
        pick alpha
      </button>
    </div>
  )
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string, fallback?: string) => fallback ?? key })
}))

describe('WorkbenchProjectTray', () => {
  beforeEach(() => {
    localStorage.clear()
    resetHomeComposerModeForTests()
  })

  it('offers a folder choice before any project is picked', () => {
    render(<WorkbenchProjectTray goal={null} onClearGoal={vi.fn()} />)

    expect(screen.getByRole('toolbar')).toHaveAttribute('data-workbench-project-tray', 'true')
    expect(screen.getByRole('button', { name: '项目文件夹' })).toHaveTextContent('选择项目文件夹')
  })

  it('leaves the scope row for the folder and the goal', () => {
    act(() => selectHomeComposerMode('code'))
    render(<WorkbenchProjectTray goal="ship it" onClearGoal={vi.fn()} />)
    const toolbar = screen.getByRole('toolbar', { name: '任务范围' })

    expect(within(toolbar).queryByRole('radiogroup', { name: '工作区' })).toBeNull()
    expect(within(toolbar).getByRole('button', { name: '项目文件夹' })).toBeInTheDocument()
    expect(within(toolbar).getByText('ship it')).toBeInTheDocument()
  })

  it('holds the pick as pending until the composer is bound to a session', async () => {
    const { rerender } = render(<WorkbenchProjectTray goal={null} onClearGoal={vi.fn()} />)

    fireEvent.click(screen.getByRole('button', { name: 'pick alpha' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '项目文件夹 alpha' })).toHaveTextContent('alpha'))
    expect(readPendingProject()).toBe('p1')

    const onWorkspaceChange = vi.fn()
    rerender(<WorkbenchProjectTray onWorkspaceChange={onWorkspaceChange} goal={null} onClearGoal={vi.fn()} />)

    await waitFor(() => expect(onWorkspaceChange).toHaveBeenCalledWith('p1'))
    expect(readPendingProject()).toBeNull()
  })

  it('switches the bound session workspace directly and shows its folder', () => {
    writePendingProject(null)
    const onWorkspaceChange = vi.fn()
    render(
      <WorkbenchProjectTray
        workspace={{ type: 'user', name: 'beta', path: '/work/beta' }}
        workspaceId="p2"
        onWorkspaceChange={onWorkspaceChange}
        goal={null}
        onClearGoal={vi.fn()}
      />
    )

    const trigger = screen.getByRole('button', { name: '项目文件夹 beta' })
    expect(trigger).toHaveTextContent('beta')
    expect(trigger).toHaveAttribute('title', '/work/beta')

    fireEvent.click(screen.getByRole('button', { name: 'pick alpha' }))
    expect(onWorkspaceChange).toHaveBeenCalledWith('p1')
    expect(readPendingProject()).toBeNull()
  })

  it('uses current typed goal details and clears terminal or unavailable controls without deleting history', () => {
    const active = {
      status: 'known' as const,
      sessionId: 's1',
      acpSessionId: 'acp',
      executionId: 'run',
      goal: {
        goalId: 'g',
        goalRevision: 1,
        goalSequence: 2,
        planRevision: 1,
        state: 'active' as const,
        objective: '当前目标',
        tick: 'needs_input' as const
      }
    }
    const onClear = vi.fn()
    const view = render(<WorkbenchProjectTray locked goal="过期目标" goalSnapshot={active} onClearGoal={onClear} />)
    expect(screen.getByRole('toolbar')).toHaveTextContent('当前目标')
    expect(screen.queryByText('过期目标')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '移除目标' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.goal.details' }))
    expect(screen.getByText('workbench.goal.tick.needs_input')).toBeVisible()
    view.rerender(
      <WorkbenchProjectTray
        locked
        goal="过期目标"
        goalSnapshot={{ ...active, goal: { ...active.goal, state: 'completed', tick: 'complete' } }}
        onClearGoal={onClear}
      />
    )
    expect(document.querySelector('[data-composer-goal]')).toBeNull()
    view.rerender(
      <WorkbenchProjectTray
        locked
        goal="过期目标"
        goalSnapshot={{ ...active, status: 'unavailable', goal: null, reason: 'stale_binding' }}
        onClearGoal={onClear}
      />
    )
    expect(document.querySelector('[data-composer-goal]')).toBeNull()
    expect(onClear).not.toHaveBeenCalled()
  })

  it('keeps the goal chip in the tray', () => {
    const onClearGoal = vi.fn()
    render(<WorkbenchProjectTray goal="ship it" onClearGoal={onClearGoal} />)

    fireEvent.click(screen.getByRole('button', { name: '移除目标' }))
    expect(onClearGoal).toHaveBeenCalled()
    expect(screen.getByText('ship it')).toBeInTheDocument()
  })

  it('hides the project folder on a task conversation and still shows a goal', () => {
    const onWorkspaceChange = vi.fn()
    const onClearGoal = vi.fn()
    const { rerender } = render(
      <WorkbenchProjectTray
        locked
        workspace={{ type: 'user', name: 'beta', path: '/work/beta' }}
        workspaceId="p2"
        onWorkspaceChange={onWorkspaceChange}
        goal={null}
        onClearGoal={onClearGoal}
      />
    )

    expect(screen.queryByRole('toolbar')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '项目文件夹' })).not.toBeInTheDocument()
    expect(screen.queryByRole('status', { name: '目标' })).not.toBeInTheDocument()

    act(() => selectWorkbenchProject('p1'))
    expect(onWorkspaceChange).not.toHaveBeenCalled()

    rerender(
      <WorkbenchProjectTray
        locked
        workspace={{ type: 'user', name: 'beta', path: '/work/beta' }}
        workspaceId="p2"
        onWorkspaceChange={onWorkspaceChange}
        goal="ship it"
        onClearGoal={onClearGoal}
      />
    )

    const toolbar = screen.getByRole('toolbar')
    const goalChip = toolbar.querySelector('[data-composer-goal]')
    expect(goalChip).toHaveTextContent('ship it')
    expect(goalChip).toHaveClass('ml-auto')
    expect(screen.queryByRole('button', { name: '项目文件夹' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '移除目标' }))
    expect(onClearGoal).toHaveBeenCalled()
  })

  it('shows a paused stored goal on the right of the toolbar without a clear button', () => {
    render(<WorkbenchProjectTray locked goal="清理垃圾文件" goalPaused goalClearable={false} onClearGoal={vi.fn()} />)

    const goal = screen.getByRole('toolbar').querySelector('[data-composer-goal]')
    expect(goal).toHaveTextContent('清理垃圾文件')
    expect(goal).toHaveTextContent('已暂停')
    expect(goal).toHaveAttribute('data-goal-paused', 'true')
    expect(goal).toHaveClass('ml-auto')
    expect(screen.queryByRole('button', { name: '移除目标' })).not.toBeInTheDocument()
  })

  it('keeps the task goal on the right of the workspace file picker', () => {
    render(
      <WorkbenchProjectTray
        locked
        goal="n"
        onClearGoal={vi.fn()}
        files={{ root: '/work/beta', selectedPaths: [], onCommit: vi.fn() }}
      />
    )

    const toolbar = screen.getByRole('toolbar')
    const files = screen.getByRole('button', { name: '工作区文件' })
    const goal = toolbar.querySelector('[data-composer-goal]')
    expect(goal).toHaveTextContent('n')
    expect(goal).toHaveClass('ml-auto')
    expect(files.compareDocumentPosition(goal as Node) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('selects a project requested from the sidebar', async () => {
    render(<WorkbenchProjectTray goal={null} onClearGoal={vi.fn()} />)

    act(() => selectWorkbenchProject('p1'))

    await waitFor(() => expect(screen.getByRole('button', { name: '项目文件夹 alpha' })).toHaveTextContent('alpha'))
    expect(readPendingProject()).toBe('p1')
  })

  it('switches a bound session to the requested project', async () => {
    const onWorkspaceChange = vi.fn()
    render(<WorkbenchProjectTray onWorkspaceChange={onWorkspaceChange} goal={null} onClearGoal={vi.fn()} />)

    act(() => selectWorkbenchProject('p1'))

    await waitFor(() => expect(onWorkspaceChange).toHaveBeenCalledWith('p1'))
    expect(readPendingProject()).toBeNull()
  })

  it('places the workspace file picker beside the project menu', () => {
    render(
      <WorkbenchProjectTray
        goal={null}
        onClearGoal={vi.fn()}
        files={{ root: '/work/alpha', selectedPaths: [], onCommit: vi.fn() }}
      />
    )

    const toolbar = screen.getByRole('toolbar')
    expect(toolbar).toContainElement(screen.getByRole('button', { name: '项目文件夹' }))
    expect(toolbar).toContainElement(screen.getByRole('button', { name: '工作区文件' }))
  })

  it('keeps a workspace file picker on a locked task conversation', () => {
    render(
      <WorkbenchProjectTray
        locked
        goal={null}
        onClearGoal={vi.fn()}
        files={{ root: '/work/beta', selectedPaths: ['/work/beta/notes.md'], onCommit: vi.fn() }}
      />
    )

    expect(screen.queryByRole('button', { name: '项目文件夹' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '工作区文件' })).toHaveTextContent('工作区文件')
    expect(screen.getByRole('button', { name: '工作区文件' })).toHaveAttribute('title', '/work/beta/notes.md')
  })
})

describe('selected workspace paths', () => {
  it('splits a directory group into its folder name', () => {
    expect(selectedWorkspacePaths([], [{ kind: 'folder', promptText: '/Users/roylin/code/a3s/compat/e2b/' }])).toEqual([
      '/Users/roylin/code/a3s/compat/e2b/'
    ])
    expect(selectedWorkspacePaths([], [{ kind: 'folder', promptText: '/work/notes.md\n/work/readme.md' }])).toEqual([
      '/work/notes.md',
      '/work/readme.md'
    ])
  })
})

describe('WorkbenchWorkspaceFilePicker', () => {
  const listDirectoryEntries = vi.fn()

  it('keeps the picker label stable and shows the chosen directory path on hover', () => {
    render(
      <WorkbenchWorkspaceFilePicker
        root="/Users/roylin/code/a3s"
        selectedPaths={['/Users/roylin/code/a3s/compat/e2b/']}
        onCommit={vi.fn()}
      />
    )

    const trigger = screen.getByRole('button', { name: '工作区文件' })
    expect(trigger).toHaveTextContent('工作区文件')
    expect(trigger.querySelector('[data-workspace-file-icon="directory"]')).toBeInTheDocument()
    expect(trigger).not.toHaveTextContent('/Users/roylin/code/a3s')
    expect(trigger).toHaveAttribute('title', '/Users/roylin/code/a3s/compat/e2b')
  })

  it('keeps multiple selected paths on hover without repeating the input tokens', () => {
    render(
      <WorkbenchWorkspaceFilePicker
        root="/work/alpha"
        selectedPaths={['/work/alpha/notes.md', '/work/alpha/readme.md']}
        onCommit={vi.fn()}
      />
    )

    const trigger = screen.getByRole('button', { name: '工作区文件' })
    expect(trigger).toHaveTextContent('工作区文件')
    expect(trigger).toHaveAttribute('title', '/work/alpha/notes.md\n/work/alpha/readme.md')
    expect(trigger.querySelector('[data-workspace-file-icon="markdown"]')).toBeInTheDocument()
  })

  beforeEach(() => {
    listDirectoryEntries.mockReset()
    listDirectoryEntries.mockResolvedValue([])
    Object.assign(window, { api: { file: { listDirectoryEntries } } })
  })

  it('keeps the panel open until the selection is added once', async () => {
    vi.useFakeTimers()
    try {
      listDirectoryEntries.mockResolvedValue([
        { path: '/work/alpha/notes.md', isDirectory: false },
        { path: '/work/alpha/readme.md', isDirectory: false }
      ])
      const onCommit = vi.fn()
      render(<WorkbenchWorkspaceFilePicker root="/work/alpha" selectedPaths={[]} onCommit={onCommit} />)

      fireEvent.click(screen.getByRole('button', { name: '工作区文件' }))
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200)
      })
      expect(listDirectoryEntries).toHaveBeenCalledWith(
        '/work/alpha',
        expect.objectContaining({ recursive: false, searchPattern: '.' })
      )

      fireEvent.change(screen.getByPlaceholderText('搜索文件'), { target: { value: 'note' } })
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200)
      })
      expect(listDirectoryEntries).toHaveBeenLastCalledWith(
        '/work/alpha',
        expect.objectContaining({ recursive: true, searchPattern: 'note' })
      )

      const notes = screen.getByRole('option', { name: /notes\.md/ })
      fireEvent.click(notes)
      fireEvent.click(screen.getByRole('option', { name: /readme\.md/ }))
      expect(notes).toHaveClass('bg-[#f2f3f5]')
      expect(onCommit).not.toHaveBeenCalled()
      expect(screen.getByPlaceholderText('搜索文件')).toBeInTheDocument()
      const panel = document.querySelector('[data-selector-shell-content]')
      expect(panel).toHaveAttribute('style', expect.stringContaining('max-height'))

      fireEvent.click(screen.getByRole('button', { name: '添加 2 项' }))
      expect(onCommit).toHaveBeenCalledWith([
        { path: '/work/alpha/notes.md', isDirectory: false },
        { path: '/work/alpha/readme.md', isDirectory: false }
      ])
      expect(screen.queryByPlaceholderText('搜索文件')).not.toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })

  it('expands a directory into nested files', async () => {
    vi.useFakeTimers()
    try {
      listDirectoryEntries.mockImplementation(async (directory: string) => {
        if (directory === '/work/alpha/docs') {
          return [{ path: '/work/alpha/docs/notes.md', isDirectory: false }]
        }
        return [
          { path: '/work/alpha/docs', isDirectory: true },
          { path: '/work/alpha/readme.md', isDirectory: false }
        ]
      })
      render(<WorkbenchWorkspaceFilePicker root="/work/alpha" selectedPaths={[]} onCommit={vi.fn()} />)

      fireEvent.click(screen.getByRole('button', { name: '工作区文件' }))
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0)
      })

      expect(screen.queryByRole('option', { name: /notes\.md/ })).not.toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: '展开 docs' }))
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0)
      })

      expect(listDirectoryEntries).toHaveBeenLastCalledWith(
        '/work/alpha/docs',
        expect.objectContaining({ recursive: false })
      )
      const notes = screen.getByRole('option', { name: /notes\.md/ })
      expect(notes).toHaveAttribute('data-depth', '1')
      expect(screen.getByRole('option', { name: /docs/ })).toHaveAttribute('data-depth', '0')
    } finally {
      vi.useRealTimers()
    }
  })
})
