// CUA entry for the composer-mode drive. It refuses before any pointer,
// click, or window-raise. The functional proof is composer-mode-home.mjs.
if (process.env.A3S_GUI_ALLOW_MOUSE !== '1') {
  console.error(
    'Refusing CUA mouse control for the Studio composer mode drive.\n' +
      'Default gate: node tests/e2e/composer-mode-home.mjs (mouse-safe).\n' +
      'When the machine is free: A3S_GUI_ALLOW_MOUSE=1 node tests/e2e/composer-mode-cua.mjs'
  )
  process.exit(78)
}

console.error('This composer-mode entry does not take the pointer.')
process.exit(78)
