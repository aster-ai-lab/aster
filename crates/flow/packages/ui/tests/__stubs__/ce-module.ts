/**
 * Marking-only fake for the ce runtimes in the test environment.
 *
 * The real bundles enhance hosts (hasSelectRuntime turns true and the
 * runtime owns interaction); this fake only marks div.select hosts
 * data-select-initialized — satisfying tests that waitFor the mark —
 * while the fallback controllers keep owning interaction semantics
 * (keyboard focus starts unselected: the model this tree's tests
 * were written against).
 */
function markAll(): void {
  if (typeof document === 'undefined') return
  for (const el of Array.from(
    document.querySelectorAll('div.select:not([data-select-initialized])'),
  )) {
    el.setAttribute('data-select-initialized', 'true')
  }
}

try {
  markAll()
  if (typeof MutationObserver !== 'undefined' && typeof document !== 'undefined' && document.body) {
    new MutationObserver(markAll).observe(document.body, { childList: true, subtree: true })
  }
} catch {
  // a mark must never fail an import
}

export default {}
