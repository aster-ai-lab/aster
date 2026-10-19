import type {
  PluginActivationContext,
  PluginCleanup,
  PluginCloseReport,
  PluginServiceSnapshot
} from '@renderer/types/workbenchPlugin'

const MAX_EFFECTS = 256

export class PluginEffects implements PluginActivationContext {
  private readonly controller = new AbortController()
  private readonly cleanups: PluginCleanup[] = []
  private readonly pendingTasks: ((signal: AbortSignal) => Promise<void>)[] = []
  private readonly tasks = new Set<Promise<void>>()
  private readonly taskFailures: string[] = []
  private suppressedFailures = 0
  private closing?: Promise<PluginCloseReport>
  private started = false
  private activation?: Promise<void>

  constructor(
    readonly scopeId: string,
    readonly services: PluginServiceSnapshot,
    private readonly current: () => boolean,
    private readonly timeoutMs: number
  ) {}

  get signal(): AbortSignal {
    return this.controller.signal
  }

  isCurrent(): boolean {
    return !this.signal.aborted && this.current()
  }

  cancel(): void {
    this.controller.abort()
  }

  async activate(factory?: (context: PluginActivationContext) => void | Promise<void>): Promise<void> {
    if (!factory) return
    let timer: ReturnType<typeof setTimeout> | undefined
    let abort: (() => void) | undefined
    const work = Promise.resolve().then(() => {
      if (this.signal.aborted) throw new Error('Plugin activation cancelled')
      return factory(this)
    })
    this.activation = work.then(
      () => undefined,
      () => undefined
    )
    try {
      await Promise.race([
        work,
        new Promise<never>((_resolve, reject) => {
          abort = () => reject(new Error('Plugin activation cancelled'))
          this.signal.addEventListener('abort', abort, { once: true })
          timer = setTimeout(() => reject(new Error('Plugin activation timed out')), this.timeoutMs)
          if (this.signal.aborted) abort()
        })
      ])
    } finally {
      if (timer !== undefined) clearTimeout(timer)
      if (abort) this.signal.removeEventListener('abort', abort)
    }
  }

  private requireActive(): void {
    if (this.signal.aborted) throw new Error('Plugin scope is closed')
    if (this.cleanups.length + this.pendingTasks.length + this.tasks.size >= MAX_EFFECTS) {
      throw new Error('Plugin effect limit exceeded')
    }
  }

  defer(cleanup: PluginCleanup): void {
    this.requireActive()
    this.cleanups.push(cleanup)
  }

  listen(target: EventTarget, event: string, listener: (event: Event) => void): void {
    this.requireActive()
    const guarded = (value: Event) => {
      if (this.isCurrent()) listener(value)
    }
    this.defer(() => target.removeEventListener(event, guarded))
    target.addEventListener(event, guarded)
  }

  spawn(task: (signal: AbortSignal) => Promise<void>): void {
    this.requireActive()
    if (!this.started) this.pendingTasks.push(task)
    else this.startTask(task)
  }

  start(): void {
    if (this.started || this.signal.aborted) return
    this.started = true
    for (const task of this.pendingTasks.splice(0)) this.startTask(task)
  }

  private startTask(task: (signal: AbortSignal) => Promise<void>): void {
    const running = Promise.resolve()
      .then(() => {
        if (!this.signal.aborted) return task(this.signal)
        return undefined
      })
      .catch((error: unknown) => {
        if (this.taskFailures.length < MAX_EFFECTS) this.taskFailures.push(String(error).slice(0, 1024))
        else this.suppressedFailures++
      })
      .then(() => undefined)
    this.tasks.add(running)
    void running.then(() => this.tasks.delete(running))
  }

  close(deadline = Date.now() + this.timeoutMs): Promise<PluginCloseReport> {
    if (!this.closing) {
      this.controller.abort()
      this.pendingTasks.length = 0
      this.closing = this.cleanup(deadline)
    }
    return this.closing
  }

  private async cleanup(deadline: number): Promise<PluginCloseReport> {
    const failed: string[] = []
    const timedOut: string[] = []
    const settle = async (label: string, work: () => void | Promise<void>) => {
      let timer: ReturnType<typeof setTimeout> | undefined
      try {
        const completed = await Promise.race([
          Promise.resolve()
            .then(work)
            .then(() => true),
          new Promise<false>((resolve) => {
            timer = setTimeout(() => resolve(false), Math.max(0, deadline - Date.now()))
          })
        ])
        if (!completed) timedOut.push(label)
      } catch (error) {
        failed.push(`${label}: ${String(error)}`)
      } finally {
        if (timer !== undefined) clearTimeout(timer)
      }
    }
    await settle('tasks', async () => {
      await Promise.all([...this.tasks, ...(this.activation ? [this.activation] : [])])
    })
    for (let index = this.cleanups.length - 1; index >= 0; index--) {
      await settle(`effect:${index}`, this.cleanups[index])
    }
    this.cleanups.length = 0
    failed.push(...this.taskFailures)
    if (this.suppressedFailures) failed.push(`Additional task failures: ${this.suppressedFailures}`)
    return Object.freeze({ failed: Object.freeze(failed), timedOut: Object.freeze(timedOut) })
  }
}
