# Notion CLI

`aster/ntn` 0.23.25 references the immutable upstream npm tarball. Each
`variants/<target>` directory is a native AIX build source with the same package
ID. Platform selection follows the producer's explicit `install.cjs` table.
The acquisition declaration installs the native command from `dist/`, retaining
the complete tarball and upstream license. It does not execute install scripts.

Publication requires the desktop toolchain lifecycle adapter and remote registry
acceptance. These candidate artifacts are not a production publication.
