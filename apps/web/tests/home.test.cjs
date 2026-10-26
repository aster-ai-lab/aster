'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createRequire } = require('node:module');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '../../..');
const publicRequire = createRequire(path.join(root, 'packages/ui/package.json'));
const { JSDOM } = process.env.JS_DOM_MODULE
  ? require(path.resolve(process.env.JS_DOM_MODULE))
  : publicRequire('jsdom');
const publicRoot = path.join(root, 'apps/web/public');
const html = fs.readFileSync(path.join(publicRoot, 'index.html'), 'utf8');
const script = fs.readFileSync(path.join(publicRoot, 'home.js'), 'utf8');
const pageUrl = 'https://aster-ai-lab.github.io/aster/';

function page(t, { execute = true, reducedMotion = false, platform = 'MacIntel', fakeTimers = false, hoverCapable = false } = {}) {
  const dom = new JSDOM(html, { url: pageUrl, runScripts: 'outside-only' });
  t.after(() => dom.window.close());
  const { window } = dom;
  const { document } = window;
  const scrollCalls = [];
  const clipboard = [];
  const timerQueue = [];
  let timerSequence = 0;
  window.matchMedia = (query) => ({
    media: query,
    matches: query === '(prefers-reduced-motion: reduce)' ? reducedMotion : query === '(hover: hover) and (pointer: fine)' ? hoverCapable : false,
    addEventListener() {},
    removeEventListener() {}
  });
  window.requestAnimationFrame = (callback) => {
    callback(0);
    return 1;
  };
  window.scrollTo = (options) => scrollCalls.push(options);
  Object.defineProperty(window.navigator, 'platform', { value: platform, configurable: true });
  Object.defineProperty(window.navigator, 'userAgent', {
    value: platform === 'MacIntel' ? 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' : platform,
    configurable: true
  });
  Object.defineProperty(window, 'isSecureContext', { value: true, configurable: true });
  Object.defineProperty(window.navigator, 'clipboard', {
    configurable: true,
    value: { writeText: async (value) => clipboard.push(value) }
  });
  if (fakeTimers) {
    window.setTimeout = (callback) => {
      timerSequence += 1;
      timerQueue.push({ id: timerSequence, callback });
      return timerSequence;
    };
    window.clearTimeout = (id) => {
      const index = timerQueue.findIndex((timer) => timer.id === id);
      if (index >= 0) timerQueue.splice(index, 1);
    };
  }
  if (execute) window.eval(script);
  return {
    window,
    document,
    scrollCalls,
    clipboard,
    key(element, key) {
      const event = new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(event);
      return event;
    },
    resize(width) {
      window.innerWidth = width;
      window.dispatchEvent(new window.Event('resize'));
    },
    flushTimers() {
      let count = 0;
      while (timerQueue.length) {
        const timer = timerQueue.shift();
        timer.callback();
        count += 1;
        if (count > 500) throw new Error('Timer queue did not settle');
      }
    }
  };
}

function selectedScenario(ui, id) {
  const tabs = [...ui.document.querySelectorAll('.scenario-tabs [role="tab"]')];
  assert.equal(tabs.length, 4);
  for (const tab of tabs) {
    const selected = tab.id === `tab-${id}`;
    const panel = ui.document.getElementById(tab.getAttribute('aria-controls'));
    assert.equal(tab.getAttribute('aria-selected'), String(selected));
    assert.equal(tab.tabIndex, selected ? 0 : -1);
    assert.equal(panel.hidden, !selected);
    assert.equal(panel.getAttribute('aria-labelledby'), tab.id);
    assert.ok(panel.querySelector('img').alt.trim());
  }
}

