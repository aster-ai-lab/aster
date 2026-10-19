# Document tool review regression

This bounded regression loads the actual production applet, accepts its permission
prompt, imports real PNG files with one unsupported file, selects a real grayscale
filter, accepts it with the keyboard, downloads it, and inspects its actual pixels.
It also removes a source, appends a PNG with different pixels, sorts it with the
keyboard, checks previous-version feedback, processes the revised order and
checks both real downloaded PNGs. It captures desktop and 390px layouts. It is not native/live OCR, ASR, image-repair,
device/sync or full-epic certification.

Start an isolated production renderer in one terminal:

```sh
./node_modules/.bin/rsbuild dev --host 127.0.0.1 --port 4325
```

Run from `apps/studio`:

```sh
bash tests/e2e/document-tools/verify-review.sh
```

The runner reports its evidence directory. `A3S_CLOUD_DOCUMENT_TOOLS_EVIDENCE_DIR` can
select a retained evidence directory. Installed A3S Test must support protocol 15.
The local browser adapter defaults to the installed 0.4.x leaf driver at
`~/.local/bin/a3s-use-browser-driver`. Set `A3S_CLOUD_DOCUMENT_TOOLS_BROWSER_DRIVER` to
select another compatible leaf executable. The adapter only removes the CLI
`use browser` prefix; it leaves browser command/error behavior intact.

Semantic catalogue/source buttons use explicit accessible action names. The radio
uses its stable native input selector because this installed driver's role resolver
does not recognize implicit radio roles. These selectors were obtained from the
observed production page, not an alternate fixture UI. Download initiation leaves
the UI unsaved: only an actual native writer receipt may mark an output saved.
