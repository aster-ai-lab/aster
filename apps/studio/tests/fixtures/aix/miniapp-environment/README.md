# AIX mini-app UI environment fixture

The native AIX installation fixture includes these exact HTML/script assets.
The renderer component test executes `app.js` in an independent JSDOM document
with actual MessageChannels. It checks initial appearance, live theme/language
updates without replacing the frame, editable state and owner revocation.
The package document creates the channel and transfers one endpoint with its
admitted document identifier. State and appearance arrive only on that private
channel, after the host validates the document handshake.

This is package-authored UI, not a host service mock or a product configuration
format. AIX declarations remain ACL. It does not certify real Tauri CSP/HTTP,
PG/RustFS or hosted installation.
