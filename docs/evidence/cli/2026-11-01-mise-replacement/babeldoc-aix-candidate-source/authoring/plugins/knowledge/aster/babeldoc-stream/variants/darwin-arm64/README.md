# BabelDOC Stream

`aster/babeldoc-stream` recipe 0.1.0 embeds BabelDOC Stream 0.6.4.post4
and its own CPython 3.12.14. The exact runtime archive is bound by SHA-256;
installation does not invoke pipx, uv or an upstream installer. The source
identity remains `pipx:babeldoc-stream`; the recipe version is independent.

The upstream license expression is `AGPL-3.0`. LICENSE
and NOTICE are copied verbatim, and the archive retains Python and every wheel's
notices. ASTER-RECIPE-LICENSE covers Aster's acquisition recipe and launcher.
The pinned wheel lock constrains bitstring to <5 because BabelDOC uses BitStream.
The launcher uses the original `babeldoc.main:cli` with an isolated owned Python.

This macOS ARM64 candidate is not an official publication. Version startup does
not establish PDF translation, native GUI acceptance or other-target support.
