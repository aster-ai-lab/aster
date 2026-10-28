'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createRequire } = require('node:module');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '../../..');
// Development-only parser: the deployed page has no package imports.
const publicRequire = createRequire(path.join(root, 'packages/ui/package.json'));
const { JSDOM } = process.env.JS_DOM_MODULE
  ? require(path.resolve(process.env.JS_DOM_MODULE))
  : publicRequire('jsdom');
const publicRoot = path.join(root, 'apps/web/public');
const html = fs.readFileSync(path.join(publicRoot, 'download/index.html'), 'utf8');
const script = fs.readFileSync(path.join(publicRoot, 'download/app.js'), 'utf8');
const pageUrl = 'https://aster-ai-lab.github.io/aster/download/';
const releaseUrl = 'https://github.com/aster-ai-lab/aster/releases';
const screenshots = {
  workspace: 'Aster 工作区',
  office: 'Aster Office',
  review: 'Aster 自动审查',
  lineage: 'Aster 产物追溯'
};
const text = (element) => element.textContent.trim().replace(/\s+/g, ' ');

function page(t, { execute = true, width = 1280, reducedMotion = false } = {}) {
  const dom = new JSDOM(html, { url: pageUrl, runScripts: 'outside-only' });
  t.after(() => dom.window.close());
  const { window } = dom;
  const { document } = window;
  const network = [];
  const scrollCalls = [];
  window.scrollTo = (options) => scrollCalls.push(options);
  window.fetch = (...args) => { network.push(args); throw new Error('The gallery must not request a model or user files'); };
  window.XMLHttpRequest = class { constructor() { network.push('XMLHttpRequest'); throw new Error('Unexpected request'); } };
  window.navigator.sendBeacon = (...args) => { network.push(args); return false; };
  t.after(() => assert.deepEqual(network, []));
  const media = new Map();
  function matches(query) {
    if (query === '(min-width: 768px)') return width >= 768;
    if (query === '(prefers-reduced-motion: reduce)') return reducedMotion;
    throw new Error(`Add an explicit fixture for media query: ${query}`);
  }
  window.matchMedia = (query) => {
    if (!media.has(query)) {
      const listeners = new Set();
      media.set(query, { media: query, matches: matches(query), listeners,
        addEventListener: (_type, listener) => listeners.add(listener),
        removeEventListener: (_type, listener) => listeners.delete(listener) });
    }
    return media.get(query);
  };
  // jsdom has no top layer or native Escape default. These tests exercise
  // the app's cancel/close integration; the ACL owns real modal focus behavior.
  window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  window.HTMLDialogElement.prototype.close = function () {
    if (!this.open) return;
    this.open = false;
    this.dispatchEvent(new window.Event('close'));
  };
  if (execute) window.eval(script);
  return { window, document, network, scrollCalls,
    scroll(y) { window.scrollY = y; window.dispatchEvent(new window.Event('scroll')); },
    resize(nextWidth) {
      width = nextWidth;
      for (const item of media.values()) {
        const next = matches(item.media);
        if (next === item.matches) continue;
        item.matches = next;
        for (const listener of item.listeners) listener({ matches: next, media: item.media });
      }
    },
    key(element, key) {
      const event = new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(event);
      return event;
    },
    cancel(dialog) {
      const event = new window.Event('cancel', { cancelable: true });
      dialog.dispatchEvent(event);
      return event;
    },
    clickAt(element, x, y) {
      element.dispatchEvent(new window.MouseEvent('click', { bubbles: true, clientX: x, clientY: y }));
    },
    settleClose: () => new Promise((resolve) => window.setTimeout(resolve, 210))
  };
}

function selectedFeature(ui, key, { focused = false } = {}) {
  const tabs = [...ui.document.querySelectorAll('.feature-tabs [role="tab"]')];
  assert.equal(tabs.length, 4);
  for (const tab of tabs) {
    const selected = tab.id === `feature-${key}`;
    const panel = ui.document.getElementById(tab.getAttribute('aria-controls'));
    assert.equal(tab.getAttribute('aria-selected'), String(selected));
    assert.equal(tab.tabIndex, selected ? 0 : -1);
    assert.equal(panel.hidden, !selected);
    assert.equal(panel.getAttribute('aria-labelledby'), tab.id);
    assert.ok(panel.querySelector('.feature-copy'));
    assert.ok(panel.querySelector('.screenshot-figure img'));
  }
  if (focused) assert.equal(ui.document.activeElement.id, `feature-${key}`);
}

