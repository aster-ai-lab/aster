/** Admitted aster-orm runtimes. Ids match `Engine::profiles` in crates/orm. */
export const admittedEngines = [
  { id: 'sqlite', label: 'SQLite', transport: 'file' },
  { id: 'postgres', label: 'PostgreSQL', transport: 'url' }
]
