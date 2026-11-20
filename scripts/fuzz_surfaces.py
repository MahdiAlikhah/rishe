#!/usr/bin/env python3
"""Bounded fuzz of the hub, serve and embedding-file surfaces.

Each case runs in its own subprocess: a panic or stack overflow kills the
process instead of raising, so a crash must be recorded, not caught.

Usage: `fuzz_surfaces.py [--cases N] [--seed S] [--class serve|embeddings|hub|hub-net]`

Exits 1 when a class crashed, hung, or accepted a malformed input. Scope and
exit criteria: `training/FUZZ_TASK.md`.

Not part of the release gate: it needs a built CLI and spawns thousands of
processes, so it runs on demand rather than on every CI job. CI runs only
`--selftest`, which needs no binary.
"""
import argparse
import json
import os
import random
import struct
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(ROOT, "target", "debug",
                   "rishe.exe" if os.name == "nt" else "rishe")
PY = sys.executable
TIMEOUT = 25
# A hub pull to a host that black-holes the connection is bounded by design at
# retries x connect-timeout = 3 x 15s = 45s, so a per-case cap below that reads a
# correct retry as a hang. Measured: 45.0s for `http://127.0`, 0.1s for a name
# that does not resolve.
HUB_TIMEOUT = 90

# A valid .vec to mutate: dim, then word + dim floats.
VEC_DIM = 3
VEC = b"3 2\n" + "\u06a9\u062a\u0627\u0628 0.1 0.2 0.3\n\u0633\u0644\u0627\u0645 0.4 0.5 0.6\n".encode("utf-8")

CASES = 2000


# ---------------------------------------------------------------- mutation ---
def mutate(data, rng):
    """Apply one random corruption to a byte string."""
    if not data:
        return bytes([rng.randrange(256)])
    choice = rng.randrange(10)
    b = bytearray(data)
    if choice == 0:                                   # flip one byte
        i = rng.randrange(len(b))
        b[i] ^= 1 << rng.randrange(8)
    elif choice == 1:                                 # truncate
        del b[rng.randrange(len(b)):]
    elif choice == 2:                                 # append garbage
        b += bytes(rng.randrange(256) for _ in range(rng.randrange(1, 40)))
    elif choice == 3:                                 # duplicate a slice
        i = rng.randrange(len(b))
        j = min(len(b), i + rng.randrange(1, 20))
        b[i:i] = b[i:j]
    elif choice == 4:                                 # replace a slice
        i = rng.randrange(len(b))
        j = min(len(b), i + rng.randrange(1, 20))
        b[i:j] = bytes(rng.randrange(256) for _ in range(j - i))
    elif choice == 5:                                 # insert a separator
        i = rng.randrange(len(b))
        b[i:i] = rng.choice([b"\n", b"\r\n", b"\x00", b" ", b"\t"])
    elif choice == 6:                                 # huge number in place
        i = rng.randrange(len(b))
        b[i:i] = rng.choice([b"999999999999", b"-1", b"0", b"nan", b"inf"])
    elif choice == 7:                                 # reorder two chunks
        i = rng.randrange(len(b))
        head, tail = b[:i], b[i:]
        b = bytearray(tail + head)
    elif choice == 8:                                 # drop a byte
        del b[rng.randrange(len(b))]
    else:                                             # keep, but change case
        b = bytearray(bytes(b).lower() if rng.random() < 0.5 else bytes(b).upper())
    return bytes(b)


def run(args, stdin=None, env=None, timeout=TIMEOUT):
    try:
        r = subprocess.run(args, input=stdin, capture_output=True,
                           timeout=timeout, env=env)
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return "TIMEOUT", b"", b""


# ------------------------------------------------------------ serve corpus ---
def json_request(rng):
    """A well-shaped request for a random command, then corrupt its fields."""
    command = rng.choice(["normalize", "tokenize", "lemmatize", "mask", "date",
                          "version", "embed", "similar", "similarity", "pos",
                          "ner", "ezafe", "depparse", "nope"])
    req = {"command": command, "text": rng.choice(["", "x", "\u0633\u0644\u0627\u0645", "a" * 50])}
    if command == "mask":
        req["maskers"] = rng.choice([["url"], [], ["nope"], "url", [1]])
    if command == "similar":
        req["n"] = rng.choice([0, 1, -1, 5, 2**64 - 1, 1.5, "3"])
    if command == "similarity":
        req["a"] = "x"
        req["b"] = rng.choice(["y", "", None, 5])
    return req


