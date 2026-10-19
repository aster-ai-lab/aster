import { fireEvent, render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type * as HomeSlashCatalog from '../../agents/homeSlashCatalog'

const surface = vi.hoisted(() => ({
  props: null as null | Record<string, unknown>
}))

vi.mock('@renderer/components/composer/ComposerSurface', async () => {
  const { useComposerToolProviderLaunchers } = await import('@renderer/components/composer/tools/ComposerToolProvider')
  const { useQuickPanel } = await import('@renderer/components/QuickPanel')
  return {
    default: (props: Record<string, unknown>) => {
      // The real surface reads these as soon as its runtime loads. Missing either
      // provider is the route error "useComposerToolProviderLaunchers must be used
      // within ComposerToolProvider".
      useComposerToolProviderLaunchers()
      useQuickPanel()
      surface.props = props
      const renderLeftControls = props.renderLeftControls as (() => ReactNode) | undefined
      return (
        <div data-ui="composer.surface">
          <textarea
            aria-label={String(props.placeholder)}
            disabled={props.editable === false}
            value={String(props.text ?? '')}
            onChange={(event) => (props.onTextChange as (value: string) => void)(event.target.value)}
          />
          {renderLeftControls?.()}
        </div>
      )
    }
  }
})

vi.mock('@renderer/pages/agents/homeSlashCatalog', async () => {
  const actual = await vi.importActual<typeof HomeSlashCatalog>('../../agents/homeSlashCatalog')
  return { ...actual, useHomeLocalSkills: () => [] }
})

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string, fallback?: string) => fallback ?? key })
}))

import { TaskPromptComposer } from '../TaskPromptComposer'

describe('TaskPromptComposer', () => {
  beforeEach(() => {
    surface.props = null
  })

  it('reuses the work composer to collect the task without a send button', () => {
    const onPromptChange = vi.fn()
    render(
      <TaskPromptComposer
        prompt=""
        label="提示词"
        toolbar={<button type="button">模型</button>}
        onPromptChange={onPromptChange}
      />
    )

    const field = document.querySelector('[data-ui~="task.prompt.composer"]')
    expect(field).toHaveAttribute('aria-label', '提示词')
    expect(field?.getAttribute('data-ui')).toBe('task.prompt.composer workbench.composer')
    expect(surface.props).toMatchObject({
      hideSend: true,
      sendDisabled: true,
      lockEditorHeight: true,
      editorMinHeight: 96,
      fontSize: 13,
      rootPanelCatalogOnly: true,
      placeholder: 'agent.tasks.prompt.placeholder',
      newlineShortcut: ['Enter'],
      sendMessageShortcut: ['Alt', 'Enter'],
      quickPanelEnabled: true,
      editable: true
    })
    const items = surface.props?.rootPanelAdditionalItems as { label: string }[]
    const labels = items.map((item) => item.label)
    expect(labels).not.toContain('/goal')
    expect(labels).toEqual(expect.arrayContaining(['/help', '/tools', '/mcp']))
    expect(labels).not.toContain('创建技能')
    expect(labels).not.toContain('创建定时任务')
    const highlights = surface.props?.textHighlights as {
      pattern: RegExp
      attributes?: Record<string, string>
    }[]
    const command = highlights.find((item) => item.attributes?.['data-composer-highlight'] === 'command')
    expect([...'/goal /help'.matchAll(command!.pattern)].map((match) => match[0])).toEqual(['/help'])
    expect(screen.getByRole('button', { name: '模型' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'workbench.composer.capture' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'chat.input.send' })).not.toBeInTheDocument()

    fireEvent.change(screen.getByRole('textbox', { name: 'agent.tasks.prompt.placeholder' }), {
      target: { value: '每天汇总' }
    })
    expect(onPromptChange).toHaveBeenCalledWith('每天汇总')
  })

  it('marks the composer invalid and read-only while the form is saving', () => {
    render(<TaskPromptComposer prompt="draft" label="提示词" disabled invalid onPromptChange={vi.fn()} />)
    expect(document.querySelector('[data-ui~="task.prompt.composer"]')).toHaveAttribute('data-invalid', 'true')
    expect(surface.props?.editable).toBe(false)
    expect(screen.getByRole('textbox')).toBeDisabled()
  })
})
