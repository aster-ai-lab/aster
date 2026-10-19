/** DetailPageNav only needs the back label. The console message catalog stays out of Studio. */
export function useI18n() {
  return {
    locale: 'zh' as const,
    t: (key: string) => (key === 'shell.back' ? '返回' : key)
  }
}