def fuzz_serve(rng, cases, tmp):
    """Feed mutated request lines to one `serve` process per case."""
    crashes = []
    env = dict(os.environ, RISHE_HOME=tempfile.mkdtemp(dir=tmp, prefix="serve-"))
    for i in range(cases):
        if rng.random() < 0.5:
            payload = json.dumps(json_request(rng)).encode()
        else:
            payload = rng.choice([
                b"{}", b"[]", b"null", b"42", b'"s"', b"{",
                b'{"command":null}', b'{"command":[]}',
                b'{"command":"normalize","text":null}',
                b'{"command":"normalize","text":{}}',
                b"\xff\xfe", b"\x00", b"\n" * 3,
                json.dumps({"command": "normalize",
                            "text": "\u0633\u0644\u0627\u0645" * rng.randrange(1, 400)}).encode(),
                ("[" * rng.randrange(1, 200) + "]" * rng.randrange(1, 200)).encode(),
            ])
        if rng.random() < 0.6:
            payload = mutate(payload, rng)
        rc, out, err = run([EXE, "serve"], stdin=payload + b"\n", env=env)
        if rc == "TIMEOUT":
            crashes.append(("serve hang", payload))
            continue
        if rc != 0:
            crashes.append((f"serve rc={rc}", payload))
            continue
        # Every reply must be one JSON object carrying `ok`.
        for line in out.split(b"\n"):
            if not line.strip():
                continue
            try:
                obj = json.loads(line)
            except Exception:
                crashes.append(("serve non-JSON reply", payload))
                break
            if not isinstance(obj, dict) or "ok" not in obj:
                crashes.append(("serve reply without ok", payload))
                break
        if len(crashes) >= 3:
            break
    return crashes


# ------------------------------------------------------- embedding corpus ---
def fuzz_embeddings(rng, cases, tmp):
    """Load mutated .vec/.rvec files; a load here means a parse hole."""
    crashes = []
    rvec = os.path.join(ROOT, "tests", "fixtures", "rvec-v1-f16.rvec")
    base_rvec = open(rvec, "rb").read() if os.path.exists(rvec) else b"RVEC"
    for i in range(cases):
        seed = VEC if rng.random() < 0.5 else base_rvec
        blob = mutate(seed, rng) if rng.random() < 0.9 else seed
        ext = ".vec" if seed is VEC else ".rvec"
        path = os.path.join(tmp, f"emb{i}{ext}")
        with open(path, "wb") as f:
            f.write(blob)
        rc, out, err = run([PY, "-c",
                            "import sys,rishe;rishe.Embeddings(sys.argv[1]);print('LOADED')",
                            path],
                           env=dict(os.environ, PYTHONIOENCODING="utf-8"))
        if rc == "TIMEOUT":
            crashes.append(("embeddings hang", blob[:200]))
            continue
        if rc != 0:
            # A crash is a non-zero exit with no traceback the loader raised:
            # Python exits 1 on an uncaught exception, which is the expected
            # rejection, so distinguish by the process dying abnormally.
            if rc not in (1,):
                crashes.append((f"embeddings rc={rc}", blob[:200]))
            continue
        # Exit 0 means it loaded. A mutation may legitimately load when it changed
        # only payload values and not structure, so only flag a load when the
        # header it declares no longer matches the bytes it actually has.
        if b"LOADED" in out and not _still_valid(blob):
            crashes.append(("embeddings loaded a broken file", blob[:200]))
        if len(crashes) >= 3:
            break
    return crashes


def _still_valid(blob):
    """True when the bytes are still a structurally coherent .vec or .rvec."""
    if blob[:4] == b"RVEC":
        if len(blob) < 16:
            return False
        version, dtype, dim, vocab = struct.unpack("<HBII", blob[4:15])
        if version != 1 or dtype not in (0, 1) or dim == 0 or vocab == 0:
            return False
        # Header + per-word (u16 length + bytes) + payload must fit the file.
        width = 2 if dtype == 1 else 4
        offset = 15
        for _ in range(vocab):
            if offset + 2 > len(blob):
                return False
            length = struct.unpack("<H", blob[offset:offset + 2])[0]
            offset += 2 + length
            if offset > len(blob):
                return False
        return offset + vocab * dim * width <= len(blob)
    try:
        lines = blob.decode("utf-8").split("\n")
    except UnicodeDecodeError:
        return False
    head = lines[0].split()
    if len(head) != 2:
        return False
    try:
        dim, vocab = int(head[0]), int(head[1])
    except ValueError:
        return False
    if dim < 0 or vocab < 0:
        return False
    rows = [line for line in lines[1:] if line.strip()]
    return len(rows) == vocab and all(len(r.split()) == dim + 1 for r in rows)


