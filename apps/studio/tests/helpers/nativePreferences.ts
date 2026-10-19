import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'

/** Real Rust store over test stdio; event delivery is a controlled transport, not a WebView proof. */
export function nativePreferenceTransport(executable: string, root: string) {
  const child = spawn(
    executable,
    ['--exact', 'preferences::tests::native_preference_stdio', '--ignored', '--nocapture'],
    {
      env: { ...process.env, A3S_CLOUD_TEST_PREFERENCE_ROOT: root },
      stdio: ['pipe', 'pipe', 'pipe']
    }
  )
  let nextId = 0
  const requests = new Map<number, { resolve: (value: any) => void; reject: (error: any) => void }>()
  let stderr = ''
  child.stderr.on('data', (bytes) => {
    stderr += String(bytes)
  })
  const lines = createInterface({ input: child.stdout })
  lines.on('line', (line) => {
    const marker = 'A3S_PREFERENCE_PROOF '
    const start = line.indexOf(marker)
    if (start < 0) return
    const response = JSON.parse(line.slice(start + marker.length))
    const request = requests.get(response.id)
    if (!request) return
    requests.delete(response.id)
    if (response.ok) request.resolve(response.data)
    else request.reject(response.error)
  })
  const fail = (error: Error) => {
    for (const request of requests.values()) request.reject(error)
    requests.clear()
  }
  child.on('error', fail)
  child.on('exit', (code) => fail(new Error(`Native preference proof exited ${code}: ${stderr}`)))
  const request = (action: string, input: Record<string, unknown> = {}) =>
    new Promise<any>((resolve, reject) => {
      const id = ++nextId
      requests.set(id, { resolve, reject })
      child.stdin.write(JSON.stringify({ id, action, ...input }) + '\n')
    })
  return {
    request,
    close: async () => {
      if (child.exitCode !== null) return
      const finished = new Promise<void>((resolve) => child.once('exit', () => resolve()))
      child.stdin.end()
      await finished
      lines.close()
    }
  }
}
