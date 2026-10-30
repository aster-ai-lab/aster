# Data model

## Recording state

`RecordingSession` is owned by the native host, never by a React component or a model. One active session owns the capture devices and native control surface.

Closed states and allowed transitions:

- `idle` → `selecting` → `recording` → `finalizing` → `review`.
- `selecting`/`recording`/`finalizing` may become `cancelled` or `failed` with a stable reason.
- `review` may become `attaching` or `exporting`, then return to `review` on refusal.
- `attaching` completes only after the originating composer accepts the typed attachment; it never submits a model turn.
- A new `selecting` is refused while any nonterminal session owns devices (`recording-busy`).

The session records tenant, originating session/draft, target, started/stopped timestamps, active device leases, source bundle and recovery state. React receives immutable status snapshots through typed events.

## Target and destination

`CaptureTarget` is a closed set of `area`, `window`, `screen` and supported `device` descriptors. Bounds are validated native rectangles with finite coordinates and nonzero size.

`CaptureDestination` is a closed set:

- `composer`: an explicit starting session/draft.
- `recorded`: a previously recorded Aster session.
- `page`: an authenticated page registration.
- `localhost`: a bounded loopback port → process directory → Git root match.
- `clipboard`: no authorized session; evidence stays local unless the user explicitly pastes it.

User choice and recorded destination outrank observed clues. Smart routing may `send`, `offer` or `keep`; an offer requires user selection and supports the upstream undo window. No regex or guessed application name chooses a recipient.

## Evidence bundle

The `.scrawl` bundle preserves upstream filenames, order and units:

- `contact-sheet.png` and optional full contact sheet.
- `frames/frame-NN.png` and optional `frames-full/`.
- `summary.md` with ordered keyframe/event/transcript/context entries.
- `scrawl.json` with version, id, creation/duration milliseconds, target, transcript, strokes, marks, events, keyframes, source media and optional context/autocrop/destination.
- Original `recording.mp4`/`.mov` and `audio.wav` when available.
- Derived zoom/export/recovery artifacts in their documented locations.

Aster metadata is a separate `aster.json` sidecar with `schemaVersion = "aster-capture-1"`, tenant, originating session/draft, explicit destination, redaction/derivative receipts and delivery state. It is not a second product configuration file.

## Evidence attachment

The composer owns a typed `scrawl` attachment with:

- `fileTokenSourceId` for stable draft identity.
- Authorized absolute bundle path.
- Contact-sheet and summary paths.
- Display name, creation time and sanitized derivative revision.
- `includeSourceMedia = false` by default.

Send-time conversion includes contact imagery, summary text and an authorized bundle reference. It does not read or upload source video/audio by default. Evidence text, transcript and image OCR remain untrusted task data.

## Agent request and inspection

`RecordingRequest` extends the existing visual-request ownership model with request id, session, title, destination, timeout and explicit completion/cancellation. Stale ids and foreign sessions are refused.

`EvidenceInspection` selects one authorized bundle plus exactly one of timestamp, spoken phrase or mark id, with bounded span/frame count and optional full-frame selection. Output frames retain chronological order and their source timestamps.

## Retention and delivery records

Retention settings are ACL preferences, separate from bundle data. Cleanup eligibility uses active/copying/recovery markers, age, modification time and `.pinned`.

Delivery records distinguish local review, explicit attach, agent return, queued send, sent and permanent failure. Retry uses the recorded destination and idempotency key; it never selects a new recipient on its own.