test('homepage assets resolve under project Pages and no-JS content stays available', (t) => {
  const { document } = page(t, { execute: false });
  for (const element of document.querySelectorAll('img[src], script[src], link[rel="stylesheet"][href], link[rel="icon"][href]')) {
    const url = new URL(element.getAttribute('src') || element.getAttribute('href'), pageUrl);
    assert.equal(url.origin, new URL(pageUrl).origin);
    assert.ok(url.pathname.startsWith('/aster/'));
    const asset = path.join(publicRoot, url.pathname.slice('/aster/'.length));
    assert.ok(fs.statSync(asset).isFile(), asset);
    assert.ok(fs.statSync(asset).size > 0, asset);
  }
  const ids = [...document.querySelectorAll('[id]')].map((element) => element.id);
  assert.equal(ids.length, new Set(ids).size);
  const releaseLinks = [...document.querySelectorAll('a[href*="/releases/download/"]')];
  assert.equal(releaseLinks.length, 12);
  assert.equal(new Set(releaseLinks.map((link) => link.href)).size, 6);
  assert.deepEqual(
    [...document.querySelectorAll('.desktop-nav a')].map((link) => link.textContent.trim()),
    ['首页', '场景', '能力', '安全', '文档']
  );
  assert.deepEqual(
    [...document.querySelectorAll('.mobile-nav a')].map((link) => link.textContent.trim()),
    ['首页', '场景', '能力', '安全', '文档']
  );
  assert.equal(document.querySelector('.hero-actions .text-link'), null);
  assert.equal(document.querySelectorAll('.download-grid .download-card').length, 4);
  assert.equal(document.querySelector('.download-card-cli').getAttribute('href'), '#cli-downloads');
  assert.equal(document.querySelectorAll('.scenario-panel').length, 4);
  for (const panel of document.querySelectorAll('.scenario-panel')) assert.equal(panel.hidden, false);
  assert.equal(document.querySelectorAll('select, iframe, [contenteditable]').length, 0);
  assert.equal(/workbuddy/i.test(document.body.textContent), false);
});

test('scenario tabs switch panels and support arrow, Home and End keys', (t) => {
  const ui = page(t);
  selectedScenario(ui, 'research');
  for (const id of ['document', 'data', 'development', 'research']) {
    ui.document.querySelector(`#tab-${id}`).click();
    selectedScenario(ui, id);
  }
  ui.document.querySelector('#tab-research').focus();
  for (const [key, expected] of [
    ['ArrowLeft', 'development'], ['ArrowRight', 'research'], ['End', 'development'],
    ['Home', 'research'], ['ArrowRight', 'document'], ['ArrowRight', 'data']
  ]) {
    assert.equal(ui.key(ui.document.activeElement, key).defaultPrevented, true);
    selectedScenario(ui, expected);
    assert.equal(ui.document.activeElement.id, `tab-${expected}`);
  }
  assert.equal(ui.key(ui.document.activeElement, 'ArrowDown').defaultPrevented, false);
});

test('mobile navigation closes on selection, Escape and desktop resize', (t) => {
  const ui = page(t);
  const trigger = ui.document.querySelector('.menu-toggle');
  const menu = ui.document.querySelector('#mobile-nav');
  trigger.click();
  assert.equal(trigger.getAttribute('aria-expanded'), 'true');
  assert.equal(menu.classList.contains('is-open'), true);
  assert.equal(ui.document.body.classList.contains('menu-open'), true);
  menu.querySelector('a[href="#scenarios"]').click();
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  assert.equal(menu.classList.contains('is-open'), false);
  trigger.click();
  ui.key(ui.document.body, 'Escape');
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  assert.equal(ui.document.activeElement, trigger);
  trigger.click();
  ui.resize(1100);
  assert.equal(menu.classList.contains('is-open'), false);
  assert.equal(ui.document.body.classList.contains('menu-open'), false);
});

test('download disclosures switch, support keyboard focus and close outside or on Escape', (t) => {
  const ui = page(t);
  const headerTrigger = ui.document.querySelector('.download-dropdown-header [data-download-trigger]');
  const heroTrigger = ui.document.querySelector('.download-dropdown-hero [data-download-trigger]');
  const headerPopover = ui.document.getElementById('download-menu-header');
  const heroPopover = ui.document.getElementById('download-menu-hero');
  assert.equal(headerPopover.hidden, true);
  assert.equal(heroPopover.hidden, true);

  headerTrigger.click();
  assert.equal(headerTrigger.getAttribute('aria-expanded'), 'true');
  assert.equal(headerPopover.hidden, false);
  assert.equal(heroPopover.hidden, true);

  heroTrigger.click();
  assert.equal(headerTrigger.getAttribute('aria-expanded'), 'false');
  assert.equal(headerPopover.hidden, true);
  assert.equal(heroPopover.hidden, false);
  assert.equal(heroTrigger.getAttribute('aria-expanded'), 'true');

  assert.equal(ui.key(heroTrigger, 'ArrowDown').defaultPrevented, true);
  assert.equal(ui.document.activeElement, heroPopover.querySelector('a'));
  ui.key(ui.document.activeElement, 'Escape');
  assert.equal(heroPopover.hidden, true);
  assert.equal(ui.document.activeElement, heroTrigger);

  ui.key(heroTrigger, ' ');
  assert.equal(heroPopover.hidden, false);
  ui.document.body.click();
  assert.equal(heroPopover.hidden, true);
  assert.equal(heroTrigger.getAttribute('aria-expanded'), 'false');

  headerTrigger.click();
  ui.document.querySelector('.menu-toggle').click();
  assert.equal(headerPopover.hidden, true);
});

