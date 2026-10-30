// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Tabs, TabsContent, TabsList, TabsTrigger } from '../tabs'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

it.each(['default', 'line', 'underline'] as const)(
  'moves only the selected %s plate after the controlled owner accepts a change',
  async (variant) => {
    const onValueChange = vi.fn()
    const view = (value: string) => (
      <Tabs value={value} variant={variant} onValueChange={onValueChange}>
        <TabsList aria-label="Workspace">
          <TabsTrigger value="files">Files</TabsTrigger>
          <TabsTrigger value="terminal">Terminal</TabsTrigger>
          <TabsTrigger value="disabled" disabled>
            Unavailable
          </TabsTrigger>
        </TabsList>
      </Tabs>
    )
    const { container, rerender } = render(view('files'))
    const files = screen.getByRole('tab', { name: 'Files' })
    const terminal = screen.getByRole('tab', { name: 'Terminal' })
    fireEvent.mouseDown(terminal, { button: 0, ctrlKey: false })
    expect(onValueChange).toHaveBeenCalledWith('terminal')
    expect(files.querySelector('[data-slot="tabs-indicator"]')).not.toBeNull()
    expect(terminal).toHaveAttribute('aria-selected', 'false')
    rerender(view('terminal'))
    expect(terminal).toHaveAttribute('aria-selected', 'true')
    expect(terminal.querySelector('[data-slot="tabs-indicator"]')).not.toBeNull()
    expect(container.querySelectorAll('[data-slot="tabs-indicator"]')).toHaveLength(1)
    terminal.focus()
    await userEvent.keyboard('{ArrowRight}')
    expect(files).toHaveFocus()
  }
)

describe('Tabs workflow variant', () => {
  it('marks the active trigger with bold + underline styles', () => {
    render(
      <Tabs defaultValue="data" variant="workflow">
        <TabsList>
          <TabsTrigger value="data">Data</TabsTrigger>
          <TabsTrigger value="rag">RAG</TabsTrigger>
          <TabsTrigger value="recall">Recall</TabsTrigger>
        </TabsList>
        <TabsContent value="data">data-content</TabsContent>
        <TabsContent value="rag">rag-content</TabsContent>
        <TabsContent value="recall">recall-content</TabsContent>
      </Tabs>
    )

    const dataTrigger = screen.getByRole('tab', { name: 'Data' })
    const ragTrigger = screen.getByRole('tab', { name: 'RAG' })

    expect(dataTrigger).toHaveAttribute('data-state', 'active')
    expect(dataTrigger.className).toContain('data-[state=active]:font-semibold')
    expect(dataTrigger.className).toContain('data-[state=active]:underline')
    expect(ragTrigger).toHaveAttribute('data-state', 'inactive')
  })

  it('switches the active step when another trigger is clicked', async () => {
    const user = userEvent.setup()
    const onValueChange = vi.fn()

    render(
      <Tabs defaultValue="data" variant="workflow" onValueChange={onValueChange}>
        <TabsList>
          <TabsTrigger value="data">Data</TabsTrigger>
          <TabsTrigger value="rag">RAG</TabsTrigger>
        </TabsList>
        <TabsContent value="data">data-content</TabsContent>
        <TabsContent value="rag">rag-content</TabsContent>
      </Tabs>
    )

    await user.click(screen.getByRole('tab', { name: 'RAG' }))

    expect(onValueChange).toHaveBeenCalledWith('rag')
    expect(screen.getByRole('tab', { name: 'RAG' })).toHaveAttribute('data-state', 'active')
  })

  it('applies the chevron separator class to every trigger after the first', () => {
    render(
      <Tabs defaultValue="data" variant="workflow">
        <TabsList>
          <TabsTrigger value="data">Data</TabsTrigger>
          <TabsTrigger value="rag">RAG</TabsTrigger>
          <TabsTrigger value="recall">Recall</TabsTrigger>
        </TabsList>
      </Tabs>
    )

    const triggers = screen.getAllByRole('tab')

    expect(triggers[0].className).toContain("[&:not(:first-child)]:before:content-['›']")
    expect(triggers[1].className).toContain("[&:not(:first-child)]:before:content-['›']")
    expect(triggers[2].className).toContain("[&:not(:first-child)]:before:content-['›']")
  })
})
