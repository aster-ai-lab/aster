#!/usr/bin/env python3
"""Internal one-shot IPC adapter for a Power-owned Xolotl process.

Power parses ACL and supplies a bounded, typed launch record on stdin. This
adapter never discovers or reads a second product configuration file.
"""
import copy
import ctypes
import json
import os
from pathlib import Path
import runpy
import signal
import stat
import sys
import uuid

SCHEMA = "a3s.power.xolotl.launch.v1"
READY_SCHEMA = "a3s.power.xolotl.ready.v2"
MAX_IPC_BYTES = 64 * 1024
SENTINEL = "a3s-power-owned-ipc"
FIELDS = {"schema", "nonce", "parent_pid", "server", "tokenizer", "model_name",
          "engine_argv", "engine_env", "host", "port"}


def text(value, name, maximum=4096):
    if not isinstance(value, str) or not value or len(value.encode()) > maximum or any(c in value for c in "\0\r\n"):
        raise ValueError(f"Power launch {name} is invalid")
    return value


EXACT_ROUTE_ARGS = ("--route-bias", "0", "--route-bias-file", "0", "--route-bias-remote", "0")


def admitted_engine_args(args):
    if not isinstance(args, list):
        raise ValueError("Power launch must carry one admitted Xolotl argv")
    base = args[:-1] if args and args[-1] == "--kv-fp4" else args
    if base[-6:] != list(EXACT_ROUTE_ARGS):
        raise ValueError("Power launch must preserve exact expert routing")
    return base[:-6]


def decode_launch(stream):
    raw = stream.read(MAX_IPC_BYTES + 1)
    if len(raw) > MAX_IPC_BYTES:
        raise ValueError("Power launch exceeds its IPC byte bound")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("Power launch contains a duplicate field")
            result[key] = value
        return result
    value = json.loads(raw, object_pairs_hook=unique)
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        raise ValueError("Power launch has an unknown schema or field set")
    text(value["nonce"], "nonce", 128)
    text(value["model_name"], "model identity", 256)
    for key in ("parent_pid", "port"):
        if type(value[key]) is not int or value[key] <= 1:
            raise ValueError(f"Power launch {key} must be a positive process/port identity")
    if value["port"] > 65535 or value["host"] != "127.0.0.1":
        raise ValueError("Power adapter must listen on an explicit IPv4 loopback port")
    for key in ("server", "tokenizer"):
        if not Path(text(value[key], key)).is_absolute():
            raise ValueError(f"Power launch {key} must be absolute")
    args = value["engine_argv"]
    if not isinstance(args, list):
        raise ValueError("Power launch must carry one admitted Xolotl argv")
    base_args = admitted_engine_args(args)
    if len(base_args) not in (12, 16, 26, 30):
        raise ValueError("Power launch must carry one admitted Xolotl argv")
    for arg in args:
        text(arg, "engine argument")
    if not Path(args[0]).is_absolute() or args[1] != "--serve":
        raise ValueError("Power launch omitted the owned resident engine")
    if args[2:12:2] != ["-m", "--engram", "--rank", "--backend", "--experts"] or args[9] != "cuda" or args[11] != "gpu":
        raise ValueError("Power launch is outside the admitted CUDA expert-cache plan")
    if not Path(args[3]).is_absolute() or not Path(args[7]).is_absolute() or not all(Path(segment).is_absolute() for segment in args[5].split(":")):
        raise ValueError("Power launch model, Engram and rank paths must be absolute")
    split = len(base_args) in (16, 30)
    offset = 16 if split else 12
    if split and args[12:16] != ["--layer-split", "5,10,15,20,25,30,35", "--split-device", "0,1,2,3,4,5,6,7"]:
        raise ValueError("Power launch changed the admitted eight-card split")
    validate_tuning(base_args[offset:])
    env = value["engine_env"]
    if not isinstance(env, dict) or set(env) != {"CUDA_VISIBLE_DEVICES"}:
        raise ValueError("Power launch changed its device environment authority")
    indices = text(env["CUDA_VISIBLE_DEVICES"], "CUDA placement", 128).split(",")
    expected = 8 if split else 1
    def device_identity(identity):
        if identity.startswith("GPU-"):
            try:
                return str(uuid.UUID(identity[4:])) == identity[4:]
            except ValueError:
                return False
        return identity.isascii() and identity.isdecimal() and str(int(identity)) == identity and int(identity) <= 4294967295
    if len(indices) != expected or len(set(indices)) != expected or not all(device_identity(i) for i in indices):
        raise ValueError("Power launch changed its admitted device count")
    return value


