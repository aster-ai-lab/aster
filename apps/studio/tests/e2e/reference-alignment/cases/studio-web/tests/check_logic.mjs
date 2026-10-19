// Execute generated scripts only in the owned, networkless VM, never on the host.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { mock } from 'node:test'

import { JSDOM, VirtualConsole } from 'jsdom'

assert.equal(process.platform, 'linux')
assert.ok(existsSync('/run/a3s-studio-reference-web-logic'))
const results = []

function check(number, exercise) {
  const source = readFileSync(`/app/F${String(number).padStart(2, '0')}.html`, 'utf8')
  const errors = []
  const virtualConsole = new VirtualConsole()
  virtualConsole.on('jsdomError', (error) => errors.push(error.message))
  mock.timers.enable({ apis: ['setTimeout', 'setInterval'], now: 0 })
  const dom = new JSDOM(source, {
    runScripts: 'dangerously',
    url: 'https://synthetic.example.test/',
    virtualConsole,
    beforeParse(window) {
      window.HTMLElement.prototype.scrollIntoView = () => {}
    }
  })
  try {
    exercise(dom.window, (seconds = 1) => {
      for (let i = 0; i < seconds; i++) mock.timers.tick(1000)
    })
    assert.deepEqual(errors, [])
    results.push({
      case: `F${String(number).padStart(2, '0')}`,
      status: 'passed',
      sha256: createHash('sha256').update(source).digest('hex')
    })
  } finally {
    dom.window.close()
    mock.timers.reset()
  }
}

check(5, (window, tick) => {
  const doc = window.document
  const start = doc.getElementById('btnStart')
  const pause = doc.getElementById('btnPause')
  const reset = doc.getElementById('btnReset')
  const time = () => doc.getElementById('time').textContent
  assert.equal(time(), '25:00')
  start.click()
  tick(2)
  assert.equal(time(), '24:58')
  start.click()
  start.click()
  tick()
  assert.equal(time(), '24:57')
  pause.click()
  tick(5)
  assert.equal(time(), '24:57')
  start.click()
  tick()
  assert.equal(time(), '24:56')
  reset.click()
  tick(3)
  assert.equal(time(), '25:00')
})

check(6, (window) => {
  const doc = window.document
  const email = doc.getElementById('testEmail')
  doc.getElementById('heroCta').click()
  assert.equal(doc.activeElement, email)
  email.value = 'synthetic@example.test'
  const event = new window.Event('submit', { bubbles: true, cancelable: true })
  doc.getElementById('expForm').dispatchEvent(event)
  assert.ok(event.defaultPrevented)
  const confirmation = doc.getElementById('confirm')
  assert.ok(confirmation.textContent.includes(email.value))
  assert.notEqual(window.getComputedStyle(confirmation).display, 'none')
  assert.ok(confirmation.textContent.includes('未向任何服务器发送数据'))
})

check(7, (window) => {
  const doc = window.document
  const visible = (id) => window.getComputedStyle(doc.getElementById(id)).display !== 'none'
  const tasks = () => doc.getElementById('taskList').children.length
  assert.ok(visible('view-home'))
  assert.equal(tasks(), 2)
  const first = doc.querySelector('#taskList input[type=checkbox]')
  first.checked = true
  first.dispatchEvent(new window.Event('change', { bubbles: true }))
  doc.querySelector('#taskList button').click()
  assert.ok(visible('view-detail'))
  assert.ok(!visible('view-home'))
  doc.getElementById('detailBack').click()
  assert.ok(visible('view-home'))
  doc.getElementById('toSettings').click()
  assert.ok(visible('view-settings'))
  const onlyUndone = doc.getElementById('onlyUndone')
  onlyUndone.checked = true
  onlyUndone.dispatchEvent(new window.Event('change', { bubbles: true }))
  doc.getElementById('settingsBack').click()
  assert.ok(visible('view-home'))
  assert.equal(tasks(), 1)
  assert.ok(!doc.querySelector('#taskList input[type=checkbox]').checked)
  doc.getElementById('toSettings').click()
  assert.ok(onlyUndone.checked)
})

check(8, (window) => {
  const links = [...window.document.querySelectorAll('a[href^="#"]')]
  assert.ok(links.length >= 3)
  for (const link of links) {
    link.click()
    mock.timers.tick(1)
    assert.equal(window.location.hash, link.hash)
    assert.ok(window.document.getElementById(decodeURIComponent(link.hash.slice(1))))
  }
})

process.stdout.write(
  JSON.stringify(
    {
      schema: 'a3s.studio.html-logic-check.v1',
      results,
      scope: 'DOM logic in jsdom 26.1.0 with Node mocked timers; not a browser or native UI pass',
      limitations: [
        'layout and scrolling not simulated',
        'form native constraint validation not exercised',
        'selectors grounded in this frozen artifact revision'
      ]
    },
    null,
    2
  ) + '\n'
)
