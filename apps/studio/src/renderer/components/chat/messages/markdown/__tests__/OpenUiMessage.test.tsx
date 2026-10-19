import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { OpenUiMessage } from '../OpenUiMessage'

afterEach(() => cleanup())

describe('OpenUI message', () => {
  it('paints a closed root before the last line finishes', () => {
    render(<OpenUiMessage source={'root = Card("Budget", [days])\ndays = Slider("days"'} streaming />)
    expect(screen.getByText('Budget')).toBeTruthy()
    expect(document.querySelector('[data-openui="ready"]')).toBeTruthy()
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('puts a checkbox on the same control line as a neighboring field', () => {
    render(
      <OpenUiMessage
        source={
          'root = Grid(2, [path, dry])\npath = Input("path", "Path", "", "text")\ndry = Checkbox("dry", "Preview", true)'
        }
        streaming={false}
      />
    )
    const box = screen.getByRole('checkbox', { name: 'Preview' })
    const row = box.closest('label')
    expect(row).toHaveTextContent('Preview')
    expect(row).toHaveClass('h-9')
    expect(row?.parentElement).toHaveClass('justify-end')
    expect(screen.getByLabelText('Path')).toHaveClass('h-9')
    expect(box).toHaveAttribute('aria-checked', 'true')
  })

  it('keeps an unknown component as source', () => {
    const source = 'root = Widget("x")'
    render(<OpenUiMessage source={source} streaming={false} />)
    expect(document.querySelector('[data-openui="source"]')).toHaveTextContent(source)
  })
})
