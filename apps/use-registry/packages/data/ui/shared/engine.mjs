/**
 * Database statements for the data plugin. aster-orm owns the drivers:
 * SQLite and PostgreSQL. The Studio window sends the engine and target
 * to the host command. A Node process runs that same library.
 */

const COMMAND = 'data_sqlite_query'

function tauriInvoke() {
  const internals = globalThis.__TAURI_INTERNALS__
  if (!internals || typeof internals.invoke !== 'function') return null
  return internals.invoke
}

const HELPER = 'native/data-sqlite/target/debug/data-sqlite'

async function helperProgram(fileURLToPath) {
  const override = typeof process === 'object' ? process.env?.ASTER_DATA_SQLITE_BIN : ''
  if (typeof override === 'string' && override) return override
  const { dirname, resolve } = await import(/* @vite-ignore */ 'node:path')
  const moduleUrl = import.meta.url
  if (typeof moduleUrl === 'string' && moduleUrl.startsWith('file:') && typeof fileURLToPath === 'function') {
    return resolve(dirname(fileURLToPath(moduleUrl)), '../../../../../studio', HELPER)
  }
  return resolve(process.cwd(), HELPER)
}

/** The body of the `data_sqlite_query` host command: one aster-orm driver, one statement. */
export async function runDataSqliteCommand(path, sql, engine) {
  let spawn
  let fileURLToPath
  try {
    const childProcess = await import(/* @vite-ignore */ 'node:child_process')
    const url = await import(/* @vite-ignore */ 'node:url')
    spawn = childProcess.spawn
    fileURLToPath = url.fileURLToPath
  } catch {
    spawn = null
  }
  if (typeof spawn !== 'function' || typeof fileURLToPath !== 'function') {
    throw new Error('SQLite file is unavailable')
  }
  const program = await helperProgram(fileURLToPath)
  return await new Promise((resolve, reject) => {
    const child = spawn(program, [], { stdio: ['pipe', 'pipe', 'pipe'] })
    let out = ''
    let err = ''
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      out += chunk
    })
    child.stderr.on('data', (chunk) => {
      err += chunk
    })
    child.on('error', () => reject(new Error('SQLite file is unavailable')))
    child.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(err.trim() || 'SQLite file is unavailable'))
        return
      }
      try {
        resolve(JSON.parse(out))
      } catch {
        reject(new Error('SQLite file is unavailable'))
      }
    })
    child.stdin.end(JSON.stringify({ path, sql, engine: engine || 'sqlite' }))
  })
}

const DRIVERS = {
  sqlite: "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
  postgres:
    "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY table_name"
}

function driverName(target) {
  const engine = typeof target?.engine === 'string' ? target.engine.trim() : ''
  return engine || 'sqlite'
}

/** One preview statement. The table name is a catalog identifier, quoted so it stays an identifier. */
export function sqlSelectAll(table) {
  const name = String(table).replaceAll('"', '""')
  return `SELECT * FROM "${name}" LIMIT 200`
}

async function queryOnHost(path, sql, engine) {
  const invoke = tauriInvoke()
  if (invoke) return invoke(COMMAND, { path, sql, engine })
  return runDataSqliteCommand(path, sql, engine)
}

export async function executeSqlite(target, sql) {
  const text = String(sql ?? '').trim()
  const path = typeof target?.path === 'string' ? target.path.trim() : ''
  const engine = driverName(target)
  if (!text) throw new Error('SQL 语句是空的')
  if (!path) throw new Error(engine === 'postgres' ? '连接地址是空的' : '数据库文件是空的')
  const grid = await queryOnHost(path, text, engine)
  return {
    columns: Array.isArray(grid?.columns) ? grid.columns : [],
    rows: Array.isArray(grid?.rows) ? grid.rows : []
  }
}

export async function listSqliteTables(target) {
  const engine = driverName(target)
  const sql = DRIVERS[engine]
  if (typeof sql !== 'string') {
    await executeSqlite(target, 'SELECT 1')
    return []
  }
  const result = await executeSqlite(target, sql)
  return result.rows.map((row) => String(row[0] ?? ''))
}

/** Sum of finite numbers in the rows. With none, the aggregate is the row count. */
export function analysisAggregate(rows) {
  const numbers = []
  for (const row of rows) {
    for (const cell of row) {
      if (typeof cell === 'number' && Number.isFinite(cell)) numbers.push(cell)
      else if (typeof cell === 'bigint') numbers.push(Number(cell))
    }
  }
  if (numbers.length === 0) return rows.length
  return numbers.reduce((sum, value) => sum + value, 0)
}
