;(() => {
  let activePort
  let sequence = 0
  const documentId = new URLSearchParams(location.hash.slice(1)).get('aix-document')
  if (!/^[a-f0-9]{64}$/.test(documentId || '')) return
  const channel = new MessageChannel()
  const input = document.querySelector('input')
  const output = document.querySelector('output')
  const applyEnvironment = (environment) => {
    if (!environment) return
    const root = document.documentElement
    root.lang = environment.locale
    root.dir = environment.direction
    root.dataset.theme = environment.theme
    root.style.colorScheme = environment.theme
    for (const [name, value] of Object.entries(environment.tokens)) root.style.setProperty(name, value)
    root.style.backgroundColor = 'var(--background)'
    root.style.color = 'var(--foreground)'
  }
  channel.port1.onmessage = (event) => {
    if (event.data?.protocolVersion !== 1 || event.data?.type !== 'aix.view.connect' || activePort) return
    activePort = channel.port1
    sequence = 0
    const restored = event.data.stateJSON === null ? null : JSON.parse(event.data.stateJSON)
    input.value = restored?.query ?? ''
    applyEnvironment(event.data.environment)
    activePort.onmessage = ({ data }) => {
      if (data.protocolVersion !== 1) return
      if (data.type === 'aix.view.environment') applyEnvironment(data.environment)
      if (data.type === 'aix.view.stored' && data.id === sequence) output.textContent = 'Saved'
    }
    activePort.start()
  }
  channel.port1.start()
  window.parent.postMessage({ protocolVersion: 1, type: 'aix.document.ready', documentId }, '*', [channel.port2])
  input.addEventListener('input', () => {
    activePort?.postMessage({
      protocolVersion: 1,
      type: 'aix.view.store',
      id: ++sequence,
      stateJSON: JSON.stringify({ query: input.value })
    })
  })
  window.addEventListener('pagehide', () => channel.port1.close(), { once: true })
})()