def validate_tuning(tail):
    if not tail:
        return 4096
    flags = ["--ctx", "--threads", "--arena-gib", "--prompt-batch", "--prefill-chunk", "--pcie", "--tier-batch-pcie"]
    if len(tail) != 14 or tail[::2] != flags:
        raise ValueError("Power launch changed its typed engine tuning")
    values = []
    for value, lower, upper, step in zip(tail[1:10:2], [4096, 1, 0, 256, 64], [1048576, 256, 192, 8192, 1024], [256, 1, 1, 256, 64]):
        if not value.isascii() or not value.isdecimal() or str(int(value)) != value:
            raise ValueError("Power tuning integer is not canonical")
        parsed = int(value)
        if parsed < lower or parsed > upper or parsed % step:
            raise ValueError("Power tuning exceeds admitted resource bounds")
        values.append(parsed)
    if values[4] > values[3]:
        raise ValueError("Power prefill chunk exceeds its prompt batch")
    for value in (tail[11], tail[13]):
        if len(value) != 6 or value[1] != "." or value[0] not in "01" or not value[2:].isascii() or not value[2:].isdecimal() or (value[0] == "1" and value[2:] != "0000"):
            raise ValueError("Power PCIe fraction is outside canonical basis points")
    return values[0]


def context_proof(actual, args):
    base_args = admitted_engine_args(args)
    split = len(base_args) in (16, 30)
    expected = validate_tuning(base_args[16 if split else 12:])
    if type(actual) is not int or actual != expected:
        raise ValueError("Power-owned engine READY did not prove its admitted context capacity")
    return actual


def bind_parent(expected, libc=None):
    if sys.platform != "linux":
        raise ValueError("Power-managed Xolotl currently requires Linux process supervision")
    libc = libc or ctypes.CDLL(None, use_errno=True)
    # The second identity check closes the race where Power exits before prctl.
    if os.getppid() != expected or libc.prctl(1, signal.SIGKILL, 0, 0, 0) != 0 or os.getppid() != expected:
        raise ValueError("Power process ownership was lost before engine startup")
    return libc


def engine_log_path(model):
    # systemd supplies stderr as a journal socket; reopening /dev/stderr fails.
    # The upstream narrator also seeks its log, so use a private regular file.
    path = Path(model).parent / "xolotl-engine.log"
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW |
                 os.O_CLOEXEC | os.O_NONBLOCK, 0o600)
    try:
        metadata = os.fstat(fd)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid():
            raise ValueError("Power engine log must be an owned regular file")
    finally:
        os.close(fd)
    return str(path)


def publish_ready(protocol, proof):
    protocol.write(json.dumps(proof) + "\n")
    protocol.flush()
    # Python's standard stdout wrapper has closefd=False. Closing the wrapper
    # alone leaves the readiness pipe open, while Power waits for bounded EOF.
    os.dup2(sys.stderr.fileno(), protocol.fileno())
    protocol.close()


