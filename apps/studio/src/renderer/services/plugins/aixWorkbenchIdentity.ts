import { v5 as uuidv5 } from 'uuid'

import type { AixWorkbenchPackage } from '@shared/types/aixWorkbench'

const ROUTE_KEY = /^aixv1-[a-f0-9]{32}$/

type AixIdentityPackage = Pick<AixWorkbenchPackage, 'slug' | 'packageId' | 'generation' | 'revision'>
export type AixWorkbenchIdentity = Readonly<AixIdentityPackage>
export type AixWorkbenchContributionIdentity = Readonly<{
  package: AixWorkbenchIdentity
  kind: 'container' | 'view'
  localId: string
}>
export type AixWorkbenchActivationIdentity = Readonly<{
  package: AixWorkbenchIdentity
  scopeId: string
  activationId: string
}>
export type AixWorkbenchInstanceIdentity = Readonly<{
  activation: AixWorkbenchActivationIdentity
  contribution: AixWorkbenchContributionIdentity
  instanceId: string
}>
export type AixWorkbenchOperationIdentity = Readonly<{
  instance: AixWorkbenchInstanceIdentity
  operation: 'admit' | 'cancel'
  operationId: string
}>

export type AixWorkbenchRouteIdentity =
  | ({ readonly type: 'contribution' } & AixWorkbenchContributionIdentity)
  | { readonly type: 'activation'; readonly activation: AixWorkbenchActivationIdentity }
  | { readonly type: 'instance'; readonly instance: AixWorkbenchInstanceIdentity }
  | { readonly type: 'operation'; readonly operation: AixWorkbenchOperationIdentity }

function assertIdentityPart(value: string, name: string): void {
  if (!value || value.length > 512 || /[\u0000-\u001f\u007f]/.test(value)) throw new Error(`Invalid AIX ${name}`)
}

function activationTuple(identity: AixWorkbenchActivationIdentity): readonly unknown[] {
  assertIdentityPart(identity.scopeId, 'scope identity')
  assertIdentityPart(identity.activationId, 'activation identity')
  return ['aix-activation', 1, aixWorkbenchPackageIdentity(identity.package), identity.scopeId, identity.activationId]
}

function contributionTuple(identity: AixWorkbenchContributionIdentity): readonly unknown[] {
  assertIdentityPart(identity.localId, 'contribution identity')
  return [aixWorkbenchPackageIdentity(identity.package), identity.kind, identity.localId]
}

function instanceTuple(identity: AixWorkbenchInstanceIdentity): readonly unknown[] {
  assertIdentityPart(identity.instanceId, 'instance identity')
  return [
    'aix-instance',
    1,
    activationTuple(identity.activation),
    contributionTuple(identity.contribution),
    identity.instanceId
  ]
}

/** Canonical structured package pin; JSON tuple encoding avoids delimiter ambiguity. */
export function aixWorkbenchPackageIdentity(item: AixIdentityPackage): string {
  return JSON.stringify(['aix-package', 1, item.slug, item.packageId, item.generation, item.revision])
}

/** Stable opaque route key scoped to package pin, contribution kind, and local identity. */
export function encodeAixWorkbenchRouteKey(identity: AixWorkbenchContributionIdentity): string {
  assertIdentityPart(identity.localId, 'contribution identity')
  const tuple = [
    'aix-route',
    1,
    identity.package.slug,
    identity.package.packageId,
    identity.package.generation,
    identity.package.revision,
    identity.kind,
    identity.localId
  ]
  return `aixv1-${uuidv5(JSON.stringify(tuple), uuidv5.URL).replaceAll('-', '')}`
}

/** Encode activation, mounted instance, and native operation identities into the same route-key namespace. */
export function encodeAixWorkbenchIdentityRouteKey(identity: AixWorkbenchRouteIdentity): string {
  let tuple: readonly unknown[]
  switch (identity.type) {
    case 'contribution':
      return encodeAixWorkbenchRouteKey(identity)
    case 'activation':
      tuple = ['aix-route', 1, ...activationTuple(identity.activation)]
      break
    case 'instance':
      tuple = ['aix-route', 1, ...instanceTuple(identity.instance)]
      break
    case 'operation':
      assertIdentityPart(identity.operation.operationId, 'operation identity')
      tuple = [
        'aix-route',
        1,
        'aix-operation',
        1,
        ...instanceTuple(identity.operation.instance),
        identity.operation.operation,
        identity.operation.operationId
      ]
      break
    default: {
      const exhaustive: never = identity
      throw new Error(`Unknown AIX route identity: ${String(exhaustive)}`)
    }
  }
  return `aixv1-${uuidv5(JSON.stringify(tuple), uuidv5.URL).replaceAll('-', '')}`
}

/** Route admission is closed to the canonical v1 encoding. */
export function parseAixWorkbenchRouteKey(value: unknown): string | null {
  return typeof value === 'string' && ROUTE_KEY.test(value) ? value : null
}
