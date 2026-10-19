/**
 * Product layout under the default app workspace (`~/.a3s` unless configured):
 *
 * - `projects/<name>/` is the project folder
 * - `sessions/<local date>/<session id>/` holds an unassigned task
 * - `projects/<name>/<session id>/` holds a task that was created in, or moved to, that project
 *
 * The directory name is the host-chosen a3s-code session id. Workbench tasks use the
 * agent session id as that id, so there is not a second identifier beside it.
 * `meta/a3s-session.json` records the id and the store directory. It stays out of
 * the store root so FileSessionStore does not read it as a session document.
 * a3s-code snapshots stay in the FileSessionStore layout inside the same directory.
 */

export const WORKSPACE_PROJECTS_DIRECTORY = 'projects'
export const WORKSPACE_SESSIONS_DIRECTORY = 'sessions'
export const CODE_SESSION_FILE = 'a3s-session.json'

export type WorkspaceFiles = {
  mkdir: (dirPath: string) => Promise<unknown>
  write: (filePath: string, data: string) => Promise<unknown>
  moveDir: (dirPath: string, newDirPath: string) => Promise<unknown>
}

export type CodeSessionMove = {
  directory: string
  /** Previous directory, set only when a folder was actually renamed. */
  movedFrom: string | null
}

/** One path segment. Rejects separators, `.`, `..`, and trailing dots or spaces. */
export function workspaceEntryName(value: string): string | null {
  const name = value.trim()
  if (!name || name === '.' || name === '..') return null
  if (/[\\/\u0000-\u001f<>:"|?*]/.test(name)) return null
  if (name.endsWith('.') || name.endsWith(' ')) return null
  return name
}

/** Local calendar day, `YYYY-MM-DD`. */
export function localDateFolder(date: Date): string | null {
  if (Number.isNaN(date.getTime())) return null
  const year = String(date.getFullYear()).padStart(4, '0')
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function separatorOf(value: string): '\\' | '/' {
  return value.includes('\\') && !value.includes('/') ? '\\' : '/'
}

/** Join sanitized segments onto an absolute workspace root. `..` cannot escape it. */
export function joinWorkspace(root: string, ...parts: string[]): string | null {
  const base = root.trim().replace(/[\\/]+$/, '')
  if (!base || base === '.' || base === '..') return null
  const names = parts.map(workspaceEntryName)
  if (names.some((name) => !name)) return null
  const sep = separatorOf(base)
  const target = [base, ...(names as string[])].join(sep)
  if (target !== base && !target.startsWith(`${base}${sep}`)) return null
  return target
}

export function projectDirectory(root: string, name: string): string | null {
  return joinWorkspace(root, WORKSPACE_PROJECTS_DIRECTORY, name)
}

export function inboxSessionDirectory(root: string, sessionId: string, createdAt: Date): string | null {
  const day = localDateFolder(createdAt)
  if (!day) return null
  return joinWorkspace(root, WORKSPACE_SESSIONS_DIRECTORY, day, sessionId)
}

export function projectSessionDirectory(projectPath: string, sessionId: string): string | null {
  return joinWorkspace(projectPath, sessionId)
}

/** A project root plus the session id, or that folder when the path already ends with the id. */
export function locatedSessionDirectory(parentOrFolder: string, sessionId: string): string | null {
  const sep = separatorOf(parentOrFolder)
  if (parentOrFolder.endsWith(`${sep}${sessionId}`)) return parentOrFolder
  return projectSessionDirectory(parentOrFolder, sessionId)
}

function parentDirectory(directory: string): string | null {
  const trimmed = directory.trim().replace(/[\\/]+$/, '')
  const sep = separatorOf(trimmed)
  const index = trimmed.lastIndexOf(sep)
  if (index <= 0) return null
  return trimmed.slice(0, index)
}

/** `moveDir` refuses a destination whose parent directory is missing. */
async function ensureParentDirectory(files: WorkspaceFiles, directory: string): Promise<void> {
  const parent = parentDirectory(directory)
  if (parent) await files.mkdir(parent)
}

export function codeSessionDocument(id: string, store: string, createdAt: string): string {
  return `${JSON.stringify({ runtime: 'a3s-code', id, store, createdAt })}\n`
}

function sessionMetaDirectory(directory: string): string | null {
  return joinWorkspace(directory, 'meta')
}

function sessionFile(directory: string): string | null {
  return joinWorkspace(directory, 'meta', CODE_SESSION_FILE)
}

function moveFailure(error: unknown): 'missing' | 'occupied' | 'rejected' {
  const message = error instanceof Error ? error.message : String(error)
  if (message.includes('does not exist') || message.includes('ENOENT') || message.includes('404')) return 'missing'
  if (message.includes('409') || message.includes('already exists')) return 'occupied'
  return 'rejected'
}

function firstDistinct(directory: string, ...candidates: Array<string | null | undefined>): string | null {
  for (const candidate of candidates) {
    if (candidate && candidate !== directory) return candidate
  }
  return null
}

export async function ensureCodeSessionDirectory(
  files: WorkspaceFiles,
  input: { root: string; sessionId: string; createdAt: string; projectPath?: string | null }
): Promise<string | null> {
  const directory = input.projectPath
    ? projectSessionDirectory(input.projectPath, input.sessionId)
    : inboxSessionDirectory(input.root, input.sessionId, new Date(input.createdAt))
  if (!directory) return null
  const wrote = await writeSessionLocator(files, directory, input.sessionId, input.createdAt)
  return wrote ? directory : null
}

export async function moveCodeSessionDirectory(
  files: WorkspaceFiles,
  input: {
    root: string
    sessionId: string
    createdAt: string
    fromProjectPath: string | null
    toProjectPath: string
    /**
     * Folder the session record already points at. A homepage task lives in
     * `workspace/<session id>`, which is not the dated inbox directory.
     */
    fromFolder?: string | null
  }
): Promise<CodeSessionMove | null> {
  const directory = projectSessionDirectory(input.toProjectPath, input.sessionId)
  if (!directory) return null
  const fromProject = input.fromProjectPath ? locatedSessionDirectory(input.fromProjectPath, input.sessionId) : null
  const recorded = input.fromFolder ? locatedSessionDirectory(input.fromFolder, input.sessionId) : null
  const inbox = inboxSessionDirectory(input.root, input.sessionId, new Date(input.createdAt))
  const source = firstDistinct(directory, fromProject, recorded, inbox)
  const recordedOwnsFolder = Boolean(recorded && recorded !== inbox && recorded !== directory)
  if (!source) {
    const wrote = await writeSessionLocator(files, directory, input.sessionId, input.createdAt)
    return wrote ? { directory, movedFrom: null } : null
  }
  try {
    await ensureParentDirectory(files, directory)
    await files.moveDir(source, directory)
    return { directory, movedFrom: source }
  } catch (error) {
    const failure = moveFailure(error)
    if (failure === 'occupied') return { directory, movedFrom: null }
    // The agent plane moves `workspace/<session id>` itself. Creating the
    // destination here makes that move fail with "already exists".
    if (recordedOwnsFolder && source === recorded) return { directory, movedFrom: null }
    if (failure !== 'missing') throw error
    const wrote = await writeSessionLocator(files, directory, input.sessionId, input.createdAt)
    return wrote ? { directory, movedFrom: null } : null
  }
}

async function writeSessionLocator(
  files: WorkspaceFiles,
  directory: string,
  sessionId: string,
  createdAt: string
): Promise<boolean> {
  const meta = sessionMetaDirectory(directory)
  const file = sessionFile(directory)
  if (!meta || !file) return false
  await files.mkdir(meta)
  await files.write(file, codeSessionDocument(sessionId, directory, createdAt))
  return true
}

export async function moveCodeSessionToInbox(
  files: WorkspaceFiles,
  input: {
    root: string
    sessionId: string
    createdAt: string
    fromProjectPath: string
  }
): Promise<CodeSessionMove | null> {
  const directory = inboxSessionDirectory(input.root, input.sessionId, new Date(input.createdAt))
  const source = locatedSessionDirectory(input.fromProjectPath, input.sessionId)
  if (!directory || !source) return null
  if (source === directory) {
    const wrote = await writeSessionLocator(files, directory, input.sessionId, input.createdAt)
    return wrote ? { directory, movedFrom: null } : null
  }
  try {
    await ensureParentDirectory(files, directory)
    await files.moveDir(source, directory)
    return { directory, movedFrom: source }
  } catch (error) {
    const failure = moveFailure(error)
    // The dated inbox can still exist after the task folder moved into the project.
    // The agent plane merges into that directory; aborting here skips the unassign.
    if (failure === 'occupied') return { directory, movedFrom: null }
    if (failure !== 'missing') throw error
    const wrote = await writeSessionLocator(files, directory, input.sessionId, input.createdAt)
    return wrote ? { directory, movedFrom: null } : null
  }
}

/** Move the session folder, then assign the project. A failed assignment moves the folder back. */
export async function placeTaskInProject(
  files: WorkspaceFiles,
  input: {
    root: string
    sessionId: string
    createdAt: string
    fromProjectPath: string | null
    toProjectPath: string
    fromFolder?: string | null
  },
  assign: () => Promise<boolean>
): Promise<boolean> {
  const move = await moveCodeSessionDirectory(files, input)
  if (!move) return false
  const assigned = await assign()
  if (!assigned && move.movedFrom) {
    await files.moveDir(move.directory, move.movedFrom)
  }
  return assigned
}

/** Move the session folder back to the dated inbox. A failed unassign moves the folder back. */
export async function placeTaskInInbox(
  files: WorkspaceFiles,
  input: {
    root: string
    sessionId: string
    createdAt: string
    fromProjectPath: string
  },
  assign: () => Promise<boolean>
): Promise<boolean> {
  const move = await moveCodeSessionToInbox(files, input)
  if (!move) return false
  const assigned = await assign()
  if (!assigned && move.movedFrom) {
    await files.moveDir(move.directory, move.movedFrom)
  }
  return assigned
}

export function studioWorkspaceFiles(): WorkspaceFiles | null {
  const file = window.api?.file
  if (
    !file ||
    typeof file.mkdir !== 'function' ||
    typeof file.write !== 'function' ||
    typeof file.moveDir !== 'function'
  ) {
    return null
  }
  return {
    mkdir: (dirPath) => file.mkdir(dirPath),
    write: (filePath, data) => file.write(filePath, data),
    moveDir: (dirPath, newDirPath) => file.moveDir(dirPath, newDirPath)
  }
}
