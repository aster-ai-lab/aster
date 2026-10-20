import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nextProvider } from 'react-i18next'
import { beforeAll, expect, it, vi } from 'vitest'

import i18n from '@renderer/i18n/resolver'

import { TaskPlanReview } from '../TaskPlanReview'

vi.mock('@a3s-lab/ui', async () => import('@a3s-lab/ui/components/primitives/button'))
beforeAll(async () => {
  await i18n.changeLanguage('en-us')
})

it('keeps an unsent revision out of implementation and lets the user return to it', async () => {
  const user = userEvent.setup()
  const start = vi.fn()
  const revise = vi.fn()
  render(
    <I18nextProvider i18n={i18n}>
      <TaskPlanReview disabled={false} hasDraft busy={false} onStart={start} onRevise={revise} />
    </I18nextProvider>
  )
  expect(screen.getByRole('status')).toHaveTextContent('Plan ready for review')
  expect(screen.getByText(/You have unsent changes/)).toBeVisible()
  const implement = screen.getByRole('button', { name: 'Start implementation' })
  expect(implement).toBeDisabled()
  await user.click(implement)
  expect(start).not.toHaveBeenCalled()
  await user.click(screen.getByRole('button', { name: 'Revise plan' }))
  expect(revise).toHaveBeenCalledOnce()
})
