/** How long a compile has been running. Zero stays "under a second" so the UI never shows a percent. */
export type CompileElapsed = { kind: 'under' } | { kind: 'seconds'; count: number } | { kind: 'clock'; time: string }

export function compileElapsed(seconds: number): CompileElapsed {
  const whole = Math.max(0, Math.floor(seconds))
  if (whole <= 0) return { kind: 'under' }
  if (whole < 60) return { kind: 'seconds', count: whole }
  const minutes = Math.floor(whole / 60)
  const rest = whole % 60
  return { kind: 'clock', time: `${minutes}:${String(rest).padStart(2, '0')}` }
}
