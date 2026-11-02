// @vitest-environment happy-dom
import '@testing-library/jest-dom/vitest'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import inputClasses from '../../arc/input/input.module.css'
import buttonClasses from '../../arc/button/button.module.css'
import badgeClasses from '../../arc/badge/badge.module.css'
import alertClasses from '../../arc/alert/alert.module.css'
import selectClasses from '../../arc/select/select.module.css'
import tabsClasses from '../../arc/tabs/tabs.module.css'
import avatarClasses from '../../arc/avatar/avatar.module.css'
import checkboxClasses from '../../arc/checkbox/checkbox.module.css'
import textareaClasses from '../../arc/textarea/textarea.module.css'
import tooltipClasses from '../../arc/tooltip/tooltip.module.css'
import popoverClasses from '../../arc/popover/popover.module.css'
import dialogClasses from '../../arc/dialog/dialog.module.css'
import { Tooltip } from '../tooltip'
import { Popover, PopoverContent, PopoverTrigger } from '../popover'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '../dialog'
import { InputNumber } from '../input-number'
import { Checkbox } from '../checkbox'
import { Button } from '../button'
import { Badge } from '../badge'
import { Alert } from '../alert'
import { Avatar, AvatarFallback } from '../avatar'
import { Select, SelectTrigger, SelectValue } from '../select'
import { Tabs, TabsList, TabsTrigger } from '../tabs'
import { Input as Textarea } from '../textarea'
import { Input } from '../input'
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupText } from '../input-group'

afterEach(() => {
  document.head.querySelector('[data-arc-test]')?.remove()
  document.documentElement.style.removeProperty('--foreground')
  document.documentElement.style.removeProperty('--background')
  document.documentElement.style.removeProperty('--popover')
  document.documentElement.style.removeProperty('--accent')
  document.documentElement.style.removeProperty('--accent-foreground')
  document.documentElement.classList.remove('dark')
})

function loadStyles() {
  const localRoot = resolve('src/components/arc')
  const arcRoot = existsSync(localRoot) ? localRoot : resolve('packages/ui/src/components/arc')
  const foundation = readFileSync(resolve(arcRoot, 'foundation.css'), 'utf8')
  const modules = [
    ['input/input.module.css', inputClasses],
    ['button/button.module.css', buttonClasses],
    ['badge/badge.module.css', badgeClasses],
    ['alert/alert.module.css', alertClasses],
    ['select/select.module.css', selectClasses],
    ['tabs/tabs.module.css', tabsClasses],
    ['avatar/avatar.module.css', avatarClasses],
    ['checkbox/checkbox.module.css', checkboxClasses],
    ['textarea/textarea.module.css', textareaClasses],
    ['tooltip/tooltip.module.css', tooltipClasses],
    ['popover/popover.module.css', popoverClasses],
    ['dialog/dialog.module.css', dialogClasses]
  ] as const
  // happy-dom does not yet apply @layer; unwrap the outer layer to check declarations.
  const css = modules
    .map(([file, classes]) => {
      const globals: string[] = []
      return readFileSync(resolve(arcRoot, file), 'utf8')
        .replace(/^@layer components \{\n([\s\S]*)\n\}\n$/, '$1')
        .replace(/@layer components \{\s*(\.surface \{[^}]*\})\s*\}/g, '$1')
        .replace(/:global\(([^()]+)\)/g, (_match, selector: string) => {
          globals.push(selector)
          return `__GLOBAL_${globals.length - 1}__`
        })
        .replace(/\.([a-zA-Z_][\w-]*)/g, (match, name: string) =>
          classes[name] ? `.${classes[name]}` : match
        )
        .replace(/__GLOBAL_(\d+)__/g, (_match, index: string) => globals[Number(index)])
    })
    .join('\n')
  const style = document.createElement('style')
  style.dataset.arcTest = ''
  style.textContent =
    ':root { --foreground: #111111; --background: #ffffff; --destructive: #bb2222; }' + foundation + css
  document.head.appendChild(style)
}

