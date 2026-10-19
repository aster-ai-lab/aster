import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { CloudModelPicker } from '../CloudModelPicker'

describe('CloudModelPicker', () => {
  it('fetches gateway models and renders selectable options', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          data: [{ id: 'gpt-4o' }, { id: 'claude-sonnet' }, { id: 'gemini-pro' }]
        })
    })
    render(<CloudModelPicker gatewayOrigin="http://gw.test" value={undefined} onChange={vi.fn()} />)
    fireEvent.click(await screen.findByRole('combobox'))
    expect(screen.getAllByRole('option').length).toBeGreaterThanOrEqual(4)
    expect(screen.getByRole('option', { name: 'gemini-pro' })).toBeInTheDocument()
  })

  it('renders nothing when the gateway returns no models', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ data: [] })
    })
    render(<CloudModelPicker gatewayOrigin="http://gw.test" value={undefined} onChange={vi.fn()} />)
    await waitFor(() => expect(global.fetch).toHaveBeenCalled())
    expect(screen.queryByRole('combobox')).toBeNull()
  })

  it('shows the selected value', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ data: [{ id: 'gpt-4o' }] })
    })
    render(<CloudModelPicker gatewayOrigin="http://gw.test" value="gpt-4o" onChange={vi.fn()} />)
    expect(await screen.findByRole('combobox')).toHaveTextContent('gpt-4o')
  })

  it('calls onChange with the selected alias', async () => {
    const onChange = vi.fn()
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ data: [{ id: 'gpt-4o' }, { id: 'claude' }] })
    })
    render(<CloudModelPicker gatewayOrigin="http://gw.test" value={undefined} onChange={onChange} />)
    fireEvent.click(await screen.findByRole('combobox'))
    fireEvent.click(screen.getByRole('option', { name: 'claude' }))
    expect(onChange).toHaveBeenCalledWith('claude')
  })
})
