// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import Spinner from '../spinner'

afterEach(() => cleanup())

describe('Spinner', () => {
  it('exposes the Aster loading label and uses a reduced-motion-aware indicator', () => {
    const previousTenant = document.documentElement.dataset.tenant
    document.documentElement.dataset.tenant = 'aster'
    try {
      render(<Spinner text="Loading" />)
      const status = screen.getByRole('status')
      expect(status).toHaveTextContent('Loading')
      expect(status).toHaveClass('text-muted-foreground')
      expect(status.querySelector('svg')).toHaveClass('motion-safe:animate-spin')
      expect(status.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    } finally {
      if (previousTenant === undefined) delete document.documentElement.dataset.tenant
      else document.documentElement.dataset.tenant = previousTenant
    }
  })
})
