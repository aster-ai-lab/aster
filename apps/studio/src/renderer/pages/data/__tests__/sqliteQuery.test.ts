import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { executeSqlite, listSqliteTables, sqlSelectAll } from '../sqliteQuery'
import { installDataSqliteInvoke, readDataSqliteCalls, readDataSqliteInvocations } from './dataSqliteInvoke'

describe('executeSqlite', () => {
  const directories: string[] = []
  let restoreInvoke = () => {}

  beforeEach(() => {
    restoreInvoke = installDataSqliteInvoke()
  })

  afterEach(() => {
    restoreInvoke()
    for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
  })

  it('returns the inserted rows on two reads and lists the created table', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'studio-sqlite-'))
    directories.push(directory)
    const target = { path: join(directory, 'shop.sqlite') }

    await executeSqlite(target, 'CREATE TABLE sales (sku TEXT, amount INTEGER)')
    await executeSqlite(target, "INSERT INTO sales (sku, amount) VALUES ('pen', 10), ('book', 32)")

    const tables = await listSqliteTables(target)
    const first = await executeSqlite(target, 'SELECT sku, amount FROM sales ORDER BY sku')
    const second = await executeSqlite(target, 'SELECT sku, amount FROM sales ORDER BY sku')
    console.log(JSON.stringify({ invoked: readDataSqliteInvocations(), tables, first, second }))

    expect(tables).toContain('sales')
    expect(first).toEqual(second)
    expect(first.columns).toEqual(['sku', 'amount'])
    expect(first.rows).toEqual([
      ['book', 32],
      ['pen', 10]
    ])
    expect(second.rows).toEqual([
      ['book', 32],
      ['pen', 10]
    ])
  })

  it('quotes a catalog table name and reads that preview', async () => {
    expect(sqlSelectAll('odd"name')).toBe('SELECT * FROM "odd""name" LIMIT 200')
    const directory = mkdtempSync(join(tmpdir(), 'studio-sqlite-preview-'))
    directories.push(directory)
    const target = { path: join(directory, 'shop.sqlite') }
    await executeSqlite(target, 'CREATE TABLE sales (sku TEXT, amount INTEGER)')
    await executeSqlite(target, "INSERT INTO sales (sku, amount) VALUES ('pen', 10), ('book', 32)")
    const preview = await executeSqlite(target, sqlSelectAll('sales'))
    expect(preview.columns).toEqual(['sku', 'amount'])
    expect(preview.rows).toEqual([
      ['pen', 10],
      ['book', 32]
    ])
  })

  it('sends PostgreSQL to a3s-orm and refuses an engine a3s-orm does not ship', async () => {
    await expect(executeSqlite({ path: '  ' }, 'SELECT 1')).rejects.toThrow('数据库文件是空的')
    await expect(executeSqlite({ path: '  ', engine: 'postgres' }, 'SELECT 1')).rejects.toThrow('连接地址是空的')
    await expect(executeSqlite({ path: 'unused', engine: 'mysql' }, 'SELECT 1')).rejects.toThrow(
      'a3s-orm has no MySQL driver'
    )
    expect(readDataSqliteCalls().at(-1)).toMatchObject({
      command: 'data_sqlite_query',
      engine: 'mysql',
      sql: 'SELECT 1'
    })
    await expect(executeSqlite({ path: 'not-a-postgres-url', engine: 'postgres' }, 'SELECT 1')).rejects.toThrow(
      'invalid PostgreSQL connection string'
    )
    expect(readDataSqliteCalls().at(-1)).toMatchObject({
      command: 'data_sqlite_query',
      engine: 'postgres',
      sql: 'SELECT 1'
    })
  })
})
