import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

const EMPTY: AixWorkbenchCatalogue = {
  protocolVersion: 1,
  revision: '',
  packages: [],
  diagnostics: []
}

let current = EMPTY
const listeners = new Set<() => void>()

export function publishAixCatalogue(catalogue: AixWorkbenchCatalogue) {
  current = catalogue
  for (const listener of listeners) listener()
}

export function readAixCatalogue(): AixWorkbenchCatalogue {
  return current
}

export function subscribeAixCatalogue(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
