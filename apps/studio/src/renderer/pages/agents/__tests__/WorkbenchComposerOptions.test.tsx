import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useMemo } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  ComposerToolRuntimeProvider,
  useComposerToolDispatch,
  useComposerToolState
} from '@renderer/components/composer/ComposerToolRuntime'
import { AttachmentToolRuntime } from '@renderer/components/composer/tools/components/AttachmentButton'
import type { ToolLauncherApi } from '@renderer/components/composer/tools/types'
import { QuickPanelProvider } from '@renderer/components/QuickPanel'
import { FILE_TYPE, type FileMetadata } from '@renderer/types/file'

import { installSyncRafMock } from '../../../../../tests/__mocks__/requestAnimationFrame'
import type { HomeComposerFlags } from '../workbenchHomeCapabilities'
import { WorkbenchComposerOptions } from '../WorkbenchHomeComposerExtras'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

const closed: HomeComposerFlags = { reviewMode: false, artifactTrace: false, researchMode: false }
const extensions = ['.xlsx', '.pdf']
const selected: FileMetadata = {
  id: 'selected',
  path: '/tmp/report.xlsx',
  name: 'report.xlsx',
  origin_name: 'report.xlsx',
  ext: '.xlsx',
  size: 12,
  type: FILE_TYPE.DOCUMENT,
  created_at: '2026-10-18T00:00:00Z',
  count: 1
}
const inputAdapter = {
  getText: vi.fn(() => 'draft'),
  insertText: vi.fn(),
  deleteTriggerRange: vi.fn(),
  focus: vi.fn()
}

function RegisteredAttachment({ onChange }: { onChange: (next: HomeComposerFlags) => void }) {
  const { files } = useComposerToolState()
  const { setFiles, toolsRegistry } = useComposerToolDispatch()
  const launcher = useMemo<ToolLauncherApi>(
    () => ({ registerLaunchers: (entries) => toolsRegistry.registerLaunchers('attachment', entries) }),
    [toolsRegistry]
  )
  return (
    <>
      <AttachmentToolRuntime
        launcher={launcher}
        couldAddImageFile={false}
        extensions={extensions}
        files={files}
        setFiles={setFiles}
        disabled={false}
      />
      <WorkbenchComposerOptions options={closed} onChange={onChange} inputAdapter={inputAdapter} />
      <output data-testid="staged-attachments">{files.map((file) => file.name).join(',')}</output>
    </>
  )
}

describe('WorkbenchComposerOptions', () => {
  let restoreRaf: (() => void) | undefined

  beforeEach(() => {
    vi.clearAllMocks()
    HTMLElement.prototype.hasPointerCapture ??= () => false
    HTMLElement.prototype.setPointerCapture ??= () => undefined
    HTMLElement.prototype.releasePointerCapture ??= () => undefined
    window.api.file.select = vi.fn(async () => [selected])
    restoreRaf = installSyncRafMock()
  })

  afterEach(() => {
    restoreRaf?.()
  })

  it('opens a menu with the auto-review and artifact-lineage switches', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(<WorkbenchComposerOptions options={closed} onChange={onChange} />)

    const trigger = screen.getByRole('button', { name: 'workbench.composer.options' })
    expect(trigger).toHaveAttribute('aria-haspopup', 'dialog')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await user.click(trigger)

    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(trigger).toHaveAttribute('data-open', 'true')
    const menu = document.querySelector('[data-workbench-menu="options"]')
    expect(menu).toHaveAttribute('aria-label', 'workbench.composer.options')
    expect(menu?.querySelector('[data-composer-option-kind="review"]')).toBeTruthy()
    expect(menu?.querySelector('[data-composer-option-kind="artifact"]')).toBeTruthy()
    const upload = screen.getByRole('button', { name: 'chat.input.upload.attachment' })
    expect(upload).toBeDisabled()
    expect(upload).toHaveAttribute('aria-haspopup', 'dialog')
    expect(upload).toHaveAttribute('data-composer-option-kind', 'upload')

    const review = screen.getByRole('switch', { name: 'workbench.composer.review' })
    const trace = screen.getByRole('switch', { name: 'workbench.composer.trace' })
    expect(review).not.toBeChecked()
    expect(trace).not.toBeChecked()
    await user.click(review)
    expect(onChange).toHaveBeenCalledWith({ ...closed, reviewMode: true })
    await user.click(trace)
    expect(onChange).toHaveBeenCalledWith({ ...closed, artifactTrace: true })
  })

  it('opens the registered attachment picker from the plus menu and leaves the switches unchanged', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(
      <QuickPanelProvider>
        <ComposerToolRuntimeProvider actions={{ addNewTopic: vi.fn(), onTextChange: vi.fn() }}>
          <RegisteredAttachment onChange={onChange} />
        </ComposerToolRuntimeProvider>
      </QuickPanelProvider>
    )

    await user.click(screen.getByRole('button', { name: 'workbench.composer.options' }))
    const upload = await screen.findByRole('button', { name: /chat\.input\.upload\.attachment/ })
    expect(upload).toBeEnabled()
    expect(upload).toHaveAttribute('title', 'chat.input.upload.image_not_supported')
    await user.click(upload)

    await waitFor(() => expect(screen.getByTestId('staged-attachments')).toHaveTextContent('report.xlsx'))
    expect(window.api.file.select).toHaveBeenCalledExactlyOnceWith({
      properties: ['openFile', 'multiSelections'],
      filters: [{ name: 'Files', extensions: ['xlsx', 'pdf'] }]
    })
    expect(screen.queryByRole('button', { name: /chat\.input\.upload\.attachment/ })).not.toBeInTheDocument()
    expect(inputAdapter.focus).toHaveBeenCalledTimes(1)
    expect(inputAdapter.insertText).not.toHaveBeenCalled()
    expect(onChange).not.toHaveBeenCalled()
  })
})
