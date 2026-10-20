# Test-only passive WebKit DOM observer

This library is an experimental observer, not a shipped component. It does not alter the final application binary, resources, preferences, text or editor state. It interposes the WebKit view's `load_uri`, `load_html` and `load_request` entry points, adds one bounded document-start user script to the view's existing content manager, then calls the original load function with the same arguments. The first experiment hooked only `load_uri`; that library remains at its original path. Version 2 adds the other two load functions imported by the final application. The first inspection mistakenly looked only at outer `app.log`; the host redirects stderr into the fixture's `XDG_STATE_HOME/AsterDream/logs`. Both versions actually reached `load_uri` and emitted `observer-ready`. There is no evidence that the extra entry points are needed for this application.

The script adds passive capture/bubble listeners and sends only keyboard/composition metadata to its private WebKit script-message channel. It does not dispatch or cancel events, change focus, read input contents, or call editor commands. `view.composing` is read from the Tiptap DOM editor when available. Each document emits at most 1,200 records; the native callback accepts at most 4,000 records, each no larger than 4,096 bytes. Records use `[IME-DOM]` in the application's existing stderr destination, under the fixture's `app-state/AsterDream/logs` after the host redirects its log.

Build inside the owned Linux fixture container, from this folder:

```sh
cc -shared -fPIC -O2 -Wall -Wextra -Werror -Wno-deprecated-declarations \
  -Wl,-z,defs -o libime-dom-observer-v2.so ime-dom-observer.c \
  $(pkg-config --cflags --libs webkit2gtk-4.1) -ldl
```

The generated header contains the exact `ime-dom-observer.js` as a C string. A new isolated GUI fixture can give only the application these additional Flatpak arguments:

```sh
--filesystem=/aster/target/evaluations/kylin-ime-gui/readonly-observer:ro \
--env=LD_PRELOAD=/aster/target/evaluations/kylin-ime-gui/readonly-observer/libime-dom-observer-v2.so
```

Do not apply `LD_PRELOAD` to the outer Flatpak client. Verify both the installation marker and `observer-ready` before interpreting missing event records. Instrumentation can perturb timing, so corroborate visible behavior against the same final application in a fixture without this library. This experiment does not establish sandbox security or physical Kylin acceptance.
