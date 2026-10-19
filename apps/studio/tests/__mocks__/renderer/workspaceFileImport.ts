/** HTTP boundary fixture for Knowledge document imports. Native tests prove disk authority. */
export function workspaceFileImportFixture(root: string, directories: readonly string[]) {
  const stored = new Map<string, Uint8Array>()
  let refusal = 0
  let release: (() => void) | undefined
  let pending: Promise<void> | undefined
  return {
    stored,
    refuseNext(status: number) {
      refusal = status
    },
    holdNext() {
      pending = new Promise<void>((resolve) => {
        release = resolve
      })
      return () => release?.()
    },
    async fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
      const url = new URL(String(input))
      if (url.searchParams.get('root') !== root) return new Response(null, { status: 403 })
      if (url.pathname === '/api/local-documents/entries') {
        const parent = url.searchParams.get('path') || root
        const entries = directories
          .filter((path) => path.slice(0, path.lastIndexOf('/')) === parent)
          .map((path) => ({
            name: path.slice(path.lastIndexOf('/') + 1),
            path,
            kind: 'directory',
            sizeBytes: 0,
            modifiedMs: 0
          }))
        for (const [path, bytes] of stored) {
          if (path.slice(0, path.lastIndexOf('/')) === parent)
            entries.push({
              name: path.slice(path.lastIndexOf('/') + 1),
              path,
              kind: 'file',
              sizeBytes: bytes.byteLength,
              modifiedMs: 0
            })
        }
        return new Response(JSON.stringify({ entries, capped: false }))
      }
      if (url.pathname !== '/api/workspace-file/import' || init?.method !== 'POST')
        return new Response(null, { status: 404 })
      if (pending) {
        const wait = pending
        pending = undefined
        await wait
      }
      if (refusal) {
        const status = refusal
        refusal = 0
        return new Response(null, { status })
      }
      const directory = url.searchParams.get('directory')
      const name = url.searchParams.get('name')
      if (!name || !directory || (directory !== root && !directories.includes(directory)))
        return new Response(null, { status: 403 })
      const path = `${directory}/${name}`
      if (stored.has(path) && url.searchParams.get('replace') === 'false') return new Response(null, { status: 409 })
      const body = init?.body
      if (typeof body === 'string' || !(body instanceof Uint8Array)) return new Response(null, { status: 400 })
      stored.set(path, new Uint8Array(await new Response(body).arrayBuffer()))
      return new Response(JSON.stringify({ path }), { status: 201 })
    }
  }
}