describe('Arc input styles', () => {
  it('lets a grouped field fit its addon inside the group', () => {
    loadStyles()
    render(
      <InputGroup>
        <InputGroupInput aria-label="Amount" />
        <InputGroupAddon align="inline-end"><InputGroupText>%</InputGroupText></InputGroupAddon>
      </InputGroup>
    )
    const input = screen.getByRole('textbox', { name: 'Amount' })
    const style = getComputedStyle(input)
    expect(style.height).toBe('100%')
    expect(style.minHeight).toBe('0')
    expect(style.backgroundColor).toBe('transparent')
  })
  it('uses the shared control radius', () => {
    loadStyles()
    render(<Input aria-label="Name" />)
    expect(getComputedStyle(screen.getByRole('textbox', { name: 'Name' })).borderRadius).toBe('8px')
  })
})

describe('Arc controls', () => {
  it('keeps the checkbox square sized and shows the checked state', () => {
    loadStyles()
    render(<Checkbox defaultChecked aria-label="Accept" />)
    const checkbox = screen.getByRole('checkbox', { name: 'Accept' })
    expect(getComputedStyle(checkbox).width).toBe('16px')
    expect(getComputedStyle(checkbox).height).toBe('16px')
    expect(checkbox).toBeChecked()
  })
  it('grows the textarea to fit its content', () => {
    loadStyles()
    render(<Textarea aria-label="Answer" />)
    const field = screen.getByRole('textbox', { name: 'Answer' })
    expect(getComputedStyle(field).getPropertyValue('field-sizing')).toBe('content')
  })
})

describe('Arc select and avatar styles', () => {
  it('sizes a small select to the shared 36px control height', () => {
    loadStyles()
    render(<Select value="one"><SelectTrigger aria-label="Option" size="sm"><SelectValue /></SelectTrigger></Select>)
    expect(getComputedStyle(screen.getByRole('combobox', { name: 'Option' })).height).toBe('36px')
  })
  it('keeps compact tabs constrained by their container', () => {
    loadStyles()
    render(<Tabs defaultValue="one"><TabsList className="h-8"><TabsTrigger value="one">One</TabsTrigger></TabsList></Tabs>)
    const style = getComputedStyle(screen.getByRole('tab', { name: 'One' }))
    expect(style.height).toBe('36px')
    expect(style.maxHeight).toBe('100%')
    expect(style.minHeight).not.toBe('36px')
  })
  it('keeps the avatar default size', () => {
    loadStyles()
    render(<Avatar data-testid="avatar"><AvatarFallback>A</AvatarFallback></Avatar>)
    const style = getComputedStyle(screen.getByTestId('avatar'))
    expect(style.width).toBe('32px')
    expect(style.height).toBe('32px')
  })
})

