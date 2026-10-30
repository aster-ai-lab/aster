# Data model

All document rectangles are finite integer image pixels with exclusive bottom-right edges; points are image pixels. Display coordinates are never persisted as document coordinates. Image dimensions and decoded pixel count are bounded before allocation. Original bytes are preserved locally; redacted export bytes replace original attachment bytes and have their own SHA-256.

A visual document contains original encoded image, dimensions, ordered numbered notes (point/region/global), independent nonduplicated source pieces and destination rectangles, redaction rectangles, and optional video frame timestamp. Movements derive from source/destination rather than trusting supplied dx/dy. Exported feedback is task data with explicit Aster provenance.

Projects embed document images and editor state. Video projects reference the source and embed only annotated frame documents, sorted by timestamp. History is bounded and tenant-local. Preferences are separate from project data and use existing ACL storage.

The renderer holds draft UI state and owner callback. Native commands validate the complete external document once. Cancel never invokes the completion callback. Expired or superseded drafts cannot submit to another owner.
