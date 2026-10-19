function parse(input) {
  const value = new URL(String(input), 'http://localhost')
  return {
    protocol: value.protocol,
    slashes: true,
    auth: '',
    host: value.host,
    hostname: value.hostname,
    port: value.port,
    pathname: value.pathname,
    path: `${value.pathname}${value.search}`,
    search: value.search,
    hash: value.hash,
    href: value.href
  }
}

module.exports = {
  parse,
  format: (entry) => (entry && entry.href) || '',
  resolve: (from, to) => new URL(to, from).href,
  URL: globalThis.URL,
  URLSearchParams: globalThis.URLSearchParams,
  fileURLToPath: (value) => String(value).replace(/^file:\/\//, ''),
  pathToFileURL: (value) => new URL(`file://${value}`)
}
