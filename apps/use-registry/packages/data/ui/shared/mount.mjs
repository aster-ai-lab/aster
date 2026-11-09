import { analysisAggregate, executeSqlite, listSqliteTables, sqlSelectAll } from './engine.mjs'
import { admittedEngines } from './engines.mjs'
import {
  connectionForSource,
  readDataCatalog,
  saveDataConnection,
  saveDataSource,
  subscribeDataCatalog
} from './catalog.mjs'

const FIELD = 'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none'

function icon(paths) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  svg.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  svg.setAttribute('width', '16')
  svg.setAttribute('height', '16')
  svg.setAttribute('viewBox', '0 0 24 24')
  svg.setAttribute('fill', 'none')
  svg.setAttribute('stroke', 'currentColor')
  svg.setAttribute('stroke-width', '1.7')
  svg.setAttribute('stroke-linecap', 'round')
  svg.setAttribute('stroke-linejoin', 'round')
  svg.setAttribute('data-design', 'icon')
  svg.innerHTML = paths
  return svg
}

function playIcon() {
  return icon('<polygon points="6 3 20 12 6 21 6 3"></polygon>')
}

function plusIcon() {
  return icon('<path d="M5 12h14"></path><path d="M12 5v14"></path>')
}

function databaseIcon() {
  return icon(
    '<ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M3 5v14c0 1.7 4 3 9 3s9-1.3 9-3V5"></path><path d="M3 12c0 1.7 4 3 9 3s9-1.3 9-3"></path>'
  )
}

function tableIcon() {
  return icon(
    '<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M3 9h18"></path><path d="M3 15h18"></path><path d="M12 3v18"></path>'
  )
}

