#!/usr/bin/env python3
"""Verify a Postgres address accepts the given credentials.

Guards against an impostor server (e.g. an OrbStack machine's own postgres)
shadowing the compose Postgres container forward on the host loopback.
Prints one line: "ok" or a reason.

Args: user password [host] [port]. Host defaults to 127.0.0.1, port to 5432.
"""
import socket
import struct
import sys


def startup(user: str, database: str) -> bytes:
    params = f"user\x00{user}\x00database\x00{database}\x00\x00".encode()
    return struct.pack("!ii", 8 + len(params), 196608) + params


def probe(
    user: str,
    password: str,
    database: str = "postgres",
    host: str = "127.0.0.1",
    port: int = 5432,
) -> str:
    try:
        sock = socket.create_connection((host, port), timeout=5)
    except Exception as err:  # noqa: BLE001
        return f"connect-failed: {err}"
    try:
        sock.sendall(startup(user, database))
        resp = sock.recv(128)
        if resp[:1] != b"R" or len(resp) < 9:
            return f"unexpected response: {resp[:24].hex()}"
        code = int.from_bytes(resp[5:9], "big")
        if code == 0:
            return "ok"  # trust — credentials never checked (misconfiguration)
        if code == 3 or code == 5:
            return "reachable (cleartext/md5 requested)"  # not the expected SCRAM
        if code == 10:
            result = sasl(sock, resp[9:], user, password)
        elif code == 12:
            result = "ok"  # SASL final-ok
        else:
            body = resp[9:].decode("utf-8", "replace")
            result = f"server-refused: {body[:140]}"
    except Exception as err:  # noqa: BLE001
        import traceback
        traceback.print_exc()
        return f"handshake-failed: {err}"
    finally:
        sock.close()
    return result


def sasl(sock: socket.socket, mechanisms: bytes, user: str, password: str) -> str:
    import base64
    import hashlib
    import hmac as hmac_mod

    if b"SCRAM-SHA-256" not in mechanisms:
        return f"unsupported auth mechanisms: {mechanisms[:60]!r}"

    def sasl_message(payload: bytes) -> bytes:
        # SASLInitialResponse / SASLResponse both use message type 'p'.
        return b"p" + struct.pack("!i", 4 + len(payload)) + payload

    nonce = "probe0000000000000000000000000000"
    # gs2 header "n,," + client-first-bare — the server expects both.
    client_first = f"n,,n=,r={nonce}".encode()
    initial = b"SCRAM-SHA-256\x00" + struct.pack("!i", len(client_first)) + client_first
    sock.sendall(sasl_message(initial))

    resp = sock.recv(1024)
    if resp[:1] != b"R" or int.from_bytes(resp[5:9], "big") != 11:
        return f"server-refused: {resp[:40]!r}"
    server_first = resp[9:]
    rnonce = _field(server_first.decode("utf-8", "replace"), "r")
    salt = _field(server_first.decode("utf-8", "replace"), "s")
    iterations = int(_field(server_first, "i") or "4096")
    if not rnonce.startswith(nonce) or not salt:
        return "scram-nonce/salt missing"

    salted = hashlib.pbkdf2_hmac(
        "sha256", password.encode(), base64.b64decode(salt), iterations
    )
    client_key = hmac_mod.new(salted, b"Client Key", hashlib.sha256).digest()
    stored = hashlib.sha256(client_key).digest()
    client_final_bare = f"c=biws,r={rnonce}".encode()  # rnonce now bytes-decoded str
    # RFC 5802: AuthMessage = client-first-BARE + "," + server-first + ","
    #                           + client-final-without-proof
    auth_message = (
        client_first[3:] + b"," + server_first + b"," + client_final_bare
    )
    signature = hmac_mod.new(stored, auth_message, hashlib.sha256).digest()
    proof = bytes(a ^ b for a, b in zip(client_key, signature))
    final = client_final_bare + f",p={base64.b64encode(proof).decode()}".encode()
    sock.sendall(sasl_message(final))

    resp = sock.recv(256)
    if resp[:1] == b"R":
        code = int.from_bytes(resp[5:9], "big")
        if code == 0:
            return "ok"  # AuthenticationOk — credentials accepted
        if code == 12:
            # SASLFinal ("v=…") — the session verifier; a follow-up R 0 confirms.
            return "ok"
        body = resp[9:]
        return f"auth-failed: {body[:140].decode('utf-8', 'replace')}"
    body = resp[8:].decode("utf-8", "replace") if len(resp) > 8 else ""
    return f"unexpected post-final: {body[:120]}"


def _field(payload, key):  # payload: bytes or str
    if isinstance(payload, bytes):
        payload = payload.decode("utf-8", "replace")
    for part in payload.split(","):
        if part.startswith(key + "="):
            return part[len(key) + 1:]
    return ""


if __name__ == "__main__":
    user = sys.argv[1] if len(sys.argv) > 1 else "a3s_api"
    password = sys.argv[2] if len(sys.argv) > 2 else "a3s_api"
    host = sys.argv[3] if len(sys.argv) > 3 else "127.0.0.1"
    port = int(sys.argv[4]) if len(sys.argv) > 4 else 5432
    print(probe(user, password, host=host, port=port))
