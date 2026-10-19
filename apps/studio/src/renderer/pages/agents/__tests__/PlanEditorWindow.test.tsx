import { fireEvent, render, screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { PlanEditorWindow } from '../PlanEditorWindow'
import { PLAN_DRAFTS_KEY } from '../planModel'
import { WorkbenchAppletWindow } from '../WorkbenchAppletWindow'

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

describe('PlanEditorWindow', () => {
  beforeEach(() => {
    localStorage.removeItem(PLAN_DRAFTS_KEY)
    if (!HTMLElement.prototype.hasPointerCapture) HTMLElement.prototype.hasPointerCapture = () => false
    if (!HTMLElement.prototype.releasePointerCapture) HTMLElement.prototype.releasePointerCapture = () => {}
    if (!HTMLElement.prototype.setPointerCapture) HTMLElement.prototype.setPointerCapture = () => {}
  })

  it('opens a template in the applet window and projects the cover from the title field', () => {
    render(<WorkbenchAppletWindow appletId="plan" />)
    expect(document.querySelector('[data-applet="plan"]')).toHaveAttribute('data-presentation', 'window')
    expect(document.querySelectorAll('[data-ui="workbench.plan.template"]')).toHaveLength(16)

    fireEvent.click(screen.getByRole('radio', { name: 'workbench.plan.category.academic' }))
    expect(document.querySelectorAll('[data-ui="workbench.plan.template"]')).toHaveLength(2)
    expect(document.querySelector('[data-template="research"]')).toBeTruthy()

    fireEvent.click(screen.getByRole('radio', { name: 'workbench.plan.category.all' }))
    fireEvent.click(document.querySelector('[data-template="guide"]')!)

    expect(document.querySelector('[data-ui="workbench.plan.editor"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.plan.cover.title"]')).toHaveTextContent(
      'workbench.plan.template.guide.title'
    )
    const title = document.querySelector('[data-ui="workbench.plan.field.title"]') as HTMLInputElement
    fireEvent.change(title, { target: { value: '立项说明' } })
    expect(document.querySelector('[data-ui="workbench.plan.cover.title"]')).toHaveTextContent('立项说明')
    expect(document.querySelector('[data-ui="workbench.plan.cover"]')).toHaveAttribute('data-cover', 'tech')
    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.cover.brief' }))
    expect(document.querySelector('[data-ui="workbench.plan.cover"]')).toHaveAttribute('data-cover', 'brief')
    expect(document.querySelector('[data-ui="workbench.plan.cover"]')?.textContent).not.toContain(
      'workbench.plan.footer.author'
    )
    expect(screen.getByRole('button', { name: 'workbench.plan.toolbar.h1' })).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.plan.source"] > div')?.className).toContain('absolute')
    expect(screen.getByRole('tab', { name: 'workbench.plan.panel.other' })).toBeTruthy()
    expect((document.querySelector('[data-ui="workbench.plan.field.number"]') as HTMLInputElement).value).toBe(
      'PLAN-GUIDE'
    )

    fireEvent.click(screen.getByRole('radio', { name: 'workbench.plan.view.preview' }))
    expect(document.querySelector('[data-ui="workbench.plan.source"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.plan.preview"]')).toBeTruthy()
    expect(screen.queryByRole('tab', { name: 'workbench.plan.panel.cover' })).toBeNull()

    fireEvent.click(screen.getByRole('radio', { name: 'workbench.plan.view.edit' }))
    expect(document.querySelector('[data-ui="workbench.plan.preview"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.plan.source"]')).toBeTruthy()
    expect(screen.getByRole('tab', { name: 'workbench.plan.panel.cover' })).toBeTruthy()

    fireEvent.click(screen.getByRole('radio', { name: 'workbench.plan.view.split' }))
    expect(document.querySelector('[data-ui="workbench.plan.preview"]')).toBeTruthy()

    const stored = JSON.parse(localStorage.getItem(PLAN_DRAFTS_KEY) ?? '[]') as { meta: { title: string } }[]
    expect(stored[0]?.meta.title).toBe('立项说明')
  })

  it('continues a local draft and clears it only after confirmation', () => {
    render(<PlanEditorWindow />)
    fireEvent.click(document.querySelector('[data-template="blank"]')!)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.file' }))
    const fileMenu = screen.getByRole('menu')
    expect(fileMenu.parentElement).toBe(document.body)
    expect(fileMenu).toHaveStyle({ position: 'fixed' })
    fireEvent.click(screen.getByRole('menuitem', { name: 'workbench.plan.file.new' }))
    expect(document.querySelector('[data-ui="workbench.plan.gallery"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.plan.draft"]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.gallery.mine' }))
    expect(document.querySelector('[data-ui="workbench.plan.draft"]')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.drafts.continue' }))
    expect(document.querySelector('[data-ui="workbench.plan.editor"]')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.back' }))
    expect(document.querySelector('[data-ui="workbench.plan.gallery"]')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.gallery.mine' }))
    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.drafts.clear' }))
    const dialog = screen.getByRole('dialog', { name: 'workbench.plan.drafts.clear_title' })
    expect(within(dialog).getByText('workbench.plan.drafts.clear_title')).toBeTruthy()
    fireEvent.click(within(dialog).getByRole('button', { name: 'workbench.plan.drafts.clear' }))
    expect(document.querySelector('[data-ui="workbench.plan.drafts.empty"]')).toBeTruthy()
    expect(localStorage.getItem(PLAN_DRAFTS_KEY)).toBe('[]')
  })

  it('opens the assistant, themes, and export instead of a closed toast', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    render(<PlanEditorWindow />)
    fireEvent.click(document.querySelector('[data-template="blank"]')!)

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.ai' }))
    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.ai.run' }))
    expect(screen.getByText('workbench.plan.ai.empty')).toBeTruthy()
    expect(fetchSpy).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.themes' }))
    const themes = screen.getByRole('dialog', { name: 'workbench.plan.themes' })
    fireEvent.click(within(themes).getByRole('button', { name: 'workbench.plan.cover.brief' }))
    expect(document.querySelector('[data-ui="workbench.plan.cover"]')).toHaveAttribute('data-cover', 'brief')
    const heading = document.querySelector('[data-plan-heading]') as HTMLElement
    expect(heading.className).toContain('border-l-4')
    expect(heading.style.borderColor).toBe('rgb(31, 61, 50)')

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.save' }))
    expect(screen.getByRole('button', { name: 'workbench.plan.saved' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.export' }))
    const exportMenu = screen.getByRole('menu')
    expect(exportMenu.parentElement).toBe(document.body)
    expect(screen.getByRole('menuitem', { name: 'workbench.plan.export.markdown' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'workbench.plan.export.html' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'workbench.plan.export.word' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'workbench.plan.export.print' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.plan.file' }))
    expect(screen.getByRole('menuitem', { name: 'workbench.plan.file.open' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'workbench.plan.toolbar.table' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'workbench.plan.toolbar.link' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'workbench.plan.toolbar.break' })).toBeTruthy()
    fetchSpy.mockRestore()
  })
})