# -------------------------------------------------------------- hub corpus ---
def fuzz_hub(rng, cases, tmp):
    """`hub pull` name and digest validation, before any request is made.

    These cases are chosen to be refused by argument validation (bad digest,
    refused scheme, unregistered name), so they finish in milliseconds. Network
    cases live in `fuzz_hub_net`, which is capped: a connection that is
    black-holed costs the designed 45s, so 2,000 of them is hours, not minutes.
    """
    crashes = []
    schemes = ["ftp://", "file://", "data:", "gopher://", "ws://", "jar:",
               "javascript:", "mailto:", "ssh://", "tel:", "about:", ""]
    names = ["..", ".", "../../etc/passwd", "a" * 200, "\\\\?\\c:\\a.vec",
             "C:\\Windows\\win.ini", "//server/share/a.vec", "\u00e9.vec",
             "\u0622\u0628.vec", "-", "--sha256", "?", "#", "%00.vec",
             "a/", "/a", "a b.vec", "\t.vec", "a\rb.vec", "a\nb.vec"]
    digests = ["", "zz", "0" * 63, "0" * 65, "G" * 64, "0x" + "0" * 62,
               "0" * 64 + "\n", "\u0660" * 64, "-" * 64, " " * 64]
    for i in range(cases):
        pick = rng.random()
        if pick < 0.45:
            # A digest that is not 64 hex characters is refused by validate_sha.
            name = "http://127.0.0.1:9/a.vec"
            sha = rng.choice(digests)
        elif pick < 0.8:
            # A scheme other than http/https is refused before a request.
            name = rng.choice(schemes) + rng.choice(["a.vec", "127.0.0.1/a.vec", ""])
            sha = "0" * 64
        else:
            # A name with no digest and no registry entry is refused by lookup.
            name = rng.choice(names)
            sha = ""
        if rng.random() < 0.4:
            name = mutate(name.encode("utf-8"), rng).decode("utf-8", "replace")
        name = name.replace("\x00", "")
        if not name:
            continue
        argv = [EXE, "hub", "pull", name]
        if sha:
            argv += ["--sha256", sha]
        env = dict(os.environ, RISHE_HOME=tempfile.mkdtemp(dir=tmp, prefix="hub-"))
        rc, out, err = run(argv, env=env, timeout=HUB_TIMEOUT)
        if rc == "TIMEOUT":
            crashes.append(("hub hang (validation case reached the network)", name))
            continue
        if rc not in (0, 1, 2):
            crashes.append((f"hub rc={rc}", name))
        if len(crashes) >= 3:
            break
    return crashes


# A black-holed connection costs the designed 45s, so the network class is a
# fixed small set with a real server rather than 2,000 random attempts.
def fuzz_hub_net(rng, cases, tmp):
    """`hub pull` over a real socket to a local hostile server."""
    crashes = []
    budget = min(cases, 12)
    for i in range(budget):
        port, shutdown = _hostile_server(rng)
        try:
            url = f"http://127.0.0.1:{port}/{rng.choice(['a.vec', '', 'a/b/b.vec'])}"
            env = dict(os.environ,
                       RISHE_HOME=tempfile.mkdtemp(dir=tmp, prefix="hubnet-"))
            rc, out, err = run([EXE, "hub", "pull", url, "--sha256", "0" * 64],
                               env=env, timeout=HUB_TIMEOUT)
            if rc == "TIMEOUT":
                crashes.append(("hub-net hang", url))
            elif rc not in (0, 1, 2):
                crashes.append((f"hub-net rc={rc}", url))
            elif rc == 0:
                # A hostile body that was accepted is the defect this class hunts.
                crashes.append(("hub-net accepted a hostile body", url))
        finally:
            shutdown()
        if len(crashes) >= 3:
            break
    return crashes


def _hostile_server(rng):
    """Serve one malformed HTTP response on a loopback port.

    Returns (port, shutdown). The response is one of: truncated body against a
    lying Content-Length, a body longer than declared, a 206 with a bogus range,
    a non-HTTP byte stream, and headers that never end.
    """
    import socket
    import threading

    kind = rng.choice(["short", "long", "range", "garbage", "endless", "empty"])
    payload = b"3 1\nx 0 0 0\n"
    if kind == "short":
        head = b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\n\r\n" % (len(payload) * 10)
        body = head + payload
    elif kind == "long":
        head = b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n"
        body = head + payload * 100
    elif kind == "range":
        body = (b"HTTP/1.1 206 Partial Content\r\nContent-Length: %d\r\n"
                b"Content-Range: bytes 0-0/999999999\r\n\r\n" % len(payload)) + payload
    elif kind == "garbage":
        body = bytes(rng.randrange(256) for _ in range(512))
    elif kind == "endless":
        body = b"HTTP/1.1 200 OK\r\n\r\n" + b"x" * (4 << 20)
    else:
        body = b""

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", 0))
    server.listen(4)
    port = server.getsockname()[1]
    stop = threading.Event()

    def serve():
        server.settimeout(1.0)
        while not stop.is_set():
            try:
                conn, _ = server.accept()
            except (socket.timeout, OSError):
                continue
            with conn:
                try:
                    conn.settimeout(2.0)
                    conn.recv(8192)
                    conn.sendall(body)
                except OSError:
                    pass

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()

    def shutdown():
        stop.set()
        try:
            server.close()
        except OSError:
            pass

    return port, shutdown


