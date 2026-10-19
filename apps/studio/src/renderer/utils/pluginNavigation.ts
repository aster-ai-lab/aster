const PAGE_KEY = /^[a-z][a-z0-9_.-]{0,127}\/[a-z][a-z0-9_.-]{0,127}$/

export function isPluginPageKey(value: unknown): value is string {
  return typeof value === 'string' && PAGE_KEY.test(value)
}

export function pluginPageUrl(key: string): string {
  if (!isPluginPageKey(key)) throw new Error('Invalid plugin page identity')
  return `/app/plugin?page=${encodeURIComponent(key)}`
}

export function parsePluginPageSearch(search: Record<string, unknown>): { page?: string } {
  return isPluginPageKey(search.page) ? { page: search.page } : {}
}