describe('Arc theme styles', () => {
  it('uses the shared 8px radius on dialog panels', () => {
    loadStyles()
    render(<Dialog open><DialogContent><DialogTitle>Title</DialogTitle><DialogDescription>Details</DialogDescription></DialogContent></Dialog>)
    expect(getComputedStyle(screen.getByRole('dialog')).borderRadius).toBe('8px')
  })
  it('keeps host accent colors available inside Arc controls', () => {
    loadStyles()
    document.documentElement.style.setProperty('--accent', '#2c2c2c')
    document.documentElement.style.setProperty('--accent-foreground', '#f5f5f5')
    render(<Button variant="ghost" style={{ backgroundColor: 'var(--accent)', color: 'var(--accent-foreground)' }}>Option</Button>)
    const style = getComputedStyle(screen.getByRole('button', { name: 'Option' }))
    expect(style.backgroundColor).toBe('#2c2c2c')
    expect(style.color).toBe('#f5f5f5')
  })
  it('visually marks an invalid number field', () => {
    loadStyles()
    render(<InputNumber aria-label="Amount" aria-invalid value={1} />)
    expect(getComputedStyle(screen.getByLabelText('Amount')).borderColor).toBe('#bb2222')
  })
  it('uses the opaque popover surface even when the page background is translucent', () => {
    loadStyles()
    document.documentElement.style.setProperty('--background', 'rgba(0, 0, 0, 0.55)')
    document.documentElement.style.setProperty('--popover', '#242424')
    render(<Popover open><PopoverTrigger>Open</PopoverTrigger><PopoverContent>Details</PopoverContent></Popover>)
    expect(getComputedStyle(screen.getByRole('dialog')).backgroundColor).toBe('#242424')
  })
  it('marks grammar and spelling errors on input controls', () => {
    loadStyles()
    render(<><Input aria-label="Name" aria-invalid="grammar" /><Textarea aria-label="Answer" aria-invalid="spelling" /></>)
    expect(getComputedStyle(screen.getByRole('textbox', { name: 'Name' })).borderColor).toBe('#bb2222')
    expect(getComputedStyle(screen.getByRole('textbox', { name: 'Answer' })).borderColor).toBe('#bb2222')
  })
  it('gives an icon-free error alert a visible danger border', () => {
    loadStyles()
    render(<Alert type="error" message="Could not save" />)
    expect(getComputedStyle(screen.getByRole('alert')).borderColor).toBe('#bb2222')
  })
  it('keeps outline badges transparent with the shared radius', () => {
    loadStyles()
    render(<Badge variant="outline" data-testid="status">Active</Badge>)
    const style = getComputedStyle(screen.getByTestId('status'))
    expect(style.backgroundColor).toBe('transparent')
    expect(style.borderRadius).toBe('8px')
  })
  it('adjusts badge tints for the app dark theme', () => {
    document.documentElement.classList.add('dark')
    loadStyles()
    render(<Badge data-testid="status">Active</Badge>)
    const badge = screen.getByTestId('status')
    expect(getComputedStyle(badge).getPropertyValue('--badge-tint').trim()).toBe('12%')
  })
  it('keeps tooltips legible when the theme changes', () => {
    loadStyles()
    document.documentElement.style.setProperty('--foreground', '#111111')
    document.documentElement.style.setProperty('--background', '#ffffff')
    render(
      <Tooltip content="Theme tip" isOpen>
        <button>Trigger</button>
      </Tooltip>
    )
    const tip = document.querySelector('[data-slot="tooltip-content"]') as HTMLElement
    expect(getComputedStyle(tip).backgroundColor).toBe('#111111')
    expect(getComputedStyle(tip).color).toBe('#ffffff')
    document.documentElement.style.setProperty('--foreground', '#ffffff')
    document.documentElement.style.setProperty('--background', '#111111')
    expect(getComputedStyle(tip).backgroundColor).toBe('#ffffff')
    expect(getComputedStyle(tip).color).toBe('#111111')
  })
})


describe('Arc button icon styles', () => {
  it('visually mutes a focusable disabled action', () => {
    loadStyles()
    render(<Button aria-disabled>Unavailable</Button>)
    expect(Number(getComputedStyle(screen.getByRole('button', { name: 'Unavailable' })).opacity)).toBe(0.5)
  })
  it('keeps toolbar icon buttons and their loading indicator compact', () => {
    loadStyles()
    render(<Button size="icon-navbar" loading aria-label="Refresh"><svg /></Button>)
    const button = screen.getByRole('button', { name: 'Refresh' })
    const indicator = button.querySelector('[data-slot="button-pending"]') as HTMLElement
    expect(getComputedStyle(button).width).toBe('30px')
    expect(getComputedStyle(button).height).toBe('30px')
    expect(getComputedStyle(indicator).width).toBe('18px')
  })
  it('sizes ordinary icons to 16px and keeps explicitly sized icons', () => {
    loadStyles()
    render(
      <>
        <Button><svg data-testid="default-icon" /></Button>
        <Button><svg data-testid="custom-icon" className="lucide-custom" style={{ width: 12, height: 12 }} /></Button>
      </>
    )
    expect(getComputedStyle(screen.getByTestId('default-icon')).width).toBe('16px')
    expect(getComputedStyle(screen.getByTestId('custom-icon')).width).toBe('12px')
  })
})
