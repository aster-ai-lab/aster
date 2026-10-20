/** A canonical update_plan payload. Unknown states never become completed steps. */
export type TaskPlanStep = {
  id: string
  title: string
  status: 'pending' | 'in_progress' | 'completed' | 'failed' | 'skipped' | 'cancelled'
}

export function taskPlanSteps(input: unknown): TaskPlanStep[] | undefined {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return undefined
  const value = input as Record<string, unknown>
  if (value.variant !== 'TodoWrite' || !Array.isArray(value.plan)) return undefined
  const steps: TaskPlanStep[] = []
  for (const [index, row] of value.plan.entries()) {
    if (!row || typeof row !== 'object' || Array.isArray(row)) return undefined
    const step = row as Record<string, unknown>
    if (typeof step.step !== 'string' || !step.step.trim()) return undefined
    switch (step.status) {
      case 'pending':
      case 'in_progress':
      case 'completed':
      case 'failed':
      case 'skipped':
      case 'cancelled':
        steps.push({
          id: typeof step.id === 'string' && step.id ? step.id : `plan-${index + 1}`,
          title: step.step.trim(),
          status: step.status
        })
        break
      default:
        return undefined
    }
  }
  return steps
}
