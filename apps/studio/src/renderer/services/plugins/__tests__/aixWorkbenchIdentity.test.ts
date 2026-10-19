import { describe, expect, it } from 'vitest'

import type { AixWorkbenchPackage } from '@shared/types/aixWorkbench'

import {
  aixWorkbenchPackageIdentity,
  encodeAixWorkbenchIdentityRouteKey,
  encodeAixWorkbenchRouteKey,
  parseAixWorkbenchRouteKey
} from '../aixWorkbenchIdentity'

const item: AixWorkbenchPackage = {
  slug: 'review',
  packageId: 'use/team/review',
  generation: 1,
  revision: `sha256:${'a'.repeat(64)}`,
  containers: [],
  views: []
}

const route = (value = item, localId = 'page', kind: 'container' | 'view' = 'view') =>
  encodeAixWorkbenchRouteKey({ package: value, kind, localId })

describe('AIX workbench identity', () => {
  it('encodes package pins and local view names without delimiter collisions', () => {
    const first = route()
    expect(route()).toBe(first)
    expect(route({ ...item, packageId: 'use/team/other' })).not.toBe(first)
    expect(route({ ...item, generation: 2 })).not.toBe(first)
    expect(route({ ...item, revision: `sha256:${'b'.repeat(64)}` })).not.toBe(first)
    expect(route(item, 'page', 'container')).not.toBe(first)
    expect(aixWorkbenchPackageIdentity(item)).not.toBe(aixWorkbenchPackageIdentity({ ...item, generation: 2 }))
  })

  it('rejects malformed and unknown route-key encodings', () => {
    expect(parseAixWorkbenchRouteKey(route())).toBe(route())
    for (const value of [
      null,
      '',
      'aixv2-00000000000000000000000000000000',
      'aixv1-xyz',
      'aixv1-000000000000000000000000000000000'
    ])
      expect(parseAixWorkbenchRouteKey(value)).toBeNull()
  })

  it('separates activation, mounted instance, and operation identities in the shared route namespace', () => {
    const activation = { package: item, scopeId: 'workspace:one', activationId: 'activation-1' }
    const contribution = { package: item, kind: 'view' as const, localId: 'page' }
    const instance = { activation, contribution, instanceId: 'document-1' }
    const routeKey = (identity: Parameters<typeof encodeAixWorkbenchIdentityRouteKey>[0]) =>
      encodeAixWorkbenchIdentityRouteKey(identity)

    const activationKey = routeKey({ type: 'activation', activation })
    const instanceKey = routeKey({ type: 'instance', instance })
    expect(routeKey({ type: 'contribution', ...contribution })).toBe(encodeAixWorkbenchRouteKey(contribution))
    const openKey = routeKey({
      type: 'operation',
      operation: { instance, operation: 'admit', operationId: 'request-1' }
    })
    expect(new Set([activationKey, instanceKey, openKey]).size).toBe(3)
    expect(routeKey({ type: 'activation', activation: { ...activation, activationId: 'activation-2' } })).not.toBe(
      activationKey
    )
    expect(routeKey({ type: 'instance', instance: { ...instance, instanceId: 'document-2' } })).not.toBe(instanceKey)
    expect(
      routeKey({ type: 'operation', operation: { instance, operation: 'cancel', operationId: 'request-1' } })
    ).not.toBe(openKey)
    expect(
      routeKey({ type: 'operation', operation: { instance, operation: 'admit', operationId: 'request-2' } })
    ).not.toBe(openKey)
    expect(() => routeKey({ type: 'activation', activation: { ...activation, scopeId: '' } })).toThrow(
      'Invalid AIX scope identity'
    )
  })
})
