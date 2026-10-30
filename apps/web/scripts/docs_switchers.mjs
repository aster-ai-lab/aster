const switchers = [...document.querySelectorAll('.docs-switcher')];

const linksOf = details => [...details.querySelectorAll('.switcher-menu a')];
const summaryOf = details => details.querySelector('summary');

function closeSwitcher(details, restoreFocus = false) {
  details.open = false;
  summaryOf(details).setAttribute('aria-expanded', 'false');
  details.querySelector('.switcher-menu').classList.remove('switcher-menu--up');
  if (restoreFocus) summaryOf(details).focus({ preventScroll: true });
}

function openSwitcher(details, focusTarget) {
  for (const other of switchers) if (other !== details) closeSwitcher(other);
  details.open = true;
  summaryOf(details).setAttribute('aria-expanded', 'true');
  const links = linksOf(details);
  const target = focusTarget === 'last' ? links.at(-1) : links.find(link => link.getAttribute('aria-current') === 'true') || links[0];
  target?.focus({ preventScroll: true });
  const menu = details.querySelector('.switcher-menu');
  const bounds = menu.getBoundingClientRect();
  menu.classList.toggle('switcher-menu--up', bounds.bottom > window.innerHeight - 16 && bounds.top > bounds.height + 16);
}

for (const details of switchers) {
  const summary = summaryOf(details);
  const menu = details.querySelector('.switcher-menu');
  summary.addEventListener('click', event => {
    event.preventDefault();
    event.stopPropagation();
    if (details.open) closeSwitcher(details);
    else openSwitcher(details);
  });
  summary.addEventListener('keydown', event => {
    if (!['ArrowDown', 'ArrowUp'].includes(event.key)) return;
    event.preventDefault();
    if (!details.open) openSwitcher(details, event.key === 'ArrowUp' ? 'last' : 'first');
    else {
      const links = linksOf(details);
      (event.key === 'ArrowUp' ? links.at(-1) : links[0])?.focus({ preventScroll: true });
    }
  });
  menu.addEventListener('keydown', event => {
    const links = linksOf(details);
    const index = links.indexOf(document.activeElement);
    if (event.key === 'Escape') {
      event.preventDefault();
      closeSwitcher(details, true);
      return;
    }
    if (event.key === 'Tab') {
      closeSwitcher(details);
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? links.length - 1 : event.key === 'ArrowDown' ? Math.min(links.length - 1, index + 1) : Math.max(0, index - 1);
    links[next]?.focus({ preventScroll: true });
  });
}

document.addEventListener('click', event => {
  if (!event.target.closest('.docs-switcher')) for (const details of switchers) closeSwitcher(details);
});
document.addEventListener('keydown', event => {
  if (event.key !== 'Escape') return;
  const open = switchers.find(details => details.open);
  if (open) {
    event.preventDefault();
    closeSwitcher(open, true);
  }
});
