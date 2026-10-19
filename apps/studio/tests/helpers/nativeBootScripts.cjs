const assert = require('node:assert/strict')
const { readFileSync } = require('node:fs')
const { JSDOM } = require('jsdom')

const { mode, boot, resume, runtime, development } = JSON.parse(readFileSync(0, 'utf8'))
const trusted = [
  'http://127.0.0.1:9/',
  'http://127.0.0.1:9/index.html',
  'http://127.0.0.1:4318/windows/main/index.html?window=file-editor#editor',
  'tauri://localhost/windows/main/index.html',
  'http://tauri.localhost/windows/main/index.html',
  'https://tauri.localhost/windows/main/index.html'
]
const denied = [
  'http://127.0.0.1:9/aix-view/qualification-token/ui/page.html',
  'http://127.0.0.1:4318/aix-view/qualification-token/ui/page.html',
  'http://127.0.0.1:4318/windows/main/index.html/guest',
  'http://127.0.0.1:4318/guest.html',
  'http://tauri.localhost/aix-view/qualification-token/ui/page.html',
  'tauri://localhost/aix-view/qualification-token/ui/page.html',
  'https://tauri.localhost.evil.invalid/windows/main/index.html',
  'http://127.0.0.1:4319/windows/main/index.html',
  'http://user:password@127.0.0.1:4318/windows/main/index.html',
  'https://external.invalid/windows/main/index.html',
  'about:blank'
]
if (!development) denied.push(trusted.splice(2, 1)[0])

function refuse(window, script, name) {
  const root = window.document.documentElement
  root.style.visibility = 'hidden'
  window.eval(script)
  assert.equal(Object.hasOwn(window, '__A3S_STUDIO_RUNTIME__'), false, name + ': host credential leaked')
  assert.equal(Object.hasOwn(window, '__A3S_STUDIO_DESKTOP__'), false, name + ': desktop capabilities leaked')
  assert.equal(Object.hasOwn(window, '__A3S_WINDOW_DRAG_BOOT__'), false, name + ': native drag bridge installed')
  assert.equal(root.dataset.tenant, undefined, name + ': host tenant effect installed')
  assert.equal(root.style.getPropertyValue('--a3s-traffic-light-inset'), '', name + ': host chrome effect installed')
  assert.equal(root.style.visibility, 'hidden', name + ': host resume changed guest visibility')
}

if (mode === 'trusted' || mode === 'trusted-plain') {
  for (const url of trusted) {
    const document = new JSDOM('<!doctype html><html><body></body></html>', { url, runScripts: 'outside-only' })
    const window = document.window
    window.eval(boot)
    assert.deepEqual(JSON.parse(JSON.stringify(window.__A3S_STUDIO_RUNTIME__)), runtime, url)
    assert.equal(Object.isFrozen(window.__A3S_STUDIO_RUNTIME__), true, url)
    assert.equal(Object.isFrozen(window.__A3S_STUDIO_DESKTOP__), true, url)
    assert.ok(['darwin', 'win32', 'linux'].includes(window.__A3S_STUDIO_DESKTOP__.platform), url)
    assert.equal(
      window.__A3S_STUDIO_DESKTOP__.chrome,
      window.__A3S_STUDIO_DESKTOP__.platform === 'darwin' ? 'overlay' : 'native',
      url
    )
    assert.equal(window.__A3S_STUDIO_DESKTOP__.screenshot, window.__A3S_STUDIO_DESKTOP__.platform === 'darwin', url)
    assert.equal(window.__A3S_STUDIO_DESKTOP__.autoUpdate, window.__A3S_STUDIO_DESKTOP__.platform === 'darwin', url)
    assert.equal(window.__A3S_STUDIO_DESKTOP__.singleInstance, window.__A3S_STUDIO_DESKTOP__.platform !== 'win32', url)
    assert.equal(window.document.documentElement.dataset.tenant, 'toyou', url)
    assert.equal(window.document.documentElement.dataset.a3sProduct, 'ToWork', url)
    assert.equal(
      window.document.documentElement.style.getPropertyValue('--a3s-traffic-light-inset'),
      mode === 'trusted' ? '60px' : '',
      url
    )
    assert.equal(Object.hasOwn(window, '__A3S_WINDOW_DRAG_BOOT__'), mode === 'trusted', url)
    window.document.documentElement.style.visibility = 'hidden'
    const accepted = window.__A3S_STUDIO_RUNTIME__
    window.eval(resume)
    assert.equal(window.__A3S_STUDIO_RUNTIME__, accepted, url + ': resume replaced immutable runtime')
    assert.equal(window.document.documentElement.style.visibility, 'visible', url)
    document.window.close()

    const resumed = new JSDOM('<!doctype html><html style="visibility:hidden"><body></body></html>', {
      url,
      runScripts: 'outside-only'
    })
    resumed.window.eval(resume)
    assert.deepEqual(JSON.parse(JSON.stringify(resumed.window.__A3S_STUDIO_RUNTIME__)), runtime, url)
    assert.equal(resumed.window.document.documentElement.dataset.tenant, 'toyou', url)
    assert.equal(resumed.window.document.documentElement.dataset.a3sProduct, 'ToWork', url)
    assert.equal(resumed.window.document.documentElement.style.visibility, 'visible', url)
    assert.equal(resumed.window.__A3S_WINDOW_DRAG_BOOT__, true, url + ': navigation lost the native drag bootstrap')
    assert.equal(Object.isFrozen(resumed.window.__A3S_STUDIO_DESKTOP__), true, url)
    resumed.window.close()
  }
} else if (mode === 'denied' || mode === 'invalid-runtime') {
  for (const script of [boot, resume]) {
    for (const url of mode === 'denied' ? denied : trusted.concat(denied)) {
      const document = new JSDOM('<!doctype html><html><body></body></html>', { url, runScripts: 'outside-only' })
      refuse(document.window, script, 'guest ' + url)
      document.window.close()
    }
    for (const url of trusted.concat(denied)) {
      const document = new JSDOM('<!doctype html><html><body></body></html>', {
        url: trusted[2],
        runScripts: 'outside-only'
      })
      const frame = document.window.document.createElement('iframe')
      frame.src = url
      document.window.document.body.append(frame)
      frame.contentDocument.open()
      frame.contentDocument.write('<!doctype html><html><body></body></html>')
      frame.contentDocument.close()
      refuse(frame.contentWindow, script, 'nested ' + url)
      document.window.close()
    }
  }
} else {
  throw new Error('Unknown script qualification mode')
}