// Run this without a name filter once the capture owner has supplied the four
// actual PNGs. Missing images fail; this test does not create placeholder images.
test('public assets exist, four screenshot PNGs have dimensions, and project Pages paths resolve', (t) => {
  const { document } = page(t, { execute: false });
  for (const element of document.querySelectorAll('img[src], script[src], link[rel="stylesheet"][href], link[rel="icon"][href]')) {
    const url = new URL(element.getAttribute('src') || element.getAttribute('href'), pageUrl);
    assert.equal(url.origin, new URL(pageUrl).origin);
    assert.ok(url.pathname.startsWith('/aster/'));
    const asset = path.join(publicRoot, url.pathname.slice('/aster/'.length));
    assert.ok(fs.statSync(asset).isFile(), asset);
    assert.ok(fs.statSync(asset).size > 0, asset);
  }
  for (const key of Object.keys(screenshots)) {
    const file = path.join(publicRoot, `download/screenshots/${key}.png`);
    const bytes = fs.readFileSync(file);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.equal(bytes.toString('ascii', 12, 16), 'IHDR');
    assert.ok(bytes.readUInt32BE(16) > 0 && bytes.readUInt32BE(20) > 0, file);
  }
});

test('no-JS release links and labelled screenshots remain; retired simulated Office controls are absent', (t) => {
  const { document } = page(t, { execute: false });
  const ids = [...document.querySelectorAll('[id]')].map((element) => element.id);
  assert.equal(ids.length, new Set(ids).size);
  for (const anchor of document.querySelectorAll('a[href^="#"]')) {
    const fragment = decodeURIComponent(anchor.getAttribute('href').slice(1));
    if (fragment) assert.ok(document.getElementById(fragment), `Missing #${fragment}`);
  }
  assert.equal(document.querySelectorAll('a[href*="/releases/download/"]').length, 6);
  assert.equal(document.body.textContent.includes('麒麟'), false);
  assert.equal(document.querySelector('noscript a').href, releaseUrl);
  const downloaders = [...document.querySelectorAll('[data-download]')];
  assert.ok(downloaders.length > 0);
  for (const opener of downloaders) {
    assert.equal(opener.tagName, 'A');
    assert.equal(opener.href, releaseUrl);
  }
  assert.equal(document.querySelectorAll('#demo-form, #demo-prompt, [data-demo-reset], [data-tour], [data-demo-option], [data-review-action], .workspace-showcase, .office-paper, .office-toolbar, [contenteditable], select').length, 0);
  assert.equal(document.body.classList.contains('enhanced'), false);
  for (const panel of document.querySelectorAll('.feature-panel')) assert.equal(panel.hidden, false);
  for (const opener of document.querySelectorAll('[data-screenshot], [data-enlarge]')) {
    const key = opener.dataset.screenshot || opener.dataset.enlarge;
    assert.equal(opener.tagName, 'A');
    assert.equal(opener.href, new URL(`screenshots/${key}.png`, pageUrl).href);
    assert.equal(opener.getAttribute('aria-haspopup'), 'dialog');
  }
  for (const figure of document.querySelectorAll('.screenshot-figure')) {
    assert.ok(figure.querySelector('img').alt.trim());
    const caption = figure.querySelector('figcaption');
    if (figure.classList.contains('hero-screen')) {
      assert.equal(text(caption.querySelector('span')), '真实 Aster 界面 · 演示数据');
    } else {
      const captions = { workspace: '工作区 · 真实界面，演示数据', office: 'Office · 真实界面，演示文档', review: '自动审查 · 真实界面，演示记录', lineage: '产物追溯 · 真实界面，演示快照' };
      assert.equal(text(caption), captions[figure.querySelector('[data-screenshot]').dataset.screenshot]);
    }
  }
});

