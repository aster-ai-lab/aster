import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import type { TaskPlanStep } from '../taskPlanProjection'
import { TaskPlanSnapshot } from '../TaskPlanSnapshot'

describe('TaskPlanSnapshot status semantics', () => {
  it('collapses a completed plan to its count while keeping details available', () => {
    const { container } = render(
      <TaskPlanSnapshot steps={[{ id: 'verify', title: 'Verified result', status: 'completed' }]} />
    )
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('aria-expanded', 'false')
    expect(container.querySelector('ol')).not.toBeVisible()
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('Verified result')).toBeVisible()
  })

  it('distinguishes completed, current, pending and failure states with existing theme tokens', () => {
    const states: TaskPlanStep['status'][] = ['completed', 'in_progress', 'pending', 'failed', 'skipped', 'cancelled']
    const tones = [
      'text-success-subtle-foreground',
      'text-warning-subtle-foreground',
      'text-muted-foreground',
      'text-error-subtle-foreground',
      'text-muted-foreground',
      'text-warning-subtle-foreground'
    ]
    const { container } = render(
      <TaskPlanSnapshot steps={states.map((status) => ({ id: status, title: `Step ${status}`, status }))} />
    )
    states.forEach((state, index) => {
      const row = container.querySelector(`[data-task-plan-status="${state}"]`)
      expect(row?.lastElementChild, state).toHaveClass(tones[index])
      expect(row?.querySelector('svg'), state).toHaveClass(tones[index])
      expect(row?.lastElementChild?.textContent, state).toBeTruthy()
      expect(row?.querySelector('svg'), state).toHaveAttribute('aria-hidden', 'true')
    })
    expect(container.querySelector('[data-task-plan-status="in_progress"] svg')).toHaveClass('animate-spin')
  })
})
