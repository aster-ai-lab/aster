// @vitest-environment happy-dom
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { usePopupPresence } from '../usePopupPresence'

function Probe({ value }: { value: string | null }) {
  const { present, state } = usePopupPresence(value)
  return present === null ? null : <div data-testid="popup" data-state={state}>{present}</div>
}

afterEach(() => { cleanup(); vi.useRealTimers() })

it('retains closing content and cancels its removal when reopened with a new value', () => {
  vi.useFakeTimers()
  const view = render(<Probe value="first model" />)
  view.rerender(<Probe value={null} />)
  expect(screen.getByTestId('popup').textContent).toBe('first model')
  expect(screen.getByTestId('popup').dataset.state).toBe('closed')
  act(() => vi.advanceTimersByTime(100))
  view.rerender(<Probe value="second model" />)
  act(() => vi.advanceTimersByTime(200))
  expect(screen.getByTestId('popup').textContent).toBe('second model')
  expect(screen.getByTestId('popup').dataset.state).toBe('open')
  view.rerender(<Probe value={null} />)
  act(() => vi.advanceTimersByTime(199))
  expect(screen.getByTestId('popup').textContent).toBe('second model')
  act(() => vi.advanceTimersByTime(1))
  expect(screen.queryByTestId('popup')).toBeNull()
})