test('download disclosures open on hover, close after leaving and stay pinned after click', (t) => {
  const ui = page(t, { fakeTimers: true, hoverCapable: true });
  const dropdown = ui.document.querySelector('.download-dropdown-hero');
  const trigger = dropdown.querySelector('[data-download-trigger]');
  const popover = ui.document.getElementById('download-menu-hero');
  const hover = (type) => dropdown.dispatchEvent(new ui.window.MouseEvent(type));

  hover('mouseenter');
  assert.equal(trigger.getAttribute('aria-expanded'), 'true');
  assert.equal(popover.hidden, false);
  hover('mouseleave');
  ui.flushTimers();
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  assert.equal(popover.hidden, true);

  hover('mouseenter');
  trigger.click();
  hover('mouseleave');
  ui.flushTimers();
  assert.equal(trigger.getAttribute('aria-expanded'), 'true');
  assert.equal(popover.hidden, false);
  ui.document.body.click();
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  assert.equal(popover.hidden, true);
});

test('current desktop platform is labelled and the quickstart command can be copied', async (t) => {
  const ui = page(t);
  const current = ui.document.querySelectorAll('.download-card.is-current');
  assert.equal(current.length, 1);
  assert.equal(current[0].dataset.platform, 'macos');
  assert.equal(current[0].querySelector('.current-badge').textContent, '当前设备');
  assert.equal(ui.document.querySelectorAll('[data-download-platform="macos"] .platform-current').length, 2);
  const button = ui.document.querySelector('[data-copy-command]');
  button.click();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(ui.clipboard, ['./aster --cwd /path/to/project']);
  assert.equal(button.querySelector('span').textContent, '已复制');
});

test('hero title types every character in place and retires the cursor', (t) => {
  const ui = page(t, { fakeTimers: true });
  const title = ui.document.querySelector('[data-typing-title]');
  const characters = [...title.querySelectorAll('.typing-char')];
  assert.equal(title.getAttribute('aria-label'), 'Aster，让想法落地');
  assert.equal(title.classList.contains('is-typing'), true);
  assert.equal(characters.length, Array.from('Aster，让想法落地').length);
  assert.equal(characters.every((character) => !character.classList.contains('is-visible')), true);
  ui.flushTimers();
  assert.equal(characters.every((character) => character.classList.contains('is-visible')), true);
  assert.equal(title.textContent, 'Aster，让想法落地');
  const cursor = title.querySelector('.typing-cursor');
  assert.equal(cursor.classList.contains('is-complete'), true);
  assert.equal(cursor.classList.contains('is-active'), false);
});

test('reduced motion preserves the original hero title without typing', (t) => {
  const ui = page(t, { reducedMotion: true, fakeTimers: true });
  const title = ui.document.querySelector('[data-typing-title]');
  assert.equal(title.textContent, 'Aster，让想法落地');
  assert.equal(title.classList.contains('is-typing'), false);
  assert.equal(title.querySelectorAll('.typing-char, .typing-cursor').length, 0);
});

test('back to top respects reduced motion and uses the recorded scroll position', (t) => {
  const ui = page(t, { reducedMotion: true });
  Object.defineProperty(ui.window, 'scrollY', { value: 1000, configurable: true });
  ui.window.dispatchEvent(new ui.window.Event('scroll'));
  const button = ui.document.querySelector('.back-to-top');
  assert.equal(button.classList.contains('is-visible'), true);
  button.click();
  assert.equal(ui.scrollCalls.length, 1);
  assert.equal(ui.scrollCalls[0].top, 0);
  assert.equal(ui.scrollCalls[0].behavior, 'auto');
});
