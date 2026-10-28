import { spawn } from 'node:child_process'

let host = '127.0.0.1'
let port = '7100'
const args = process.argv.slice(2)

for (let index = 0; index < args.length; index += 1) {
  const argument = args[index]
  if (argument === '--port' || argument === '--host') {
    const value = args[index + 1]
    if (!value) throw new Error(`${argument} requires a value`)
    if (argument === '--port') port = value
    else host = value
    index += 1
    continue
  }
  if (argument.startsWith('--port=')) {
    port = argument.slice('--port='.length)
    continue
  }
  if (argument.startsWith('--host=')) {
    host = argument.slice('--host='.length)
    continue
  }
  throw new Error(`Unsupported dev server argument: ${argument}`)
}

const server = spawn('python3', ['-m', 'http.server', port, '--bind', host, '--directory', 'public'], {
  stdio: 'inherit'
})
server.on('exit', (code) => {
  process.exitCode = code ?? 0
})
process.on('SIGINT', () => server.kill('SIGINT'))
process.on('SIGTERM', () => server.kill('SIGTERM'))
