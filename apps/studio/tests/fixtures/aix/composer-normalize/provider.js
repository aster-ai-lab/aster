const documentId = new URLSearchParams(location.hash.slice(1)).get('aix-document')
if (!/^[a-f0-9]{64}$/.test(documentId || '')) throw new Error('Missing AIX admission')
const channel = new MessageChannel()
let connected = false
let used = false
channel.port1.onmessage = ({ data }) => {
  if (!connected && data?.protocolVersion === 1 && data.type === 'aix.composer.connect') {
    connected = true
    channel.port1.postMessage({ protocolVersion: 1, type: 'aix.composer.ready' })
  } else if (
    connected &&
    !used &&
    data?.protocolVersion === 1 &&
    data.type === 'aix.composer.request' &&
    data.id === 1 &&
    Number.isSafeInteger(data.revision) &&
    data.revision >= 0 &&
    typeof data.text === 'string'
  ) {
    used = true
    channel.port1.postMessage({
      protocolVersion: 1,
      type: 'aix.composer.result',
      id: data.id,
      revision: data.revision,
      text: data.text.trim().replace(/\s+/g, ' ')
    })
  } else channel.port1.close()
}
channel.port1.start()
window.parent.postMessage({ protocolVersion: 1, type: 'aix.document.ready', documentId }, '*', [channel.port2])
window.addEventListener('pagehide', () => channel.port1.close(), { once: true })
