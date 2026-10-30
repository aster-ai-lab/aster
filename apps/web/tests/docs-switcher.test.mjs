import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL('../../../', import.meta.url));
const { JSDOM } = require(process.env.JS_DOM_MODULE || path.join(root, 'packages/ui/node_modules/jsdom'));

async function page(t) {
  const [html, navigation] = await Promise.all([
    readFile(path.join(root, 'apps/web/dist/docs/sandbox/current/index.html'), 'utf8'),
    readFile(path.join(root, 'apps/web/dist/docs/navigation.js'), 'utf8'),
  ]);
  const dom = new JSDOM(html, { url: 'http://localhost/docs/sandbox/current/', runScripts: 'outside-only' });
  t.after(() => dom.window.close());
  dom.window.matchMedia = () => ({ matches: true, addEventListener() {} });
  dom.window.eval(navigation);
  return dom;
}

test('switchers expose controls and close the sibling menu and outside clicks', async (t) => {
  const dom = await page(t);
  const { document } = dom.window;
  const [product, version] = document.querySelectorAll('.docs-switcher');
  const productSummary = product.querySelector('summary');
  const versionSummary = version.querySelector('summary');
  assert.equal(productSummary.getAttribute('aria-expanded'), 'false');
  assert.equal(productSummary.getAttribute('aria-controls'), 'docs-switcher-product-menu');
  assert.equal(versionSummary.getAttribute('aria-controls'), 'docs-switcher-version-menu');

  productSummary.click();
  assert.equal(product.open, true);
  assert.equal(productSummary.getAttribute('aria-expanded'), 'true');
  versionSummary.click();
  assert.equal(product.open, false);
  assert.equal(version.open, true);
  assert.equal(versionSummary.getAttribute('aria-expanded'), 'true');
  assert.equal(productSummary.getAttribute('aria-expanded'), 'false');

  document.body.click();
  assert.equal(version.open, false);
  assert.equal(versionSummary.getAttribute('aria-expanded'), 'false');
});

test('keyboard navigation selects current, wraps with Home and End, and Escape restores focus', async (t) => {
  const dom = await page(t);
  const { document } = dom.window;
  const details = document.querySelector('.docs-switcher');
  const summary = details.querySelector('summary');
  const menu = details.querySelector('.switcher-menu');
  const links = [...menu.querySelectorAll('a')];
  const current = links.find(link => link.getAttribute('aria-current') === 'true');
  summary.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  assert.equal(details.open, true);
  assert.equal(document.activeElement, current);

  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  assert.equal(document.activeElement, links[links.indexOf(current) + 1]);
  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Home', bubbles: true, cancelable: true }));
  assert.equal(document.activeElement, links[0]);
  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'End', bubbles: true, cancelable: true }));
  assert.equal(document.activeElement, links.at(-1));
  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
  assert.equal(details.open, false);
  summary.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  assert.equal(details.open, true);
  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  assert.equal(details.open, false);
  assert.equal(document.activeElement, summary);
});
