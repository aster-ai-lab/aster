;(() => {
  const documentId = new URLSearchParams(location.hash.slice(1)).get('aix-document')
  if (!/^[a-f0-9]{64}$/.test(documentId || '')) return
  const channel = new MessageChannel()
  const input = document.querySelector('input')
  const output = document.querySelector('output')
  let commands = new Set()
  let count = 0
  let stored = 0
  let revision = 0
  let tree = false
  channel.port1.onmessage = ({ data }) => {
    if (data.protocolVersion !== 1) return
    if (data.type === 'aix.view.connect') {
      input.value = data.stateJSON === null ? '' : (JSON.parse(data.stateJSON).query ?? '')
    } else if (data.type === 'aix.tree.connect') {
      tree = true
      channel.port1.postMessage({ protocolVersion: 1, type: 'aix.tree.ready' })
    } else if (data.type === 'aix.tree.request') {
      channel.port1.postMessage({
        protocolVersion: 1,
        type: 'aix.tree.data',
        id: data.id,
        nodes: [{ id: 'tasks', parent: null, label: `Tasks ${count}`, hasChildren: false }]
      })
    } else if (data.type === 'aix.command.connect') {
      commands = new Set(data.commands)
      channel.port1.postMessage({ protocolVersion: 1, type: 'aix.command.ready' })
    } else if (data.type === 'aix.command.execute') {
      const supported = commands.has(data.command) && ['refresh', 'clear'].includes(data.command)
      if (supported) {
        count = data.command === 'clear' ? 0 : count + 1
        output.textContent = `${data.command}:${count}`
        if (tree) channel.port1.postMessage({ protocolVersion: 1, type: 'aix.tree.changed', revision: ++revision })
      }
      channel.port1.postMessage({
        protocolVersion: 1,
        type: supported ? 'aix.command.completed' : 'aix.command.refused',
        id: data.id,
        command: data.command
      })
    }
  }
  channel.port1.start()
  window.parent.postMessage({ protocolVersion: 1, type: 'aix.document.ready', documentId }, '*', [channel.port2])
  input.addEventListener('input', () =>
    channel.port1.postMessage({
      protocolVersion: 1,
      type: 'aix.view.store',
      id: ++stored,
      stateJSON: JSON.stringify({ query: input.value })
    })
  )
  window.addEventListener('pagehide', () => channel.port1.close(), { once: true })
})()
