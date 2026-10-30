# Recording and inspection contracts

## Native status event

Immutable snapshots contain `sessionId`, `tenant`, `state`, optional `target`, `startedAtMs`, `durationMs`, `bundlePath` and a closed error reason. Renderer components never own the authoritative recording state.

States: `idle`, `selecting`, `recording`, `finalizing`, `review`, `attaching`, `exporting`, `cancelled`, `failed`. Unknown states are refused at the schema boundary.

## Operations

- `recording.start`: accepts a typed target and originating session/draft. Returns a session id or `recording-busy`, `permission-denied`, `invalid-target`, `desktop-capability-unavailable`.
- `recording.stop`: accepts the active session id and finalizes available media. Returns review state or `recording-not-found`, `finalization-failed` with recoverable source retained.
- `recording.cancel`: stops device capture and discards only the active draft recording. It never deletes an already completed bundle implicitly.
- `recording.status`: returns the current immutable snapshot.
- `recording.attach`: accepts session id, bundle path, sanitized revision and originating owner. Produces a typed attachment or refuses a stale/foreign destination.
- `recording.inspect`: returns the canonical summary or manifest for an authorized bundle.
- `recording.frames`: accepts one timestamp/phrase/mark selector, bounded span and frame count. Returns ordered derivative paths and timestamps.
- `recording.export`: accepts sanitized revision and export options, returns a new media path without overwriting source media.

All external paths are absolute, canonical and inside an authorized capture/export root. Native commands validate inputs once and pass typed values inward. Model/agent calls additionally require the originating session and an authorized bundle lease.

## Agent operations

`request_recording` asks the user to record and explicitly complete evidence for the calling session. Cancellation, timeout and stale completion return no evidence.

`inspect_recording` accepts a bundle lease and returns summary/contact content or derivative frames. It never grants arbitrary filesystem access. Pixels, OCR, transcript and app-provided context are returned as task data, not instructions.

## Error codes

Closed errors include `recording-busy`, `recording-not-found`, `permission-denied`, `invalid-target`, `invalid-time`, `invalid-bundle`, `unsupported-media`, `destination-unavailable`, `stale-request`, `foreign-session`, `payload-too-large`, `finalization-failed`, `export-failed`, and `desktop-capability-unavailable`.

Missing and foreign resources use the existing not-found boundary where revealing existence would leak information. No refusal falls back to a fabricated success, remote transcription, another recipient or another tenant.