const WORKSPACE_STYLE = `
[data-ui="data.browser"], [data-ui="data.analysis"] { display: flex; flex: 1; flex-direction: column; height: 100%; min-height: 0; position: relative; overflow: hidden; }
.dataws-toolbar { display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 12px; border-bottom: 1px solid #efefef; flex: none; }
.dataws-spacer { flex: 1; }
.dataws-title { margin: 0; font-size: 14px; font-weight: 600; line-height: 1; }
.dataws-muted { margin: 0; font-size: 12px; line-height: 16px; }
.dataws-tool { height: 32px; border: 1px solid #e5e5e5; background: #ffffff; border-radius: 8px; }
.dataws-tool:hover { background: #fafafa; }
.dataws-run { margin-left: 4px; border: 0; border-radius: 8px; }
.dataws-shell { display: flex; flex: 1; min-height: 0; }
.dataws-side { width: 260px; flex: none; display: flex; flex-direction: column; border-right: 1px solid #efefef; background: #ffffff; }
.dataws-search { margin: 8px 12px; width: calc(100% - 24px); height: 30px; }
.dataws-tree { flex: 1; min-height: 0; overflow: auto; padding: 0 8px 8px; }
.dataws-branch { margin: 0 0 2px 18px; padding-left: 10px; border-left: 1px solid #efefef; }
.dataws-row { width: 100%; height: 36px; justify-content: flex-start; border: 0; border-radius: 8px; background: transparent; }
.dataws-row:hover { background: #f7f7f7; }
.dataws-meta { margin-left: auto; color: #8a8a8a; font-size: 12px; font-weight: 400; }
.dataws-empty { margin: 24px 8px; color: #8a8a8a; font-size: 13px; text-align: center; }
.dataws-source { padding: 12px; border-top: 1px solid #efefef; display: flex; flex-direction: column; gap: 8px; }
.dataws-label { margin: 0; color: #8a8a8a; font-size: 12px; line-height: 16px; }
.dataws-field { display: flex; flex-direction: column; gap: 6px; }
.dataws-inline { display: flex; align-items: center; gap: 8px; }
.dataws-inline input { flex: 1; min-width: 0; }
.dataws-main { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: #ffffff; }
.dataws-tabs { display: flex; align-items: flex-end; gap: 4px; height: 36px; padding: 0 12px; border-bottom: 1px solid #efefef; flex: none; }
.dataws-tab { height: 28px; padding: 0 10px; border: 0; border-radius: 6px 6px 0 0; background: #f2f3f5; color: #171717; font-size: 13px; font-weight: 500; }
.dataws-editor { display: flex; flex: none; min-height: 120px; background: #f5f5f5; }
.dataws-gutter { width: 44px; flex: none; padding: 10px 8px; text-align: right; color: #8a8a8a; font-size: 12px; line-height: 20px; font-family: var(--code-font-family, ui-monospace), monospace; }
.dataws-sql { flex: 1; min-width: 0; height: 100%; border: 0; resize: none; padding: 10px 12px; font-size: 13px; line-height: 20px; font-family: var(--code-font-family, ui-monospace), monospace; }
.dataws-sql::placeholder { color: #c5c5c5; }
.dataws-split { height: 8px; flex: none; cursor: row-resize; background: #ffffff; position: relative; }
.dataws-split::after { content: ""; position: absolute; left: 0; right: 0; top: 3px; height: 1px; background: #efefef; }
.dataws-split:hover::after { background: #171717; }
.dataws-grid { flex: 1; min-height: 0; overflow: auto; background: #ffffff; }
.dataws-grid table { border-collapse: collapse; width: 100%; font-size: 13px; font-family: var(--code-font-family, ui-monospace), monospace; }
.dataws-grid th { position: sticky; top: 0; height: 36px; background: #ffffff; color: #8a8a8a; font-size: 12px; font-weight: 400; text-align: left; padding: 0 10px; border-bottom: 1px solid #efefef; }
.dataws-grid td { height: 36px; padding: 0 10px; border-bottom: 1px solid #efefef; color: #3f3f3f; }
.dataws-grid tr:hover td { background: #f7f7f7; }
.dataws-num { width: 48px; text-align: right; color: #8a8a8a; }
.dataws-blank { height: 100%; min-height: 160px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; color: #8a8a8a; font-size: 13px; text-align: center; padding: 64px 24px; }
.dataws-blank strong { color: #171717; font-size: 14px; font-weight: 500; }
.dataws-status { height: 32px; flex: none; display: flex; align-items: center; gap: 12px; padding: 0 12px; border-top: 1px solid #efefef; color: #8a8a8a; font-size: 12px; }
.dataws-status .dataws-count { margin-left: auto; }
.dataws-alert { margin: 0; color: #b42318; font-size: 12px; line-height: 16px; }
.dataws-scrim { position: absolute; inset: 0; z-index: 20; display: flex; align-items: center; justify-content: center; padding: 24px; background: rgba(0, 0, 0, 0.20); }
.dataws-card { width: 512px; max-width: 100%; display: flex; flex-direction: column; gap: 16px; padding: 24px; border-radius: 12px; background: #ffffff; box-shadow: 0 12px 32px rgba(0, 0, 0, 0.12); }
.dataws-card h2 { margin: 0; font-size: 15px; font-weight: 500; color: #171717; }
.dataws-actions { display: flex; justify-content: flex-end; gap: 8px; }
.dataws-quiet { height: 36px; padding: 0 14px; border: 1px solid #e5e5e5; border-radius: 8px; background: #ffffff; }
.dataws-quiet:hover { background: #fafafa; }
.dataws-filled { height: 36px; padding: 0 14px; border: 0; border-radius: 8px; }
.dataws-page input:focus-visible, .dataws-page select:focus-visible, .dataws-page textarea:focus-visible, .dataws-page button:focus-visible { box-shadow: inset 0 0 0 2px rgba(0, 0, 0, 0.10); outline: none; }
.dataws-page input::placeholder { color: #8a8a8a; }
.dataws-select { appearance: none; padding-right: 32px; background-image: linear-gradient(45deg, transparent 50%, #8a8a8a 50%), linear-gradient(135deg, #8a8a8a 50%, transparent 50%); background-position: calc(100% - 16px) 16px, calc(100% - 11px) 16px; background-size: 5px 5px, 5px 5px; background-repeat: no-repeat; }
`

function ensureWorkspaceStyle() {
  let style = document.getElementById('aster-data-workspace')
  if (!style) {
    style = document.createElement('style')
    style.id = 'aster-data-workspace'
    document.head.append(style)
  }
  if (style.textContent !== WORKSPACE_STYLE) style.textContent = WORKSPACE_STYLE
}

function targetFor(connection) {
  const engine = typeof connection.engine === 'string' ? connection.engine.trim() : ''
  return { path: connection.path, engine: engine || 'sqlite' }
}

