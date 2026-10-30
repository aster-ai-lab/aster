const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../public/cosmic.js'), 'utf8');

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

function webGLContext(state, options) {
  return {
    VERTEX_SHADER: 1, FRAGMENT_SHADER: 2, COMPILE_STATUS: 3, LINK_STATUS: 4,
    ARRAY_BUFFER: 5, STATIC_DRAW: 6, DYNAMIC_DRAW: 7, FLOAT: 8, TRIANGLES: 9, POINTS: 10,
    BLEND: 11, SRC_ALPHA: 12, ONE: 13,
    createShader: () => ({}),
    shaderSource() {},
    compileShader() {},
    getShaderParameter: () => !options.failShaders,
    createProgram: () => ({}),
    attachShader() {},
    linkProgram() {},
    getProgramParameter: () => true,
    createBuffer: () => ({}),
    bindBuffer() {},
    bufferData(_target, data) { state.bufferData.push(data); },
    vertexAttribPointer(...args) { state.attributes.push(args); },
    enableVertexAttribArray() {},
    getAttribLocation(_program, name) { return { aPosition: 0, aSize: 1, aColor: 2, aLayer: 3 }[name]; },
    getUniformLocation: (_program, name) => name,
    uniform1f(name, ...values) { state.uniforms[name] = values; },
    uniform2f(name, ...values) { state.uniforms[name] = values; },
    uniform3f(name, ...values) { state.uniforms[name] = values; },
    viewport(...args) { state.viewport = args; },
    disable() {},
    enable() {},
    blendFunc() {},
    useProgram() {},
    drawArrays(mode, first, count) { state.webglDraws++; state.drawCalls.push({ mode, first, count }); }
  };
}

function setup(options = {}) {
  const state = { draws: 0, dimensionWrites: 0, transforms: [], arcs: [], gradients: 0, frames: new Map(), nextId: 1, bufferData: [], drawCalls: [], webglDraws: 0, viewport: null, attributes: [], uniforms: {} };
  const bounds = { width: 1200, height: 900, top: 0, left: 0, ...options.bounds };
  const section = { ...eventTarget(), getBoundingClientRect: () => ({ ...bounds }) };
  const context = Object.fromEntries(['save', 'restore', 'beginPath', 'stroke', 'moveTo', 'lineTo', 'fill'].map(name => [name, () => {}]));
  Object.assign(context, {
    clearRect() { state.draws++; },
    fillRect() {},
    setTransform(...args) { state.transforms.push(args); },
    arc(...args) { state.arcs.push(args); },
    createLinearGradient() { state.gradients++; return { addColorStop() {} }; },
    createRadialGradient() { state.gradients++; return { addColorStop() {} }; }
  });
  const gl = webGLContext(state, options);
  const canvas = {
    dataset: {},
    closest: selector => selector === '.download' ? section : null,
    parentElement: {},
    getBoundingClientRect: () => ({ ...bounds }),
    getContext(type) {
      if (options.contextThrows) throw new Error('unsupported canvas');
      if (options.noContext) return null;
      return type === 'webgl' && options.webgl ? gl : context;
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
    observe(element) { assert.equal(element, section); }
  };
  globals.ResizeObserver = class {
    constructor(callback) { state.resize = callback; }
    observe(element) { assert.equal(element, canvas.parentElement); }
  };
  vm.runInNewContext(source, globals, { timeout: 1000 });
  return {
    state, bounds, section, document, window, motion, canvas,
    intersect(value) { state.intersection([{ isIntersecting: value }]); },
    advance(now) {
      const callbacks = [...state.frames.values()];
      state.frames.clear();
      callbacks.forEach(callback => callback(now));
    }
  };
}

test('missing or unavailable cosmic canvas keeps the fallback without scheduling work', () => {
  for (const options of [{ missingCanvas: true }, { noContext: true }, { contextThrows: true }]) {
    const { state, section } = setup(options);
    assert.equal(state.frames.size, 0);
    assert.equal(state.dimensionWrites, 0);
    assert.equal(section.listeners.size, 0);
  }
});

test('cosmic DPR is clamped at two and unchanged resize does not reset the canvas', () => {
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

test('reduced motion draws a static cosmos and never starts an animation loop', () => {
  const env = setup({ reduced: true });
  env.intersect(true);
  assert.equal(env.state.draws, 1);
  assert.ok(env.state.gradients >= 2);
  assert.equal(env.state.frames.size, 0);
  env.section.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'mouse' });
  assert.equal(env.state.draws, 1);
  env.motion.matches = false;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 1);
  env.motion.matches = true;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 0);
});

test('cosmic painting is capped at thirty FPS and pauses for visibility and intersection', () => {
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

test('cosmic page cache lifecycle stops and resumes without a size change', () => {
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

test('cosmic pointer parallax is small and touch does not move the star field', () => {
  const env = setup();
  env.intersect(true);
  env.advance(0);
  const before = env.state.arcs.at(-1);
  assert.equal(env.section.listeners.get('pointermove')[0].options.passive, true);
  env.section.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'mouse', preventDefault() { assert.fail('must remain passive'); } });
  env.advance(100);
  const after = env.state.arcs.at(-1);
  assert.notDeepEqual(after, before);
  const control = setup();
  control.intersect(true);
  control.advance(0);
  control.advance(100);
  const neutral = control.state.arcs.at(-1);
  assert.ok(Math.abs(after[0] - neutral[0]) < 2);
  assert.ok(Math.abs(after[1] - neutral[1]) < 2);
  const touch = setup();
  touch.intersect(true);
  touch.advance(0);
  touch.section.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'touch' });
  touch.advance(100);
  assert.deepEqual(touch.state.arcs.at(-1), control.state.arcs.at(-1));
});

