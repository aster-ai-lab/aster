# Aster local patch

This is Crossterm 0.28.1 from the published crates.io archive, under the
retained MIT license. Its package name, version and public Rust API remain
unchanged. Archive identity and upstream revision are recorded in
`docs/source-manifest.json` at the repository root.

Windows ConPTY commits some Unicode text, including combining accents, in a
`VK_MENU` key-up record carrying a nonzero UTF-16 character. The local parser
emits that committed character as a text press. Ordinary character key-up
records and empty Alt releases keep their upstream behavior. Regression tests
convert actual Windows input-record structures through the existing parser.

The package uses the root workspace and lockfile, with `publish = false`.
Upstream examples and notices are retained; Rust formatting follows the pinned
workspace toolchain. The paired Aster/ACP source identity includes this tree.
