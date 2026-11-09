import { getStartingSnapshot, calculateTransitions } from 'codehike/utils/token-transitions';
import './harness_demo.mjs';
import './docs_switchers.mjs';

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

const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)');

for (const walk of document.querySelectorAll('[data-source-walk]')) {
  const stops = [...walk.querySelectorAll('[data-walk-stop]')];
  const controls = walk.querySelector('.walk-controls');
  let current = 0;
  let all = false;
  let animations = [];
  const cancelAnimations = () => { animations.forEach(animation => animation.cancel()); animations = []; };
  const select = (index, announce = true, animate = true) => {
    cancelAnimations();
    const previous = all ? undefined : stops[current].querySelector('.walk-code');
    const next = stops[Math.max(0, Math.min(stops.length - 1, index))].querySelector('.walk-code');
    const canAnimate = animate && !all && previous !== next && !previous?.hidden && !reducedMotion?.matches && typeof next.animate === 'function';
    const sameFile = previous?.dataset.sourceFile === next.dataset.sourceFile && !previous?.classList.contains('is-wrapped') && !next.classList.contains('is-wrapped');
    const snapshot = canAnimate && sameFile ? getStartingSnapshot(previous.querySelector('pre'), { selector: '.walk-token' }) : undefined;
    current = Math.max(0, Math.min(stops.length - 1, index));
    for (const [i, stop] of stops.entries()) {
      const active = all || i === current;
      stop.querySelector('.walk-code').hidden = !active;
      stop.querySelector('.walk-detail').hidden = !active;
      const button = stop.querySelector('[data-walk-step]');
      button.disabled = false;
      button.setAttribute('aria-expanded', String(active));
      button.toggleAttribute('aria-current', i === current && !all);
      if (i === current && !all) button.setAttribute('aria-current', 'step');
      stop.querySelector('.walk-narrative').classList.toggle('is-current', i === current && !all);
    }
    walk.classList.toggle('is-reading-all', all);
    controls.querySelector('[data-walk-action="previous"]').disabled = all || current === 0;
    controls.querySelector('[data-walk-action="next"]').disabled = all || current === stops.length - 1;
    const reading = controls.querySelector('[data-walk-action="all"]');
    reading.textContent = all ? '回到分步漫游' : '连续阅读';
    reading.setAttribute('aria-pressed', String(all));
    walk.querySelector('[data-walk-position]').textContent = all ? `全部 / ${stops.length}` : `${String(current + 1).padStart(2, '0')} / ${String(stops.length).padStart(2, '0')}`;
    walk.querySelector('[data-walk-inline-position]').textContent = walk.querySelector('[data-walk-position]').textContent;
    if (!all) {
      const pre = next.querySelector('pre');
      const focus = pre.querySelector('.walk-focus');
      if (announce && focus) pre.scrollTop = Math.max(0, focus.offsetTop - pre.offsetTop - pre.clientHeight * .3);
      if (canAnimate && snapshot) {
        animations = calculateTransitions(pre, snapshot, { selector: '.walk-token' }).map(({ element, keyframes, options }) => {
          const frames = {};
          if (keyframes.translateX || keyframes.translateY) frames.transform = [0, 1].map(i => `translate(${keyframes.translateX?.[i] || 0}px,${keyframes.translateY?.[i] || 0}px)`);
          if (keyframes.opacity) frames.opacity = keyframes.opacity;
          return element.animate(frames, { duration: Math.max(300, Math.min(500, options.duration * 900)), delay: Math.min(200, options.delay * 300), easing: 'cubic-bezier(.22,1,.36,1)', fill: 'backwards' });
        });
        walk.dataset.transition = 'tokens';
      } else if (canAnimate) {
        animations = [pre.animate([{ opacity: .25, transform: 'translateY(4px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 300, easing: 'cubic-bezier(.22,1,.36,1)' })];
        walk.dataset.transition = 'file';
      } else walk.dataset.transition = 'none';
    }
    if (announce) walk.querySelector('.walk-status').textContent = all ? '正在阅读全部源码。' : `第 ${current + 1} 站，共 ${stops.length} 站：${stops[current].querySelector('.walk-title').textContent.trim()}`;
  };
  const go = index => {
    all = false;
    select(index);
    const button = stops[current].querySelector('[data-walk-step]');
    button.focus({ preventScroll: true });
    if (window.getComputedStyle(walk.querySelector('.walk-stops')).display !== 'grid') {
      stops[current].scrollIntoView?.({ block: 'start', behavior: 'instant' });
    } else {
      const bounds = button.getBoundingClientRect();
      if (bounds.top < 96 || bounds.bottom > window.innerHeight - 24) button.scrollIntoView?.({ block: 'nearest', behavior: 'instant' });
    }
  };
  walk.addEventListener('click', event => {
    const button = event.target.closest('[data-walk-step]');
    if (button) { go(Number(button.dataset.walkStep)); return; }
    const action = event.target.closest('[data-walk-action]')?.dataset.walkAction;
    if (action === 'previous') go(current - 1);
    if (action === 'next') go(current + 1);
    if (action === 'all') { all = !all; select(current, true, false); }
  });
  walk.addEventListener('keydown', event => {
    if (!event.target.closest('[data-walk-step], .walk-controls') || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    go(event.key === 'Home' ? 0 : event.key === 'End' ? stops.length - 1 : current + (['ArrowRight', 'ArrowDown'].includes(event.key) ? 1 : -1));
  });
  for (const panel of walk.querySelectorAll('.walk-code')) {
    panel.querySelector('.walk-code-tools').hidden = false;
    panel.classList.add('is-enhanced');
    panel.addEventListener('click', async event => {
      const button = event.target.closest('[data-code-action]');
      if (!button) return;
      if (button.dataset.codeAction === 'focus') {
        const context = panel.classList.toggle('is-context');
        button.setAttribute('aria-pressed', String(!context));
      }
      if (button.dataset.codeAction === 'wrap') {
        const wrapped = panel.classList.toggle('is-wrapped');
        button.setAttribute('aria-pressed', String(wrapped));
      }
      if (button.dataset.codeAction === 'copy') {
        const status = panel.querySelector('[data-copy-status]');
        try { await navigator.clipboard.writeText(panel.querySelector('pre').textContent); status.textContent = '已复制'; }
        catch { status.textContent = '请选中代码复制'; }
      }
    });
  }
  controls.hidden = false;
  walk.classList.add('is-enhanced');
  reducedMotion?.addEventListener?.('change', cancelAnimations);
  window.addEventListener('pagehide', cancelAnimations, { once: true });
  select(0, false, false);
}

for (const button of document.querySelectorAll('[data-lab-expand]')) button.hidden = typeof HTMLDialogElement === 'undefined' || typeof HTMLDialogElement.prototype.showModal !== 'function';
document.addEventListener('click', event => {
  const button = event.target.closest('[data-lab-expand]');
  if (!button) return;
  const panel = button.closest('.walk-code');
  if (!panel) return;
  const owner = button.closest('.source-walk');
  owner.dispatchEvent(new CustomEvent('reading-view-open'));
  const title = panel.querySelector('.walk-file-location').title;
  const placeholder = document.createComment('reading view');
  const position = window.scrollY;
  const dialog = document.createElement('dialog');
  dialog.className = 'doc-viewer';
  dialog.setAttribute('aria-labelledby', 'doc-viewer-title');
  const header = document.createElement('div'); header.className = 'doc-viewer-header';
  const heading = document.createElement('h2'); heading.id = 'doc-viewer-title'; heading.textContent = title;
  const close = document.createElement('button'); close.type = 'button'; close.textContent = '关闭 ×';
  header.append(heading, close); dialog.append(header); document.body.append(dialog);
  const originalLabel = panel.getAttribute('aria-labelledby');
  panel.setAttribute('aria-labelledby', 'doc-viewer-title');
  panel.before(placeholder); dialog.append(panel);
  const note = document.createElement('div'); note.className = 'doc-viewer-note';
  const updateNote = () => {
    const selected = owner.querySelector('.walk-narrative.is-current');
    note.replaceChildren();
    if (selected) {
      const title = document.createElement('h3'); title.textContent = selected.querySelector('.lab-step-label').textContent;
      note.append(title);
      for (const paragraph of selected.querySelectorAll('p')) note.append(paragraph.cloneNode(true));
      const source = selected.querySelector('.source-ref');
      if (source) note.append(source.cloneNode(true));
    }
  };
  updateNote(); dialog.append(note);
  panel.addEventListener('reading-step-change', updateNote);
  close.addEventListener('click', () => dialog.close());
  dialog.addEventListener('click', event => { if (event.target === dialog) dialog.close(); });
  dialog.addEventListener('close', () => { panel.removeEventListener('reading-step-change', updateNote); placeholder.replaceWith(panel); if (originalLabel) panel.setAttribute('aria-labelledby', originalLabel); dialog.remove(); window.scrollTo({ top: position, behavior: 'instant' }); button.focus({ preventScroll: true }); }, { once: true });
  dialog.showModal();
});