# ----------------------------------------------------------------------- run ---
def selftest():
    """Prove the harness can fail, so a green run means something.

    A harness that silently reports "ok" for everything passes the exit criteria
    while testing nothing. Each check below feeds a known-bad case through the
    real detector and requires it to be caught.
    """
    problems = []

    # 1. The .vec validity check must reject a header that contradicts its rows.
    good = b"3 1\nw 0.1 0.2 0.3\n"
    if not _still_valid(good):
        problems.append("_still_valid rejected a valid .vec")
    if _still_valid(b"3 5\nw 0.1 0.2 0.3\n"):
        problems.append("_still_valid accepted 5 declared rows with 1 present")
    if _still_valid(b"3 1\nw 0.1 0.2\n"):
        problems.append("_still_valid accepted a row with 2 values for dim 3")
    if _still_valid(b""):
        problems.append("_still_valid accepted an empty file")

    # 2. The .rvec check must reject a truncated payload or a bad version.
    rvec = struct.pack("<4sHBII", b"RVEC", 1, 0, 3, 1)
    rvec += struct.pack("<H", 1) + b"w" + struct.pack("<3f", 0.1, 0.2, 0.3)
    if not _still_valid(rvec):
        problems.append("_still_valid rejected a valid .rvec")
    if _still_valid(rvec[:-4]):
        problems.append("_still_valid accepted a truncated .rvec payload")
    if _still_valid(struct.pack("<4sHBII", b"RVEC", 9, 0, 3, 1) + rvec[15:]):
        problems.append("_still_valid accepted .rvec version 9")

    # 3. The mutator must actually change bytes.
    unchanged = 0
    check_rng = random.Random(7)
    for _ in range(200):
        if mutate(VEC, check_rng) == VEC:
            unchanged += 1
    if unchanged > 20:
        problems.append(f"mutate returned the input unchanged {unchanged}/200 times")

    # 4. The hostile server must really answer, or hub-net tests nothing.
    port, shutdown = _hostile_server(random.Random(3))
    try:
        import socket
        with socket.create_connection(("127.0.0.1", port), timeout=5) as sock:
            sock.sendall(b"GET / HTTP/1.0\r\n\r\n")
            if not sock.recv(64):
                problems.append("hostile server sent nothing")
    except OSError as error:
        problems.append(f"hostile server unreachable: {error}")
    finally:
        shutdown()

    for problem in problems:
        print(f"  FAIL {problem}")
    print("fuzz_surfaces: selftest ok" if not problems else
          f"fuzz_surfaces: selftest FAILED ({len(problems)})")
    return 1 if problems else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cases", type=int, default=CASES)
    parser.add_argument("--seed", type=int, default=20261120)
    parser.add_argument("--class", dest="only",
                        choices=["serve", "embeddings", "hub", "hub-net"],
                        help="run one class only")
    parser.add_argument("--selftest", action="store_true",
                        help="check the harness can detect a defect, then exit")
    args = parser.parse_args()

    if args.selftest:
        return selftest()

    if not os.path.exists(EXE):
        raise SystemExit(f"build the CLI first: cargo build ({EXE} missing)")

    rng = random.Random(args.seed)
    classes = [
        ("serve", lambda r, n, t: fuzz_serve(r, n, t)),
        ("embeddings", lambda r, n, t: fuzz_embeddings(r, n, t)),
        ("hub", lambda r, n, t: fuzz_hub(r, n, t)),
        ("hub-net", lambda r, n, t: fuzz_hub_net(r, n, t)),
    ]
    if args.only:
        classes = [c for c in classes if c[0] == args.only]
    print(f"seed={args.seed} cases/class={args.cases}")
    total = 0
    failed = 0
    for name, fn in classes:
        with tempfile.TemporaryDirectory() as tmp:
            crashes = fn(rng, args.cases, tmp)
        total += args.cases
        status = "ok" if not crashes else f"CRASH x{len(crashes)}"
        print(f"  {name:12} {args.cases:6} cases  {status}")
        for label, blob in crashes:
            failed += 1
            hexed = blob[:20].hex() if isinstance(blob, (bytes, bytearray)) else str(blob)[:80]
            print(f"      {label}: {hexed}")
    print()
    print(f"{total} cases, {failed} crash class(es)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
