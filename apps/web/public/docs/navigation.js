(() => {
  const menu = document.querySelector('.docs-sidebar .chapter-nav');
  if (!menu?.dataset.scrollScope) return;
  const key = `aster.docs.sidebar:${menu.dataset.scrollScope}`;
  try {
    const saved = sessionStorage.getItem(key);
    const position = saved === null ? 0 : Number(saved);
    if (Number.isFinite(position) && position >= 0) menu.scrollTop = position;
  } catch { /* Storage can be disabled; ordinary links still work. */ }
  const save = () => {
    if (menu.clientHeight === 0) return;
    try { sessionStorage.setItem(key, String(menu.scrollTop)); } catch { /* Optional enhancement. */ }
  };
  menu.addEventListener('click', save, { capture: true });
  menu.addEventListener('auxclick', save, { capture: true });
  window.addEventListener('pagehide', save);
})();
