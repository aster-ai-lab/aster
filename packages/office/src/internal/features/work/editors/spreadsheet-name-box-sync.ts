const NAME_BOX_SELECTOR = '.fortune-name-box';

/**
 * Keeps Fortune's name box on the active cell address.
 *
 * Fortune derives the box from the last entry of its selection list while the formula input and
 * the status line follow the host's active selection. After a programmatic selection those two can
 * disagree (A25 beside a grid that highlights C2). The host selection is the source of truth, so
 * a rewrite by Fortune is reverted. Returns the cleanup.
 *
 * Fortune may remount the formula bar without changing the workbook key; the container observer
 * rebinds to the live box so a detached node cannot leave A25 stuck on screen.
 */
export function bindSpreadsheetNameBox(
  container: HTMLElement,
  address: string,
): () => void {
  const boxObserver = new MutationObserver(() => apply());
  let box: HTMLElement | null = null;
  const apply = () => {
    if (!box || !box.isConnected) return;
    if (box.textContent !== address) box.textContent = address;
  };
  const attach = (next: HTMLElement | null): boolean => {
    if (!next) return false;
    if (box === next) {
      apply();
      return true;
    }
    boxObserver.disconnect();
    box = next;
    apply();
    boxObserver.observe(box, {
      characterData: true,
      childList: true,
      subtree: true,
    });
    return true;
  };
  const sync = () => {
    attach(container.querySelector<HTMLElement>(NAME_BOX_SELECTOR));
  };
  const mountObserver = new MutationObserver(sync);
  mountObserver.observe(container, { childList: true, subtree: true });
  sync();
  return () => {
    boxObserver.disconnect();
    mountObserver.disconnect();
  };
}
