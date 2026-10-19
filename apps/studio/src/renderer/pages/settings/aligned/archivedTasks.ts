export type ArchivedTask = {
  id: string
  title: string
  projectId: string | null
  projectName: string | null
}

export const ALL_PROJECTS = 'all'
export const NO_PROJECT = ''

export type ArchivedSessionSource = {
  id?: string
  name?: string
  workspace?: { id?: string; name?: string; type?: string } | null
}

export function sessionToArchivedTask(session: ArchivedSessionSource): ArchivedTask | null {
  if (!session.id) return null
  const workspace = session.workspace?.type === 'user' ? session.workspace : null
  const title = session.name?.trim() || session.id
  return {
    id: session.id,
    title,
    projectId: workspace?.id || null,
    projectName: workspace?.name || null
  }
}

export function filterArchivedTasks(all: readonly ArchivedTask[], query: string, projectId: string): ArchivedTask[] {
  const needle = query.trim().toLowerCase()
  return all.filter((task) => {
    if (projectId !== ALL_PROJECTS && (task.projectId ?? NO_PROJECT) !== projectId) return false
    if (!needle) return true
    const haystack = `${task.title} ${task.projectName ?? ''}`.toLowerCase()
    return haystack.includes(needle)
  })
}
