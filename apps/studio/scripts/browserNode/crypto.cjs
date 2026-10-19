function unavailable(name) {
  return () => {
    throw new Error(`browser renderer has no node crypto.${name}`)
  }
}

module.exports = {
  createHash: unavailable('createHash'),
  createHmac: unavailable('createHmac'),
  randomBytes: (size) => new Uint8Array(size),
  randomFillSync: (buffer) => buffer,
  randomUUID: () => '00000000-0000-4000-8000-000000000000',
  webcrypto: globalThis.crypto
}
