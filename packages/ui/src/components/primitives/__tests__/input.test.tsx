// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { useRef, useState } from 'react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { Input } from '../input'

describe('Input', () => {
  it('can be reached and edited by keyboard', async () => {
    const user = userEvent.setup()
    render(<Input aria-label="Name" defaultValue="" />)
    const input = screen.getByRole('textbox', { name: 'Name' })
    await user.tab()
    expect(input).toHaveFocus()
    await user.keyboard('Aster')
    expect(input).toHaveValue('Aster')
  })

  it('clears an uncontrolled field, reports a real change and follows form reset', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn((event: React.ChangeEvent<HTMLInputElement>) => ({
      value: event.currentTarget.value,
      target: event.target
    }))
    const onClear = vi.fn()
    render(<form><Input aria-label="Name" defaultValue="Aster" allowClear onChange={onChange} onClear={onClear} /><button type="reset">Reset</button></form>)
    const field = screen.getByRole('textbox', { name: 'Name' })
    await user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(field).toHaveValue('')
    expect(onChange).toHaveReturnedWith({ value: '', target: field })
    expect(onClear).toHaveBeenCalledOnce()
    expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Reset' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Clear' })).toBeInTheDocument())
    expect(field).toHaveValue('Aster')
  })

  it('shows the clear control after typing into an uncontrolled field', async () => {
    const user = userEvent.setup()
    render(<Input aria-label="Name" allowClear />)
    const field = screen.getByRole('textbox', { name: 'Name' })
    expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
    await user.type(field, 'Aster')
    await user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(field).toHaveValue('')
  })

  it('clears a controlled value and forwards the input ref', async () => {
    const user = userEvent.setup()
    function Controlled() {
      const [value, setValue] = useState('Aster')
      const ref = useRef<HTMLInputElement>(null)
      return <><Input ref={ref} aria-label="Name" value={value} allowClear onChange={(event) => setValue(event.currentTarget.value)} /><button onClick={() => ref.current?.focus()}>Focus</button></>
    }
    render(<Controlled />)
    await user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('')
    await user.click(screen.getByRole('button', { name: 'Focus' }))
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveFocus()
  })

  it.each([{ disabled: true }, { readOnly: true }])('does not clear a disabled or read-only value', async (state) => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const onClear = vi.fn()
    render(<Input aria-label="Name" value="Aster" allowClear onChange={onChange} onClear={onClear} {...state} />)
    const clear = screen.getByRole('button', { name: 'Clear' })
    expect(clear).toBeDisabled()
    await user.click(clear)
    expect(onChange).not.toHaveBeenCalled()
    expect(onClear).not.toHaveBeenCalled()
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Aster')
  })
})
