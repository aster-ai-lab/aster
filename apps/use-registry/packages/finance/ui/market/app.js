const SYMBOLS = [
  { id: 'Aster', name: 'Aster', seed: 11, start: 128 },
  { id: 'BTC', name: 'BTC', seed: 29, start: 64000 },
  { id: 'ETH', name: 'ETH', seed: 47, start: 3200 }
]

const HOUR = 60 * 60 * 1000
const ORIGIN = Date.UTC(2026, 0, 1)

function series(seed) {
  let state = seed >>> 0
  return () => {
    state = (Math.imul(1664525, state) + 1013904223) >>> 0
    return state / 4294967296
  }
}

function barsFor(symbol) {
  const next = series(symbol.seed)
  let price = symbol.start
  const bars = []
  for (let index = 0; index < 160; index += 1) {
    const open = price
    const drift = (next() - 0.48) * open * 0.012
    const close = Math.max(1, open + drift)
    const high = Math.max(open, close) * (1 + next() * 0.004)
    const low = Math.min(open, close) * (1 - next() * 0.004)
    bars.push({
      time: ORIGIN + index * HOUR,
      open,
      high,
      low,
      close,
      volume: Math.round(1000 + next() * 9000)
    })
    price = close
  }
  return bars
}

function money(value) {
  return value.toLocaleString('zh-CN', { maximumFractionDigits: 2 })
}

let theme = 'dark'

function show(symbol, nextTheme) {
  if (nextTheme === 'light' || nextTheme === 'dark') theme = nextTheme
  const bars = barsFor(symbol)
  const first = bars[0]
  const last = bars[bars.length - 1]
  const high = bars.reduce((max, bar) => Math.max(max, bar.high), last.high)
  const low = bars.reduce((min, bar) => Math.min(min, bar.low), last.low)
  const change = ((last.close - first.open) / first.open) * 100
  document.getElementById('summary').textContent =
    `${symbol.name}  收盘 ${money(last.close)}  涨跌 ${change.toFixed(2)}%  高 ${money(high)}  低 ${money(low)}`
  for (const button of document.querySelectorAll('#symbols button')) {
    button.setAttribute('aria-pressed', button.dataset.symbol === symbol.id ? 'true' : 'false')
  }
  const host = document.getElementById('chart')
  host.replaceChildren()
  if (!window.Vela || typeof window.Vela.Vela !== 'function') {
    const note = document.createElement('p')
    note.textContent = 'Vela 图表没有加载。'
    host.append(note)
    return
  }
  new window.Vela.Vela(host, { data: bars, timeframe: '1h', theme, symbol: symbol.name })
}

function applyConfig(values) {
  if (!values || typeof values !== 'object') return
  const requested = String(values.symbol || '')
    .trim()
    .toUpperCase()
  const symbol = SYMBOLS.find((item) => item.id === requested) || SYMBOLS[0]
  const next = values.theme === 'light' || values.theme === 'dark' ? values.theme : theme
  show(symbol, next)
}

const symbols = document.getElementById('symbols')
for (const symbol of SYMBOLS) {
  const button = document.createElement('button')
  button.type = 'button'
  button.dataset.symbol = symbol.id
  button.textContent = symbol.name
  button.addEventListener('click', () => show(symbol, theme))
  symbols.append(button)
}

window.addEventListener('message', (event) => {
  const data = event.data
  if (!data || data.type !== 'aster.plugin.config') return
  applyConfig(data.values)
})

show(SYMBOLS[0])
