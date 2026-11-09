const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../public/download/canvas.js'), 'utf8');

function eventTarget() {
  const listeners = new Map();
  return {
    listeners,
    addEventListener(name, callback, options) {
      if (!listeners.has(name)) listeners.set(name, []);
      listeners.get(name).push({ callback, options });
    },
    dispatch(name, event = {}) {
      for (const { callback } of listeners.get(name) || []) callback(event);
    }
  };
}

function setup(options = {}) {
  const state = { draws: 0, dimensionWrites: 0, transforms: [], translations: [], gradients: 0, frames: new Map(), nextId: 1 };
  const bounds = { width: 1200, height: 900, top: 0, left: 0, ...options.bounds };
  const hero = { ...eventTarget(), getBoundingClientRect: () => ({ ...bounds }) };
  const context = Object.fromEntries(['save', 'restore', 'rotate', 'beginPath', 'ellipse', 'stroke', 'moveTo', 'lineTo', 'closePath', 'arc', 'fill'].map(name => [name, () => {}]));
  Object.assign(context, {
    clearRect() { state.draws++; },
    setTransform(...args) { state.transforms.push(args); },
    translate(...args) { state.translations.push(args); },
    createLinearGradient() { return { addColorStop() {} }; },
    createRadialGradient() { state.gradients++; return { addColorStop() {} }; }
  });
  const canvas = {
    closest: selector => selector === '.hero' ? hero : null,
    parentElement: {},
    getBoundingClientRect: () => ({ ...bounds }),
    getContext() {
      if (options.contextThrows) throw new Error('unsupported canvas');
      return options.noContext ? null : context;
    }
  };
  for (const name of ['width', 'height']) Object.defineProperty(canvas, name, {
    set(value) { state.dimensionWrites++; state[name] = value; }, get() { return state[name]; }
  });
  const document = { ...eventTarget(), hidden: false, getElementById: () => options.missingCanvas ? null : canvas };
  const motion = { ...eventTarget(), matches: options.reduced || false };
  const window = {
    ...eventTarget(), devicePixelRatio: options.dpr || 1,
    matchMedia: () => motion,
    requestAnimationFrame(callback) { const id = state.nextId++; state.frames.set(id, callback); return id; },
    cancelAnimationFrame(id) { state.frames.delete(id); }
  };
  const globals = { document, window, console };
  if (!options.noIntersection) globals.IntersectionObserver = class {
    constructor(callback) { state.intersection = callback; }
    observe(element) { assert.equal(element, hero); }
  };
  globals.ResizeObserver = class {
    constructor(callback) { state.resize = callback; }
    observe(element) { assert.equal(element, canvas.parentElement); }
  };
  vm.runInNewContext(source, globals, { timeout: 1000 });
  return {
    state, bounds, hero, document, window, motion,
    intersect(value) { state.intersection([{ isIntersecting: value }]); },
    advance(now) {
      const callbacks = [...state.frames.values()];
      state.frames.clear();
      callbacks.forEach(callback => callback(now));
    }
  };
}

test('missing or unavailable canvas keeps the existing fallback without scheduling work', () => {
  for (const options of [{ missingCanvas: true }, { noContext: true }, { contextThrows: true }]) {
    const { state, hero } = setup(options);
    assert.equal(state.frames.size, 0);
    assert.equal(state.dimensionWrites, 0);
    assert.equal(hero.listeners.size, 0);
  }
});

test('DPR is clamped at two and unchanged resize observations do not reset the backing store', () => {
  const env = setup({ dpr: 3 });
  assert.equal(env.state.width, 2400);
  assert.equal(env.state.height, 1800);
  assert.deepEqual(env.state.transforms[0], [2, 0, 0, 2, 0, 0]);
  env.state.resize();
  env.window.dispatch('resize');
  assert.equal(env.state.dimensionWrites, 2);
  env.bounds.width = 900;
  env.state.resize();
  assert.equal(env.state.width, 1800);
  assert.equal(env.state.dimensionWrites, 4);
});

test('reduced motion draws a static frame and never starts an animation loop', () => {
  const env = setup({ reduced: true });
  env.intersect(true);
  assert.equal(env.state.draws, 1);
  assert.ok(env.state.gradients >= 1);
  assert.equal(env.state.frames.size, 0);
  env.state.resize();
  env.hero.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'mouse' });
  assert.equal(env.state.draws, 1);
  assert.equal(env.state.frames.size, 0);
  env.motion.matches = false;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 1);
  env.motion.matches = true;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 0);
});

test('painting is limited to thirty FPS and pauses for both visibility and intersection', () => {
  const env = setup();
  assert.equal(env.state.frames.size, 0);
  env.intersect(true);
  env.advance(0);
  assert.equal(env.state.draws, 1);
  env.advance(16);
  assert.equal(env.state.draws, 1);
  env.advance(34);
  assert.equal(env.state.draws, 2);
  env.document.hidden = true;
  env.document.dispatch('visibilitychange');
  assert.equal(env.state.frames.size, 0);
  env.document.hidden = false;
  env.document.dispatch('visibilitychange');
  assert.equal(env.state.frames.size, 1);
  env.intersect(false);
  assert.equal(env.state.frames.size, 0);
  env.intersect(true);
  assert.equal(env.state.frames.size, 1);
});

test('page cache lifecycle stops work and resumes without requiring a size change', () => {
  const env = setup();
  env.intersect(true);
  env.window.dispatch('pagehide');
  assert.equal(env.state.frames.size, 0);
  env.intersect(true);
  assert.equal(env.state.frames.size, 0);
  env.window.dispatch('pageshow');
  assert.equal(env.state.frames.size, 1);
  assert.equal(env.state.dimensionWrites, 2);
});

test('pointer response stays small, passive and does not intercept touch or scroll', () => {
  const env = setup();
  env.intersect(true);
  assert.equal(env.hero.listeners.get('pointermove')[0].options.passive, true);
  env.hero.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'mouse', preventDefault() { assert.fail('must remain passive'); } });
  env.advance(100);
  const [x, y] = env.state.translations.at(-1);
  assert.ok(x > 600 && x <= 606);
  assert.ok(y > 288 && y <= 292);
  for (const target of [env.hero, env.window, env.document]) {
    assert.equal(target.listeners.has('wheel'), false);
    assert.equal(target.listeners.has('touchmove'), false);
  }
  const touch = setup();
  touch.intersect(true);
  touch.hero.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'touch' });
  touch.advance(100);
  assert.deepEqual(touch.state.translations.at(-1), [600, 288]);
});

test('older browsers without intersection observation keep a static fallback', () => {
  const env = setup({ noIntersection: true });
  assert.equal(env.state.draws, 1);
  assert.equal(env.state.frames.size, 0);
});
