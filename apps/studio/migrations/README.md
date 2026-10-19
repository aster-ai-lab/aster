**THIS DIRECTORY IS NOT FOR RUNTIME USE**

**Migration files are append-only**
The chain was consolidated into a single initial migration and shipped with `v2.0.0-rc.1`, so it now runs against databases holding real user data. Never reinitialize, rewrite, or renumber a migration that has shipped, and never advise deleting `Data/cherrystudio.sqlite`.

- `migrations/sqlite-drizzle` contains the shipped SQL. Do not modify it.
