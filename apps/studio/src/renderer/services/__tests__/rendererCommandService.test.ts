import { describe, expect, it } from 'vitest'

import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type {
  ExtensionCommandDefinition,
  ExtensionCommandId,
  ExtensionCommandRegistration
} from '@renderer/types/command'
import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'

import { extensionCommandId, RendererCommandService } from '../rendererCommandService'

const id = extensionCommandId('aix.proof.views', 'a'.repeat(64), 'refresh')
const definition = { id, title: 'Refresh', category: 'Proof views' }

function owner() {
  const abort = new globalThis.AbortController()
  return { signal: abort.signal, isCurrent: () => !abort.signal.aborted, abort: () => abort.abort() }
}

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

describe('renderer extension command ownership', () => {
  it('keeps document targets independent and refuses builtin claims or duplicate targets', async () => {
    const service = new RendererCommandService()
    const effects: string[] = []
    const first = { ...definition }
    const secondId = extensionCommandId('aix.proof.views', 'b'.repeat(64), 'refresh')
    const behavior = {
      enabled: () => true,
      execute: () => {
        effects.push('first')
      }
    }
    service.registerCommand(first, owner(), behavior)
    service.registerCommand({ ...definition, id: secondId }, owner(), {
      enabled: () => true,
      execute: () => {
        effects.push('second')
      }
    })
    first.title = 'Mutated title'
    expect(service.resolveCommand(id)?.title).toBe('Refresh')
    expect(() => service.registerCommand(definition, owner(), behavior)).toThrow('Duplicate')
    expect(() =>
      service.registerCommand({ ...definition, id: 'app.settings.open' as ExtensionCommandId }, owner(), behavior)
    ).toThrow('Invalid')
    expect(() => extensionCommandId('aix.proof.views', 'not-a-document', 'refresh')).toThrow('identity')
    expect(await service.resolveCommand(secondId)?.execute()).toBe(true)
    expect(effects).toEqual(['second'])
    expect(await service.resolveCommand(id)?.execute()).toBe(true)
    expect(effects).toEqual(['second', 'first'])
    service.close()
  })

  it('withdraws with the actual plugin scope and never retargets saved actions after ID reuse', async () => {
    const service = new RendererCommandService()
    const host = new WorkbenchPluginHost('command-owner')
    let context!: PluginActivationContext
    await host.setPlugins([
      {
        id: 'proof',
        generation: 1,
        scopes: ['window'],
        contributions: [],
        activate: (value) => {
          context = value
        }
      }
    ])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    const effects: string[] = []
    const registration = service.registerCommand(definition, context, {
      enabled: () => true,
      execute: () => {
        effects.push('old')
      }
    })
    const saved = service.resolveCommand(id)!
    await scope.close()
    expect(service.resolveCommand(id)).toBeUndefined()
    service.registerCommand(definition, owner(), {
      enabled: () => true,
      execute: () => {
        effects.push('new')
      }
    })
    registration.dispose()
    registration.refresh()
    expect(await saved.execute()).toBe(false)
    expect(effects).toEqual([])
    expect(await service.resolveCommand(id)?.execute()).toBe(true)
    expect(effects).toEqual(['new'])
    service.close()
  })

  it('rechecks conditions at dispatch and cancels a pending operation when its owner closes', async () => {
    const service = new RendererCommandService()
    const authority = owner()
    const waiting = deferred()
    let enabled = true
    let executions = 0
    let operationSignal: globalThis.AbortSignal | undefined
    const registration = service.registerCommand(definition, authority, {
      enabled: () => enabled,
      execute: (signal) => {
        executions++
        operationSignal = signal
        return waiting.promise
      }
    })
    const saved = service.resolveCommand(id)!
    enabled = false
    expect(await saved.execute()).toBe(false)
    expect(executions).toBe(0)
    enabled = true
    registration.refresh()
    const running = saved.execute()
    expect(service.resolveCommand(id)?.enabled).toBe(false)
    expect(await saved.execute()).toBe(false)
    expect(executions).toBe(1)
    authority.abort()
    expect(operationSignal?.aborted).toBe(true)
    waiting.resolve()
    expect(await running).toBe(false)
    expect(service.resolveCommand(id)).toBeUndefined()
    service.close()
  })

  it('refuses throwing extension callbacks and makes failed commands available for another attempt', async () => {
    const service = new RendererCommandService()
    let failingCondition = true
    service.registerCommand(definition, owner(), {
      enabled: () => {
        if (failingCondition) throw new Error('condition failed')
        return true
      },
      execute: async () => {
        throw new Error('operation failed')
      }
    })
    expect(service.resolveCommand(id)?.enabled).toBe(false)
    expect(await service.resolveCommand(id)?.execute()).toBe(false)
    failingCondition = false
    expect(await service.resolveCommand(id)?.execute()).toBe(false)
    expect(service.resolveCommand(id)?.enabled).toBe(true)
    service.close()
  })

  it('refuses completion when enablement changes while the operation is pending', async () => {
    const service = new RendererCommandService()
    const waiting = deferred()
    let enabled = true
    service.registerCommand(definition, owner(), {
      enabled: () => enabled,
      execute: () => waiting.promise
    })
    const running = service.resolveCommand(id)!.execute()
    enabled = false
    waiting.resolve()
    expect(await running).toBe(false)
    expect(service.resolveCommand(id)?.enabled).toBe(false)
    service.close()
  })

  it('does not dispatch when a subscriber closes the owner during the busy-state notification', async () => {
    const service = new RendererCommandService()
    const authority = owner()
    let executions = 0
    service.registerCommand(definition, authority, {
      enabled: () => true,
      execute: () => {
        executions++
      }
    })
    const saved = service.resolveCommand(id)!
    const unsubscribe = service.subscribe(() => authority.abort())
    expect(await saved.execute()).toBe(false)
    expect(executions).toBe(0)
    expect(service.resolveCommand(id)).toBeUndefined()
    unsubscribe()
    service.close()
  })

  it('bounds each owner and the whole dispatcher while reclaiming disposed registrations', () => {
    const service = new RendererCommandService()
    const behavior = { enabled: () => true, execute: () => {} }
    const authorities = Array.from({ length: 33 }, () => owner())
    const registrations: ExtensionCommandRegistration[] = []
    for (let instance = 0; instance < 32; instance++) {
      for (let command = 0; command < 32; command++) {
        const commandId = extensionCommandId(
          'aix.proof.views',
          instance.toString(16).padStart(64, '0'),
          `action-${command}`
        )
        registrations.push(service.registerCommand({ id: commandId, title: 'Action' }, authorities[instance], behavior))
      }
    }
    const extra: ExtensionCommandDefinition = {
      id: extensionCommandId('aix.proof.views', 'f'.repeat(64), 'extra'),
      title: 'Extra'
    }
    expect(() => service.registerCommand(extra, authorities[32], behavior)).toThrow('budget')
    registrations[0].dispose()
    expect(() => service.registerCommand(extra, authorities[1], behavior)).toThrow('budget')
    service.registerCommand(extra, authorities[0], behavior)
    authorities[0].abort()
    expect(service.resolveCommand(extra.id)).toBeUndefined()
    service.registerCommand(extra, authorities[32], behavior)
    service.close()
    expect(() => service.registerCommand(definition, owner(), behavior)).toThrow('closed')
  })

  it('checks builtin enablement for direct dispatch as well as keyboard resolution', () => {
    const service = new RendererCommandService()
    let executions = 0
    service.registerHandler('screenshot.capture', () => {
      executions++
    })
    service.setContext({ 'feature.screenshot.enabled': false })
    service.execute('screenshot.capture')
    expect(executions).toBe(0)
    service.setContext({ 'feature.screenshot.enabled': true })
    service.execute('screenshot.capture')
    expect(executions).toBe(1)
    service.close()
    service.execute('screenshot.capture')
    expect(executions).toBe(1)
  })
})