async function chooseSqlitePath() {
  const open = globalThis.window?.api?.file?.open
  if (typeof open !== 'function') return ''
  const chosen = await open({ filters: [{ name: 'SQLite', extensions: ['sqlite', 'db'] }] })
  return typeof chosen?.filePath === 'string' ? chosen.filePath.trim() : ''
}

function engineSelect(value, onChange) {
  const select = document.createElement('select')
  select.className = `${FIELD} dataws-select`
  select.style.backgroundColor = '#f5f5f5'
  select.style.color = '#171717'
  select.setAttribute('aria-label', '数据库引擎')
  for (const { id, label } of admittedEngines) {
    const option = document.createElement('option')
    option.value = id
    option.textContent = label
    select.append(option)
  }
  select.value = value
  select.addEventListener('change', () => onChange(select.value))
  return select
}

function field(label, value, onInput, design, placeholder) {
  const input = document.createElement('input')
  input.type = 'text'
  input.className = FIELD
  input.style.backgroundColor = '#f5f5f5'
  input.style.color = '#171717'
  input.setAttribute('aria-label', label)
  input.value = value
  if (placeholder) input.placeholder = placeholder
  if (design) input.setAttribute('data-design', design)
  input.addEventListener('input', () => onInput(input.value))
  return input
}

function fieldGroup(caption, control) {
  const group = document.createElement('div')
  group.className = 'dataws-field'
  const text = document.createElement('label')
  text.className = 'dataws-label'
  text.textContent = caption
  if (control.id) text.htmlFor = control.id
  group.append(text, control)
  return group
}

function button(label, onClick, pressed, tone) {
  const node = document.createElement('button')
  node.type = 'button'
  node.textContent = label
  node.setAttribute('aria-label', label)
  node.className = 'inline-flex h-9 items-center gap-2 rounded-[8px] px-2.5 text-[13px] text-[#171717]'
  node.style.color = '#171717'
  if (pressed) {
    node.className += ' bg-[#f2f3f5]'
    node.style.backgroundColor = '#f2f3f5'
    node.setAttribute('aria-current', 'page')
    node.setAttribute('data-design', 'selected')
  }
  if (tone === 'quiet') node.classList.add('dataws-quiet')
  if (tone === 'filled') {
    node.classList.add('dataws-filled')
    node.style.backgroundColor = '#171717'
    node.style.color = '#ffffff'
  }
  node.addEventListener('click', () => {
    void onClick()
  })
  return node
}

function toolButton(label, glyph, onClick) {
  const node = button(label, onClick, false)
  node.className += ' dataws-tool'
  node.prepend(glyph)
  return node
}

function rowButton(label, glyph, onClick, pressed) {
  const node = button(label, onClick, pressed)
  node.className += ' dataws-row'
  node.prepend(glyph)
  return node
}

