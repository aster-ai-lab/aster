// CUA entry for the two-account team drive. It refuses before any pointer,
// click, or window-raise. The functional proof is two_account_gui.py, which
// posts accessibility events with delivery_mode background.
if (process.env.A3S_GUI_ALLOW_MOUSE !== '1') {
  console.error(
    'Refusing CUA mouse control for the team workbench two-account drive.\n' +
      'A3S_GUI_ALLOW_MOUSE is unset.\n' +
      'Stopped before any pointer move, click, or window-raise.\n' +
      'Mouse-safe drive: python3 tests/e2e/team-workbench/two_account_gui.py'
  )
  process.exit(78)
}

console.error('This team-workbench entry does not take the pointer.')
process.exit(78)
