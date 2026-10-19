/** Directory name of the default application workspace, under the user home. */
export const APP_WORKSPACE_DIRECTORY = '.a3s'

function separatorOf(value: string): '\\' | '/' {
  return value.includes('\\') && !value.includes('/') ? '\\' : '/'
}

/** Drop trailing slashes. A filesystem root stays recognizable. */
export function normalizeWorkspacePath(value: string): string {
  const trimmed = value.trim()
  if (trimmed === '/' || trimmed === '\\' || /^[A-Za-z]:[\\/]?$/.test(trimmed)) {
    return trimmed.replace(/[\\/]+$/, '') || trimmed
  }
  return trimmed.replace(/[\\/]+$/, '')
}

export function isAbsoluteWorkspacePath(value: string): boolean {
  return value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value)
}

export function isFilesystemRoot(value: string): boolean {
  const path = normalizeWorkspacePath(value)
  return path === '/' || path === '\\' || /^[A-Za-z]:$/.test(path)
}

/** `~` and `~/...` use the known home directory. Anything else is unchanged. */
export function expandWorkspacePath(value: string, homePath: string): string {
  const trimmed = value.trim()
  if (trimmed === '~') return normalizeWorkspacePath(homePath)
  if (trimmed.startsWith('~/') || trimmed.startsWith('~\\')) {
    const home = normalizeWorkspacePath(homePath)
    if (!home) return trimmed
    return `${home}${trimmed.slice(1)}`
  }
  return trimmed
}

/** Home directory plus `.a3s`. An empty home yields an empty path. */
export function defaultAppWorkspacePath(homePath: string): string {
  const home = normalizeWorkspacePath(homePath)
  if (!home || isFilesystemRoot(home)) return ''
  return `${home}${separatorOf(home)}${APP_WORKSPACE_DIRECTORY}`
}

/**
 * Value stored in `app.workspace.path`.
 * The default directory is stored as an empty string.
 * A relative path or a filesystem root is rejected.
 */
export function workspacePreferenceValue(selected: string, homePath: string): string | null {
  const path = normalizeWorkspacePath(selected)
  if (!isAbsoluteWorkspacePath(path) || isFilesystemRoot(path)) return null
  const fallback = normalizeWorkspacePath(defaultAppWorkspacePath(homePath))
  return path === fallback ? '' : path
}

/**
 * A personal corpus can be compiled when it is an absolute directory and it is not the user home.
 * The default `~/.a3s` workspace is allowed. The home directory itself is not.
 */
export function knowledgeCorpusAllowed(root: string, homePath: string): boolean {
  const shown = normalizeWorkspacePath(root)
  const home = normalizeWorkspacePath(homePath)
  if (!shown || !home || !isAbsoluteWorkspacePath(shown) || isFilesystemRoot(shown)) return false
  return shown !== home
}

export function displayedWorkspacePath(stored: string | null | undefined, homePath: string): string {
  const custom = typeof stored === 'string' ? normalizeWorkspacePath(stored) : ''
  if (custom && isAbsoluteWorkspacePath(custom) && !isFilesystemRoot(custom)) return custom
  return defaultAppWorkspacePath(homePath)
}

/**
 * Folder the composer workspace-file picker lists.
 * A selected project wins. With none selected, the panel lists the app workspace.
 */
export function fileMentionRoot(
  projectPath: string | null | undefined,
  workspacePath: string | null | undefined
): string {
  const project = typeof projectPath === 'string' ? normalizeWorkspacePath(projectPath) : ''
  if (project) return project
  return typeof workspacePath === 'string' ? normalizeWorkspacePath(workspacePath) : ''
}
