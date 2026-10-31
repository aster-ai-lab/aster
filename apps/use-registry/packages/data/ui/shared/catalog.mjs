const STORAGE_KEY = 'aster.data.catalog.v1'
const EMPTY = { connections: [], sources: [] }

let catalog = readStored()
const listeners = new Set()

function readStored() {
  if (typeof localStorage === 'undefined') return EMPTY
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return EMPTY
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed.connections) || !Array.isArray(parsed.sources)) return EMPTY
    return { connections: parsed.connections, sources: parsed.sources }
  } catch {
    return EMPTY
  }
}

function publish(next) {
  catalog = next
  if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
  queueMicrotask(() => {
    if (catalog !== next) return
    for (const listener of listeners) listener()
  })
}

export function readDataCatalog() {
  return catalog
}

export function subscribeDataCatalog(listener) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function resetDataCatalogForTests() {
  if (typeof localStorage !== 'undefined') localStorage.removeItem(STORAGE_KEY)
  publish(EMPTY)
}

export function saveDataConnection(input) {
  const name = input.name.trim()
  const path = input.path.trim()
  if (!name || !path) throw new Error('连接需要名称和数据库目标')
  const engine = storedEngine(input.engine)
  const connection = {
    id: crypto.randomUUID(),
    name,
    path,
    engine,
    ...(typeof input.bytesBase64 === 'string' && input.bytesBase64 ? { bytesBase64: input.bytesBase64 } : {})
  }
  publish({ ...catalog, connections: [...catalog.connections, connection] })
  return connection
}

export function saveDataSource(input) {
  const name = input.name.trim()
  const connection = catalog.connections.find((item) => item.id === input.connectionId)
  if (!name || !connection) throw new Error('数据源需要名称和已保存的连接')
  const source = { id: crypto.randomUUID(), name, connectionId: connection.id }
  publish({ ...catalog, sources: [...catalog.sources, source] })
  return source
}

export function connectionForSource(sourceId) {
  const source = catalog.sources.find((item) => item.id === sourceId)
  if (!source) return undefined
  return catalog.connections.find((item) => item.id === source.connectionId)
}

function storedEngine(value) {
  if (typeof value !== 'string') return 'sqlite'
  const engine = value.trim()
  return engine || 'sqlite'
}

export function replaceConnectionBytes(connectionId, bytesBase64) {
  publish({
    ...catalog,
    connections: catalog.connections.map((item) => (item.id === connectionId ? { ...item, bytesBase64 } : item))
  })
}