test('cosmic gathers and expands smoothly in an endless cycle', () => {
  const env = setup();
  env.intersect(true);
  const sampleRadius = () => {
    const arcs = env.state.arcs;
    assert.ok(arcs.length > 400);
    const radius = arcs.reduce((sum, arc) => sum + Math.hypot(arc[0] - 600, (arc[1] - 450) / 0.42), 0) / arcs.length;
    arcs.length = 0;
    return radius;
  };
  env.advance(0);
  const initial = sampleRadius();
  const radii = [];
  for (let now = 64; now <= 20480; now += 64) {
    env.advance(now);
    if (now % 1024 === 0) radii.push(sampleRadius());
  }
  assert.equal(radii.length, 20);
  const first = radii.slice(0, 10);
  const second = radii.slice(10);
  const range = values => ({ min: Math.min(...values), max: Math.max(...values) });
  const one = range(first);
  const two = range(second);
  assert.ok(one.max > initial * 1.1, JSON.stringify({ initial, one, two }));
  assert.ok(one.min < initial * 0.85);
  assert.ok(Math.abs(two.max - one.max) / one.max < 0.03);
  assert.ok(Math.abs(two.min - one.min) / one.min < 0.03);
});

test('older browsers without intersection observation keep a static cosmos', () => {
  const env = setup({ noIntersection: true });
  assert.equal(env.state.draws, 1);
  assert.equal(env.state.frames.size, 0);
});

test('WebGL supplies three-dimensional star layers and a galaxy with clamped DPR', () => {
  const env = setup({ webgl: true, dpr: 3 });
  env.intersect(true);
  env.advance(0);
  assert.deepEqual(env.state.viewport, [0, 0, 2400, 1800]);
  assert.ok(env.state.drawCalls.at(-1).count > 400);
  const vertices = env.state.bufferData.at(-1);
  assert.equal(vertices.length % 9, 0);
  const depths = [];
  const layers = new Map();
  for (let index = 0; index < vertices.length; index += 9) {
    assert.ok([...vertices.slice(index, index + 9)].every(Number.isFinite));
    depths.push(vertices[index + 2]);
    const layer = vertices[index + 8];
    layers.set(layer, (layers.get(layer) || 0) + 1);
  }
  assert.ok(Math.min(...depths) < -180);
  assert.ok(Math.max(...depths) > -10);
  assert.deepEqual([...layers.keys()].sort(), [0, 1, 2, 3]);
  assert.ok(env.state.attributes.some(([location, size, , , stride]) => location === 0 && size === 3 && stride === 36));
  assert.deepEqual(env.state.uniforms.uRatio, [2]);
  assert.equal(env.state.frames.size, 1);
});

test('WebGL reduced motion renders once and toggles the animation lifecycle', () => {
  const env = setup({ webgl: true, reduced: true });
  env.intersect(true);
  assert.equal(env.state.webglDraws, 2);
  assert.equal(env.state.frames.size, 0);
  env.motion.matches = false;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 1);
  env.motion.matches = true;
  env.motion.dispatch('change');
  assert.equal(env.state.frames.size, 0);
});

test('WebGL camera parallax changes with mouse input while touch stays neutral', () => {
  const env = setup({ webgl: true });
  env.intersect(true);
  env.advance(0);
  const before = [...env.state.uniforms.uCamera];
  const field = [...env.state.bufferData.at(-1)];
  env.section.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'mouse' });
  env.advance(100);
  const after = [...env.state.uniforms.uCamera];
  assert.notDeepEqual(after, before);
  assert.deepEqual([...env.state.bufferData.at(-1)], field);
  const touch = setup({ webgl: true });
  touch.intersect(true);
  touch.advance(0);
  touch.section.dispatch('pointermove', { clientX: 1200, clientY: 900, pointerType: 'touch' });
  touch.advance(100);
  const control = setup({ webgl: true });
  control.intersect(true);
  control.advance(0);
  control.advance(100);
  assert.deepEqual([...touch.state.uniforms.uCamera], [...control.state.uniforms.uCamera]);
  assert.ok(after[0] > control.state.uniforms.uCamera[0]);
  assert.ok(after[1] > control.state.uniforms.uCamera[1]);
});

test('galaxy stars cluster around two logarithmic arms at rest', () => {
  const env = setup({ webgl: true, reduced: true });
  env.intersect(true);
  const vertices = env.state.bufferData.at(-1);
  let galaxy = 0;
  let aligned = 0;
  for (let index = 0; index < vertices.length; index += 9) {
    if (vertices[index + 8] !== 0) continue;
    galaxy++;
    const x = vertices[index];
    const z = vertices[index + 2];
    const phase = Math.sin(2 * Math.atan2(z, x) - 3.4 * Math.log(Math.hypot(x, z)));
    if (phase > 0.25) aligned++;
  }
  assert.ok(galaxy > 400);
  assert.ok(aligned / galaxy > 0.65);
});

test('unavailable WebGL shaders retain the existing Canvas fallback', () => {
  const env = setup({ webgl: true, failShaders: true });
  env.intersect(true);
  env.advance(0);
  assert.equal(env.canvas.dataset.renderer, 'canvas');
  assert.ok(env.state.draws > 0);
  assert.equal(env.state.webglDraws, 0);
});