test('four feature tabs switch whole panels, wrap left/right, support Home/End and rove one tab stop', (t) => {
  const ui = page(t);
  selectedFeature(ui, 'workspace');
  assert.equal(ui.document.body.classList.contains('enhanced'), true);
  for (const key of ['office', 'review', 'lineage', 'workspace']) {
    ui.document.querySelector(`#feature-${key}`).click();
    selectedFeature(ui, key);
  }
  ui.document.querySelector('#feature-workspace').focus();
  for (const [key, expected] of [
    ['ArrowLeft', 'lineage'], ['ArrowRight', 'workspace'], ['End', 'lineage'],
    ['Home', 'workspace'], ['ArrowRight', 'office'], ['ArrowRight', 'review']
  ]) {
    assert.equal(ui.key(ui.document.activeElement, key).defaultPrevented, true);
    selectedFeature(ui, expected, { focused: true });
  }
  assert.equal(ui.key(ui.document.activeElement, 'ArrowDown').defaultPrevented, false);
  selectedFeature(ui, 'review', { focused: true });
});

test('every screenshot and enlarge opener selects its own PNG, original URL, alt, title and disclosure', (t) => {
  const ui = page(t, { reducedMotion: true });
  const dialog = ui.document.querySelector('#screenshot-dialog');
  const openers = [...ui.document.querySelectorAll('[data-screenshot], [data-enlarge]')];
  assert.deepEqual([...new Set(openers.map((opener) => opener.dataset.screenshot || opener.dataset.enlarge))].sort(), Object.keys(screenshots).sort());
  for (const opener of openers) {
    const key = opener.dataset.screenshot || opener.dataset.enlarge;
    const panel = opener.closest('[role="tabpanel"]');
    if (panel) ui.document.querySelector(`#${panel.getAttribute('aria-labelledby')}`).click();
    opener.focus();
    const click = new ui.window.MouseEvent('click', { bubbles: true, cancelable: true });
    opener.dispatchEvent(click);
    assert.equal(click.defaultPrevented, true);
    assert.equal(ui.window.location.href, pageUrl);
    assert.equal(dialog.open, true);
    const image = ui.document.querySelector('#screenshot-full');
    const original = ui.document.querySelector('#screenshot-original');
    const expected = new URL(`screenshots/${key}.png`, pageUrl).href;
    assert.equal(image.src, expected);
    assert.equal(original.href, expected);
    assert.equal(original.target, '_blank');
    assert.equal(original.rel, 'noopener noreferrer');
    assert.equal(image.alt, `${screenshots[key]}真实界面，演示数据`);
    assert.equal(text(ui.document.querySelector('#screenshot-title')), `${screenshots[key]}真实界面`);
    assert.equal(text(ui.document.querySelector('.screenshot-dialog-footer p')), '实际 Aster 界面 · 独立演示数据');
    dialog.querySelector('[data-close-dialog]').focus();
    dialog.querySelector('[data-close-dialog]').click();
    assert.equal(dialog.open, false);
    assert.equal(ui.document.activeElement, opener);
  }
});

