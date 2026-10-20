# Studio Kylin / Linux package

This is Studio's separate `dev.a3s.studio` Flatpak, targeting x86-64 Kylin V10 with
`org.gnome.Platform//50` (GTK3/WebKitGTK 4.1). It does not install or replace Work,
change Studio's server update feed, copy user state, or run a user installer.

## Build one generation

From `apps/studio`, with the root pinned Rust toolchain, installed frontend dependencies,
Docker and the prepared `aster-kylin-build:24.04` image:

```sh
A3S_TENANT_SLUG=toyou bash scripts/kylin-flatpak/build-host.sh
bash scripts/kylin-flatpak/build-immodule.sh
```

The first script builds the renderer and Office kernel, then builds Studio, workspace ACP
and Office together under `target/studio-kylin-x86_64`. Linux outputs go to
`apps/studio/dist/kylin`; the checkout `.env` is replaced by an empty read-only file in
the build container. No credentials are copied. `studio-build.json` seals the three binary
hashes and exact renderer generation. Failed builds cannot create a new receipt.

The second script builds the independent fcitx4 and fcitx5 GTK3 clients and GDK backend
helper. The fcitx4 recipe keeps `ENABLE_SNOOPER=ON`: disabling it caused duplicate native
key delivery and first-key `repeat=true` in WebKit. The host's daemon is never replaced.
The source archives, fcitx4 portal patch, licenses and reproducible recipes accompany the
modules. The corresponding source SHA-256 values are pinned in `delivery.py`.

An explicitly checked reuse of the qualified Work R4 **native modules only** is available:

```sh
curl -fL https://github.com/fcitx/fcitx5-gtk/archive/refs/tags/5.1.7.tar.gz -o /tmp/fcitx5-gtk-5.1.7.tar.gz
python3 scripts/kylin-flatpak/delivery.py import-r4 \
  ../../target/worktrees/kylin-ime-hardening-20261020/apps/work/dist/kylin \
  dist/kylin-ime-r4 /tmp/fcitx5-gtk-5.1.7.tar.gz
```

`import-r4` refuses any changed module/helper hash, including the old snooper-disabled
fcitx4 module. It verifies both exact upstream archives and preserves the source patch.
Copy the verified `immodules`, `kylin-im-backend` and `ime-assets.json` from that directory
into `dist/kylin` before packaging. Do not copy Work's host, ACP, Office, feed or installer.
The mechanism provenance and original file hashes are in `work-provenance.json`.

## Package without installing

Run in an x86-64 Linux container with GNOME Platform 50 installed for its isolated user:

```sh
bash scripts/kylin-flatpak/package-bundle.sh
```

The packaging environment needs `flatpak`, `python3`, `binutils`, `bubblewrap`,
`xdg-dbus-proxy`, GTK3 development tools, `dbus-x11`, `fcitx-bin`, `fcitx-module-dbus`,
`fcitx-pinyin`, `fcitx-ui-classic`, `xvfb` and `xauth`. The default tenant input is the
restored **brand data only** at `../web/public/tenant`; only the receipt's selected tenant
is copied. Provider/model configuration is not copied from that tree.

The package uses this exact layout:

| Resource | Path inside Flatpak |
| --- | --- |
| Studio | `/app/bin/Studio` |
| ACP | `/app/bin/agent/a3s-code-acp` |
| Office | `/app/bin/a3s-office` |
| Renderer | `/app/Resources/studio-frontend` |
| Brand data / default tenant | `/app/Resources/tenant/<slug>` / `/app/Resources/studio-tenant` |
| Independent fcitx caches | `/app/lib/im/fcitx4`, `/app/lib/im/fcitx5` |

Those companion paths already match Studio's Linux lookup; no fallback to another product
is added. Every launch verifies all listed files against `studio-generation.json` and the
complete `SHA256SUMS` before executing Studio. Missing, modified, mixed-generation,
unexpected or symlinked files refuse startup. `--print` reports the input-method decision
without starting the app. It does not certify a generation.

Packaging requires all host/companion/module links to resolve in the actual GNOME runtime,
loads both modules with `RTLD_NOW`, requires the runtime's IBus cache entry, and runs the
private-bus GTK gate before export. It grants exact fcitx/portal/IBus talk names, not the
complete session bus. A stale host Fcitx address is replaced by the filtered session bus.
X11 chooses the actually owned fcitx5, display-specific fcitx4/alias or IBus service and
never substitutes an incompatible module when its selected module is missing. Backend
selection uses GDK's actual display rather than guessing from a stale environment value.

The product packaging script has **no seccomp fallback**. An unsupported kernel refuses
the real Flatpak runtime gate and prevents export; no security bypass is shipped. Private
test-only bwrap probes can diagnose module/protocol behavior but cannot prove a working
production Flatpak sandbox. Native Wayland input and physical Kylin hardware remain
unverified until separately exercised.

## Maintained validation

```sh
python3 -m unittest discover -s scripts/kylin-flatpak -p test_delivery.py -v
bash scripts/kylin-flatpak/test-input-method.sh APP_FILES RUNTIME_FILES
```

The hermetic tests use synthetic ELF/archive inputs at a controlled build-data boundary;
they prove generation refusal, Studio layout, cache rewriting and environment decisions.
They do not qualify upstream binaries. The native GTK gate uses real pinned modules,
Chinese commit, mixed input, Escape cancellation and exactly-once ASCII delivery on both
the host display and Flatpak 1.6-style `:99` mapping. Its additional eight protocol cases
cover modern and legacy fcitx portal paths and refusal boundaries. Neither fixture is a
physical Kylin Studio GUI test. Current evidence belongs in
`docs/evidence/studio/2026-10-21-work-alignment` at the repository root.
