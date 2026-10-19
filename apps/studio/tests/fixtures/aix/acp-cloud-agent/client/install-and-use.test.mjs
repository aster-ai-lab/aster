import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

test('client installs a session and uses two streamed turns', async () => {
  const script = fileURLToPath(new URL('./install-and-use.mjs', import.meta.url))
  const child = spawn(process.execPath, [script], { stdio: ['ignore', 'pipe', 'pipe'] })
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8')
  child.stderr.setEncoding('utf8')
  child.stdout.on('data', (chunk) => {
    stdout += chunk
  })
  child.stderr.on('data', (chunk) => {
    stderr += chunk
  })
  const code = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill()
      reject(new Error(`client timed out\n${stdout}\n${stderr}`))
    }, 5000)
    child.on('exit', (status) => {
      clearTimeout(timer)
      resolve(status)
    })
  })
  assert.equal(stderr, '')
  assert.equal(code, 0)
  assert.equal(stdout, '收到：第一句\n上一轮是第一句。这一轮是第二句\n')
})