export function mount(root, kind) {
  const state = {
    connectionName: '',
    connectionPath: '',
    connectionEngine: 'sqlite',
    sourceName: '',
    selectedConnectionId: null,
    selectedSourceId: null,
    sql: 'SELECT ',
    tables: [],
    openTable: null,
    result: null,
    error: null,
    connectionDialog: readDataCatalog().connections.length === 0,
    editorHeight: 240,
    treeQuery: '',
    focusSearch: null
  }
  let renderToken = 0

  const unsubscribe = subscribeDataCatalog(() => {
    render()
  })

  function selectedConnection() {
    return readDataCatalog().connections.find((item) => item.id === state.selectedConnectionId) ?? null
  }

  async function refreshTables(connection) {
    state.tables = connection ? await listSqliteTables(targetFor(connection)) : []
  }

  async function onSaveConnection() {
    const token = ++renderToken
    try {
      const connection = saveDataConnection({
        name: state.connectionName,
        path: state.connectionPath,
        engine: state.connectionEngine
      })
      state.selectedConnectionId = connection.id
      state.connectionName = ''
      state.connectionPath = ''
      state.connectionEngine = 'sqlite'
      state.openTable = null
      state.result = null
      state.error = null
      state.connectionDialog = false
      await refreshTables(connection)
    } catch (error) {
      state.error = error instanceof Error ? error.message : '查询失败'
    }
    if (token === renderToken) render()
  }

  async function onSelectConnection(id) {
    const token = ++renderToken
    state.selectedConnectionId = id
    state.openTable = null
    state.result = null
    state.error = null
    try {
      await refreshTables(selectedConnection())
    } catch (error) {
      state.tables = []
      state.error = error instanceof Error ? error.message : '查询失败'
    }
    if (token === renderToken) render()
  }

  function onSaveSource() {
    try {
      const source = saveDataSource({ name: state.sourceName, connectionId: state.selectedConnectionId ?? '' })
      state.selectedSourceId = source.id
      state.sourceName = ''
      state.error = null
    } catch (error) {
      state.error = error instanceof Error ? error.message : '查询失败'
    }
    render()
  }

  async function onChooseFile() {
    const open = globalThis.window?.api?.file?.open
    if (typeof open !== 'function') {
      const file = root.querySelector('.dataws-file')
      if (file instanceof HTMLInputElement) file.click()
      return
    }
    const token = ++renderToken
    try {
      const path = await chooseSqlitePath()
      if (path) {
        state.connectionPath = path
        state.error = null
      }
    } catch (error) {
      state.error = error instanceof Error ? error.message : '查询失败'
    }
    if (token === renderToken) render()
  }

  function onNewQuery() {
    state.openTable = null
    state.sql = 'SELECT '
    state.result = null
    state.error = null
    render()
  }

  async function onOpenTable(name) {
    state.openTable = name
    state.sql = sqlSelectAll(name)
    await onRun()
  }

  async function onRun() {
    const token = ++renderToken
    const connection = kind === 'analysis' ? connectionForSource(state.selectedSourceId ?? '') : selectedConnection()
    if (!connection) {
      state.result = null
      state.error = kind === 'analysis' ? '先选择数据源' : '先选择连接'
      if (token === renderToken) render()
      return
    }
    try {
      state.result = await executeSqlite(targetFor(connection), state.sql)
      state.error = null
    } catch (error) {
      state.result = null
      state.error = error instanceof Error ? error.message : '查询失败'
    }
    if (token === renderToken) render()
  }

  function alertLine() {
    const error = document.createElement('p')
    error.setAttribute('role', 'alert')
    error.className = 'dataws-alert'
    error.textContent = state.error
    return error
  }

  function sqlEditor() {
    const wrap = document.createElement('div')
    wrap.className = 'dataws-editor'
    wrap.style.height = `${state.editorHeight}px`
    const gutter = document.createElement('div')
    gutter.className = 'dataws-gutter'
    const count = Math.max(String(state.sql).split('\n').length, 1)
    for (let line = 1; line <= count; line += 1) {
      const number = document.createElement('div')
      number.textContent = String(line)
      gutter.append(number)
    }
    const input = document.createElement('textarea')
    input.className = 'dataws-sql w-full border-0 bg-[#f5f5f5] text-[13px] text-[#171717] outline-none'
    input.style.backgroundColor = '#f5f5f5'
    input.style.color = '#171717'
    input.setAttribute('aria-label', 'SQL')
    input.setAttribute('data-design', 'field')
    input.placeholder = 'SELECT * FROM …'
    input.spellcheck = false
    input.value = state.sql
    input.addEventListener('input', () => {
      state.sql = input.value
      const nextCount = Math.max(String(state.sql).split('\n').length, 1)
      if (gutter.childElementCount !== nextCount) {
        gutter.replaceChildren()
        for (let line = 1; line <= nextCount; line += 1) {
          const number = document.createElement('div')
          number.textContent = String(line)
          gutter.append(number)
        }
      }
    })
    input.addEventListener('keydown', (event) => {
      if (event.key !== 'Enter' || (!event.metaKey && !event.ctrlKey)) return
      event.preventDefault()
      void onRun()
    })
    wrap.append(gutter, input)
    return wrap
  }

  function editorSplitter() {
    const bar = document.createElement('div')
    bar.className = 'dataws-split'
    bar.setAttribute('role', 'separator')
    bar.setAttribute('aria-orientation', 'horizontal')
    bar.setAttribute('aria-label', '调整编辑区高度')
    bar.addEventListener('pointerdown', (event) => {
      if (!(event instanceof PointerEvent)) return
      event.preventDefault()
      const startY = event.clientY
      const start = state.editorHeight
      const move = (ev) => {
        state.editorHeight = Math.min(520, Math.max(120, start + ev.clientY - startY))
        const editor = root.querySelector('.dataws-editor')
        if (editor instanceof HTMLElement) editor.style.height = `${state.editorHeight}px`
      }
      const up = () => {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', up)
      }
      window.addEventListener('pointermove', move)
      window.addEventListener('pointerup', up)
    })
    return bar
  }

  function resultGrid() {
    const wrap = document.createElement('div')
    wrap.className = 'dataws-grid'
    if (!state.result) {
      const empty = document.createElement('div')
      empty.className = 'dataws-blank'
      const title = document.createElement('strong')
      title.textContent = state.error ? '查询没有完成' : '运行 SQL 查看结果'
      const hint = document.createElement('p')
      hint.className = 'dataws-empty'
      hint.textContent = state.error
        ? state.error
        : kind === 'analysis'
          ? '选择数据源后运行当前语句。'
          : '从左侧打开一张表，或编写 SELECT 语句后运行。'
      empty.append(title, hint)
      wrap.append(empty)
      return wrap
    }
    const table = document.createElement('table')
    table.setAttribute('data-ui', 'data.result.grid')
    const head = document.createElement('tr')
    const indexHead = document.createElement('th')
    indexHead.className = 'dataws-num'
    indexHead.textContent = '#'
    head.append(indexHead)
    for (const column of state.result.columns) {
      const cell = document.createElement('th')
      cell.textContent = column
      head.append(cell)
    }
    const thead = document.createElement('thead')
    thead.append(head)
    const body = document.createElement('tbody')
    state.result.rows.forEach((row, index) => {
      const line = document.createElement('tr')
      const indexCell = document.createElement('td')
      indexCell.className = 'dataws-num'
      indexCell.textContent = String(index + 1)
      line.append(indexCell)
      for (const cell of row) {
        const node = document.createElement('td')
        node.setAttribute('data-ui', 'data.result.cell')
        node.textContent = cell === null || cell === undefined ? '—' : String(cell)
        line.append(node)
      }
      body.append(line)
    })
    table.append(thead, body)
    wrap.append(table)
    if (state.result.rows.length === 0) {
      const empty = document.createElement('p')
      empty.className = 'dataws-empty'
      empty.textContent = '没有结果'
      wrap.append(empty)
    }
    return wrap
  }

  function closeConnectionDialog() {
    state.connectionDialog = false
    state.error = null
    render()
  }

  function statusBar() {
    const bar = document.createElement('div')
    bar.className = 'dataws-status'
    const hint = document.createElement('p')
    hint.setAttribute('data-design', 'muted')
    hint.className = 'dataws-muted text-[#8a8a8a]'
    hint.style.color = '#8a8a8a'
    const connection = kind === 'analysis' ? connectionForSource(state.selectedSourceId ?? '') : selectedConnection()
    hint.textContent = connection
      ? connection.name
      : kind === 'analysis'
        ? '分析结果'
        : '用已连接的数据库查询表和记录'
    const count = document.createElement('span')
    count.className = 'dataws-count'
    count.textContent = state.result ? `${state.result.rows.length} 行` : connection ? '未运行' : ''
    bar.append(hint, count)
    if (kind === 'analysis' && state.result) {
      const label = document.createElement('span')
      label.textContent = '合计'
      const figure = document.createElement('span')
      figure.setAttribute('data-ui', 'data.analysis.figure')
      figure.className = 'text-[13px] text-[#171717]'
      figure.style.color = '#171717'
      figure.textContent = String(analysisAggregate(state.result.rows))
      bar.append(label, figure)
    }
    if (state.error && !(kind === 'browser' && state.connectionDialog)) bar.append(alertLine())
    return bar
  }

  function connectionDialog() {
    const scrim = document.createElement('div')
    scrim.className = 'dataws-scrim'
    scrim.addEventListener('click', (event) => {
      if (event.target === scrim) closeConnectionDialog()
    })
    const card = document.createElement('div')
    card.className = 'dataws-card'
    card.setAttribute('role', 'dialog')
    card.setAttribute('aria-modal', 'true')
    card.setAttribute('aria-label', '新建连接')
    card.tabIndex = -1
    card.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        closeConnectionDialog()
      }
    })
    const heading = document.createElement('h2')
    heading.textContent = '新建连接'
    const engine = engineSelect(state.connectionEngine, (value) => {
      state.connectionEngine = admittedEngines.some((item) => item.id === value) ? value : 'sqlite'
      render()
    })
    engine.id = 'data-engine'
    const name = field('连接名称', state.connectionName, (value) => {
      state.connectionName = value
    }, 'field', '例如 门店库')
    name.id = 'data-connection-name'
    const sqlite = state.connectionEngine !== 'postgres'
    const targetLabel = sqlite ? '数据库文件' : '连接地址'
    const target = field(targetLabel, state.connectionPath, (value) => {
      state.connectionPath = value
    }, undefined, sqlite ? 'SQLite 文件路径' : 'localhost:5432/app')
    target.id = 'data-connection-target'
    const targetControl = document.createElement('div')
    targetControl.className = 'dataws-inline'
    targetControl.append(target)
    if (sqlite) {
      const file = document.createElement('input')
      file.type = 'file'
      file.accept = '.sqlite,.db,application/vnd.sqlite3'
      file.hidden = true
      file.tabIndex = -1
      file.setAttribute('aria-hidden', 'true')
      file.className = 'dataws-file'
      file.addEventListener('change', () => {
        const chosen = file.files?.[0]
        const nativePath = chosen && typeof chosen.path === 'string' ? chosen.path.trim() : ''
        if (!nativePath) return
        state.connectionPath = nativePath
        render()
      })
      const choose = button('选择数据库文件', onChooseFile, false, 'quiet')
      targetControl.append(choose, file)
    }
    const actions = document.createElement('div')
    actions.className = 'dataws-actions'
    actions.append(button('取消', closeConnectionDialog, false, 'quiet'))
    actions.append(button('保存连接', onSaveConnection, false, 'filled'))
    const targetGroup = fieldGroup(targetLabel, targetControl)
    const targetCaption = targetGroup.querySelector('label')
    if (targetCaption) targetCaption.htmlFor = target.id
    card.append(heading, fieldGroup('数据库引擎', engine), fieldGroup('连接名称', name), targetGroup, actions)
    if (state.error) card.append(alertLine())
    scrim.append(card)
    return scrim
  }

  function browserSide(catalog) {
    const side = document.createElement('aside')
    side.className = 'dataws-side'
    const search = document.createElement('input')
    search.type = 'search'
    search.className = `${FIELD} dataws-search`
    search.style.backgroundColor = '#f5f5f5'
    search.style.color = '#171717'
    search.setAttribute('aria-label', '搜索')
    search.setAttribute('data-design', 'field')
    search.placeholder = '搜索连接或表'
    search.value = state.treeQuery
    search.addEventListener('input', () => {
      state.treeQuery = search.value
      state.focusSearch = search.selectionStart
      render()
    })
    side.append(search)

    const tree = document.createElement('div')
    tree.className = 'dataws-tree'
    tree.setAttribute('aria-label', '表')
    const query = state.treeQuery.trim()
    const connections = catalog.connections.filter((item) => {
      if (!query) return true
      if (item.name.includes(query)) return true
      return item.id === state.selectedConnectionId && state.tables.some((table) => table.includes(query))
    })
    if (connections.length === 0) {
      const empty = document.createElement('p')
      empty.className = 'dataws-empty'
      empty.textContent = catalog.connections.length === 0 ? '还没有连接' : '没有匹配的连接'
      tree.append(empty)
    }
    for (const connection of connections) {
      const row = rowButton(
        connection.name,
        databaseIcon(),
        () => onSelectConnection(connection.id),
        connection.id === state.selectedConnectionId
      )
      const meta = document.createElement('span')
      meta.className = 'dataws-meta'
      meta.textContent = connection.engine === 'postgres' ? 'PostgreSQL' : 'SQLite'
      row.append(meta)
      tree.append(row)
      if (connection.id !== state.selectedConnectionId) continue
      const branch = document.createElement('div')
      branch.className = 'dataws-branch'
      const shown = !query || connection.name.includes(query)
        ? state.tables
        : state.tables.filter((table) => table.includes(query))
      for (const table of shown) {
        branch.append(rowButton(table, tableIcon(), () => onOpenTable(table), table === state.openTable))
      }
      if (shown.length === 0) {
        const empty = document.createElement('p')
        empty.className = 'dataws-empty'
        empty.textContent = '没有表'
        branch.append(empty)
      }
      tree.append(branch)
    }
    side.append(tree)

    const source = document.createElement('div')
    source.className = 'dataws-source'
    const label = document.createElement('p')
    label.className = 'dataws-label'
    label.textContent = '数据源'
    source.append(label)
    const sourceName = field('数据源名称', state.sourceName, (value) => {
      state.sourceName = value
    }, undefined, '例如 销售')
    sourceName.id = 'data-source-name'
    source.append(fieldGroup('数据源名称', sourceName))
    source.append(button('保存数据源', onSaveSource, false, 'quiet'))
    side.append(source)
    return side
  }

  function analysisSide(catalog) {
    const side = document.createElement('aside')
    side.className = 'dataws-side'
    const label = document.createElement('p')
    label.className = 'dataws-label'
    label.textContent = '数据源'
    label.style.padding = '8px'
    side.append(label)
    const tree = document.createElement('div')
    tree.className = 'dataws-tree'
    if (catalog.sources.length === 0) {
      const empty = document.createElement('p')
      empty.className = 'dataws-empty'
      empty.textContent = '还没有数据源'
      tree.append(empty)
    }
    for (const source of catalog.sources) {
      tree.append(rowButton(source.name, databaseIcon(), () => {
        state.selectedSourceId = source.id
        render()
      }, source.id === state.selectedSourceId))
    }
    side.append(tree)
    return side
  }

  function render() {
    ensureWorkspaceStyle()
    const catalog = readDataCatalog()
    const page = document.createElement('section')
    page.setAttribute('data-design', 'page')
    page.setAttribute('data-ui', kind === 'analysis' ? 'data.analysis' : 'data.browser')
    page.className = 'dataws-page flex w-full flex-col bg-[#ffffff] text-[#171717]'
    page.style.backgroundColor = '#ffffff'
    page.style.color = '#171717'

    const toolbar = document.createElement('div')
    toolbar.className = 'dataws-toolbar'
    const title = document.createElement('h1')
    title.setAttribute('data-design', 'ink')
    title.className = 'dataws-title text-[#171717]'
    title.style.color = '#171717'
    title.textContent = kind === 'analysis' ? '数据分析' : '数据'
    const spacer = document.createElement('div')
    spacer.className = 'dataws-spacer'
    toolbar.append(title, spacer)
    if (kind === 'browser') {
      toolbar.append(toolButton('新建连接', plusIcon(), () => {
        state.connectionDialog = true
        state.error = null
        render()
      }))
      toolbar.append(toolButton('新建查询', plusIcon(), onNewQuery))
    }
    const run = toolButton('运行', playIcon(), onRun)
    run.classList.add('dataws-run')
    run.style.backgroundColor = '#171717'
    run.style.color = '#ffffff'
    toolbar.append(run)

    const shell = document.createElement('div')
    shell.className = 'dataws-shell'
    shell.append(kind === 'browser' ? browserSide(catalog) : analysisSide(catalog))
    const main = document.createElement('div')
    main.className = 'dataws-main'
    const tabs = document.createElement('div')
    tabs.className = 'dataws-tabs'
    tabs.setAttribute('role', 'tablist')
    tabs.setAttribute('aria-label', '查询')
    const tab = document.createElement('div')
    tab.className = 'dataws-tab'
    tab.setAttribute('role', 'tab')
    tab.setAttribute('aria-selected', 'true')
    tab.textContent = state.openTable ? state.openTable : '查询'
    tabs.append(tab)
    main.append(tabs, sqlEditor(), editorSplitter(), resultGrid(), statusBar())
    shell.append(main)
    page.append(toolbar, shell)
    if (kind === 'browser' && state.connectionDialog) page.append(connectionDialog())

    root.replaceChildren(page)
    if (typeof state.focusSearch === 'number') {
      const caret = state.focusSearch
      state.focusSearch = null
      const next = root.querySelector('[aria-label="搜索"]')
      if (next instanceof HTMLInputElement) {
        next.focus()
        next.setSelectionRange(caret, caret)
      }
    }
  }

  root.style.display = 'flex'
  root.style.flexDirection = 'column'
  root.style.flex = '1 1 auto'
  root.style.height = '100%'
  root.style.minHeight = '0'
  render()
  return () => {
    unsubscribe()
    root.replaceChildren()
  }
}
