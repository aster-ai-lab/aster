export function chromeImportAllowed(platform: string | undefined): boolean {
  return platform === 'darwin' || platform === 'win32' || platform === 'win64'
}
