function basename(value, suffix) {
  const text = String(value).replace(/[\\/]+$/, '')
  const name = text.slice(text.lastIndexOf('/') + 1).slice(text.lastIndexOf('\\') + 1)
  return suffix && name.endsWith(suffix) ? name.slice(0, -suffix.length) : name
}

function dirname(value) {
  const text = String(value)
  const index = Math.max(text.lastIndexOf('/'), text.lastIndexOf('\\'))
  return index < 0 ? '.' : text.slice(0, index) || '/'
}

function extname(value) {
  const name = basename(value)
  const index = name.lastIndexOf('.')
  return index <= 0 ? '' : name.slice(index)
}

function join(...parts) {
  return parts
    .flat()
    .filter((part) => part !== '' && part != null)
    .join('/')
    .replace(/\\/g, '/')
    .replace(/\/{2,}/g, '/')
}

function resolve(...parts) {
  const joined = join(...parts)
  return joined.startsWith('/') ? joined : `/${joined}`
}

function parse(value) {
  const text = String(value)
  const dir = dirname(text)
  const base = basename(text)
  const ext = extname(text)
  return { root: text.startsWith('/') ? '/' : '', dir, base, ext, name: base.slice(0, base.length - ext.length) }
}

const path = {
  sep: '/',
  delimiter: ':',
  basename,
  dirname,
  extname,
  join,
  resolve,
  normalize: (value) => String(value),
  isAbsolute: (value) => String(value).startsWith('/'),
  relative: () => '',
  parse,
  format: (entry) => join(entry.dir || '', `${entry.name || ''}${entry.ext || ''}`)
}
path.posix = path
path.win32 = path

module.exports = path