for (const [id, openerSelector] of [
  ['download-dialog', '.site-header [data-download]'],
  ['screenshot-dialog', '.hero-screen [data-screenshot="workspace"]']
]) {
  for (const method of ['close button', 'cancel', 'backdrop']) {
    test(`${id}: ${method} waits for its close animation and restores the opener`, async (t) => {
      const ui = page(t);
      const opener = ui.document.querySelector(openerSelector);
      const dialog = ui.document.getElementById(id);
      opener.focus(); opener.click();
      assert.equal(dialog.open, true);
      dialog.getBoundingClientRect = () => ({ left: 100, right: 600, top: 100, bottom: 600 });
      ui.clickAt(dialog, 200, 200);
      ui.clickAt(dialog.querySelector('h2'), 0, 0);
      assert.equal(dialog.open, true);
      assert.equal(dialog.classList.contains('closing'), false);
      dialog.querySelector('[data-close-dialog]').focus();
      if (method === 'close button') dialog.querySelector('[data-close-dialog]').click();
      if (method === 'cancel') assert.equal(ui.cancel(dialog).defaultPrevented, true);
      if (method === 'backdrop') ui.clickAt(dialog, 50, 50);
      assert.equal(dialog.open, true);
      assert.equal(dialog.classList.contains('closing'), true);
      await ui.settleClose();
      assert.equal(dialog.open, false);
      assert.equal(dialog.classList.contains('closing'), false);
      assert.equal(ui.document.activeElement, opener);
    });
  }
  test(`${id}: reopening cancels the prior close timer`, async (t) => {
    const ui = page(t);
    const opener = ui.document.querySelector(openerSelector);
    const dialog = ui.document.getElementById(id);
    opener.focus(); opener.click();
    dialog.querySelector('[data-close-dialog]').click();
    assert.equal(dialog.classList.contains('closing'), true);
    // Re-dispatch the real opener before the previous 200ms leave completes.
    opener.click();
    assert.equal(dialog.open, true);
    assert.equal(dialog.classList.contains('closing'), false);
    await ui.settleClose();
    assert.equal(dialog.open, true);
    dialog.querySelector('[data-close-dialog]').click();
    await ui.settleClose();
    assert.equal(dialog.open, false);
    assert.equal(ui.document.activeElement, opener);
  });
  test(`${id}: reduced motion cancels immediately without a delayed close`, (t) => {
    const ui = page(t, { reducedMotion: true });
    const opener = ui.document.querySelector(openerSelector);
    const dialog = ui.document.getElementById(id);
    opener.focus(); opener.click();
    dialog.querySelector('[data-close-dialog]').focus();
    assert.equal(ui.cancel(dialog).defaultPrevented, true);
    assert.equal(dialog.open, false);
    assert.equal(dialog.classList.contains('closing'), false);
    assert.equal(ui.document.activeElement, opener);
  });
}

test('mobile navigation closes on Escape, destination selection and desktop resize', (t) => {
  const ui = page(t, { width: 390 });
  const trigger = ui.document.querySelector('.menu-toggle');
  const menu = ui.document.querySelector('#mobile-nav');
  assert.equal(menu.hidden, true);
  trigger.click();
  assert.equal(menu.hidden, false);
  assert.equal(trigger.getAttribute('aria-expanded'), 'true');
  assert.equal(trigger.getAttribute('aria-label'), '关闭导航');
  menu.querySelector('a').focus();
  ui.key(ui.document.activeElement, 'Escape');
  assert.equal(menu.hidden, true);
  assert.equal(ui.document.activeElement, trigger);
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  trigger.click(); menu.querySelector('a').click();
  assert.equal(menu.hidden, true);
  trigger.click(); ui.resize(1280);
  assert.equal(menu.hidden, true);
  assert.equal(trigger.getAttribute('aria-expanded'), 'false');
  assert.equal(trigger.getAttribute('aria-label'), '打开导航');
});

for (const reducedMotion of [false, true]) {
  test(`back-to-top visibility, scroll intent and brand focus (reduced motion: ${reducedMotion})`, (t) => {
    const ui = page(t, { reducedMotion });
    const button = ui.document.querySelector('.back-to-top');
    assert.equal(button.hidden, true);
    ui.scroll(599); assert.equal(button.hidden, true);
    ui.scroll(600); assert.equal(button.hidden, false);
    button.focus(); button.click();
    assert.equal(ui.scrollCalls.length, 1);
    assert.equal(ui.scrollCalls[0].top, 0);
    assert.equal(ui.scrollCalls[0].behavior, reducedMotion ? 'instant' : 'smooth');
    assert.equal(ui.document.activeElement, ui.document.querySelector('.brand'));
    ui.scroll(0); assert.equal(button.hidden, true);
  });
}

test('the real-screenshot FAQ is an ordinary disclosure and states the fixture boundary', (t) => {
  const { document } = page(t);
  const details = [...document.querySelectorAll('.faq-list details')].find((item) => text(item.querySelector('summary')).startsWith('网站里的应用画面是真实界面吗？'));
  assert.ok(details);
  assert.equal(details.open, false);
  details.querySelector('summary').click();
  assert.equal(details.open, true);
  assert.match(text(details.querySelector('p')), /截图由实际 Aster 界面运行后采集/);
  assert.match(text(details.querySelector('p')), /不会运行 Office、调用模型或访问你的文件/);
});
