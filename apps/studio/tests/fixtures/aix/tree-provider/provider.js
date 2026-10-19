;(() => {
  let activePort
  const documentId = new URLSearchParams(location.hash.slice(1)).get('aix-document')
  if (!/^[a-f0-9]{64}$/.test(documentId || '')) return
  const channel = new MessageChannel()
  channel.port1.onmessage = (event) => {
    if (event.data?.protocolVersion !== 1 || event.data?.type !== 'aix.tree.connect' || activePort) return
    const port = channel.port1
    activePort = port
    let sequence = 0
    let refreshed = 0
    let nodes = [{ id: 'tasks', parent: null, label: 'Tasks', hasChildren: true }]
    port.onmessage = ({ data }) => {
      if (
        activePort !== port ||
        data.protocolVersion !== 1 ||
        data.type !== 'aix.tree.request' ||
        !Number.isSafeInteger(data.id) ||
        data.id <= sequence
      )
        return
      sequence = data.id
      if (data.action === 'refresh') nodes[0].description = `Refresh ${++refreshed}`
      if (data.action === 'expand' && data.nodeId === 'tasks' && nodes.length === 1) {
        nodes.push({ id: 'task-1', parent: 'tasks', label: 'First task', hasChildren: false, command: 'open-task' })
      }
      if (data.action === 'select') document.body.dataset.selected = data.nodeId
      if (data.action === 'invoke' && data.nodeId === 'task-1' && data.command === 'open-task') {
        document.body.dataset.opened = 'task-1'
        nodes[1].description = 'Opened'
      }
      port.postMessage({ protocolVersion: 1, type: 'aix.tree.data', id: data.id, nodes })
    }
    port.start()
    port.postMessage({ protocolVersion: 1, type: 'aix.tree.ready' })
  }
  channel.port1.start()
  window.parent.postMessage({ protocolVersion: 1, type: 'aix.document.ready', documentId }, '*', [channel.port2])
  window.addEventListener('pagehide', () => channel.port1.close(), { once: true })
})()