def install_dsml_tool_choice(module):
    """Use the model's complete DSML envelope for an explicit tool selection.

    The pinned upstream writes only an invoke opener. The model was trained on
    a calls block, so that prefix can end before any arguments are generated.
    Names come exclusively from the declared tool set; arguments still come
    from model generation and are parsed by the existing DSML parser.
    """
    frontend, service = module["F"], module["S"]
    calls_open, invoke_open = module["CALLS_OPEN"], module["INVOKE_OPEN"]

    def forced_call(choice, tools):
        kind, name = frontend.tool_choice_of(choice)
        names = {tool.get("name") for tool in tools or [] if isinstance(tool, dict)}
        if kind in ("auto", "none"):
            return None
        if kind == "required" and len(names) == 1:
            kind, name = "named", next(iter(names))
        prefix = calls_open + "\n" + invoke_open
        if kind == "required" and names:
            return prefix + " "
        if kind != "named" or name not in names:
            raise ValueError("tool_choice must name an offered function")
        if not isinstance(name, str) or not name or any(c in name for c in '\"<>\r\n\0'):
            raise ValueError("tool name cannot be represented in a DSML attribute")
        return prefix + ' name="' + name + '">\n'

    frontend.forced_call = service.forced_call = forced_call


def run_owned(value, protocol):
    libc = bind_parent(value["parent_pid"])
    server = Path(value["server"])
    if not server.is_file() or not Path(value["engine_argv"][0]).is_file():
        raise ValueError("Power-owned adapter or engine artifact is missing")
    tokenizer = Path(value["tokenizer"])
    if not all((tokenizer / n).is_file() for n in ("vocab.json", "merges.txt", "token_type.json")):
        raise ValueError("Power-owned tokenizer is incomplete")
    # Upstream's optional request-content logs are disabled before importing it.
    os.environ["STRATA_TEXT_LOG"] = "0"
    os.environ.pop("STRATA_REQUEST_LINES", None)
    sys.stdout = sys.stderr
    sys.argv = [str(server)]
    module = runpy.run_path(str(server), run_name="a3s_power_owned_xolotl")
    install_dsml_tool_choice(module)
    service = module["S"]
    root = Path(module["STRATA"])
    args = value["engine_argv"]
    derived = {"exe": args[0], "args": args[2:], "cwd": str(root),
               "tokenizer": str(tokenizer), "model_name": value["model_name"],
               "env": value["engine_env"], "backend": "cuda", "log": engine_log_path(args[3]),
               "lazy_load": False, "api_monitor": False}
    def read_owned(marker):
        if marker != SENTINEL:
            raise ValueError("Power adapter refused an external settings path")
        return copy.deepcopy(derived)
    service.read_config = read_owned
    original_popen = service.popen
    def popen_owned(*a, **kw):
        before_exec = kw.get("preexec_fn")
        expected_parent = os.getpid()
        def child_owner():
            if os.getppid() != expected_parent or libc.prctl(1, signal.SIGKILL, 0, 0, 0) != 0 or os.getppid() != expected_parent:
                os._exit(126)
            if before_exec:
                before_exec()
        kw["preexec_fn"] = child_owner
        return original_popen(*a, **kw)
    service.popen = popen_owned
    original_engine = service.StrataEngine
    sent = False
    class OwnedEngine(original_engine):
        def __init__(self, *a, **kw):
            nonlocal sent
            super().__init__(*a, **kw)
            if self.ended or self.unloaded or self.proc is None or self.proc.poll() is not None:
                raise ValueError("Power-owned engine failed before its READY protocol")
            if not sent:
                publish_ready(protocol, {"schema": READY_SCHEMA, "nonce": value["nonce"],
                                         "model_name": value["model_name"], "engine_pid": self.proc.pid,
                                         "context_tokens": context_proof(self.max_context, args)})
                sent = True
    service.StrataEngine = OwnedEngine
    os.chdir(root)
    sys.argv = [str(server), "--engine", "strata", "--config", SENTINEL,
                "--host", value["host"], "--port", str(value["port"]), "--tokenizer", str(tokenizer)]
    return service.main()


def main():
    try:
        value = decode_launch(sys.stdin.buffer)
        return run_owned(value, sys.stdout)
    except (ValueError, OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        # No payload values, file contents, or upstream response bodies are emitted.
        print(f"Power-owned Xolotl startup refused: {type(error).__name__}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
