import errno
from pathlib import Path
import os
import socket
import select
import subprocess
import sys
import tempfile
import copy
import io
import json
import unittest
from unittest.mock import patch
import power_xolotl_bridge as bridge


def launch():
    return {"schema": bridge.SCHEMA, "nonce": "owned-test-nonce", "parent_pid": 123,
            "server": "/opt/xolotl/tools/v41/serve/serve_ds41.py", "tokenizer": "/models/main/tokenizer",
            "model_name": "deepseek-v41-1017-xq", "engine_argv": ["/opt/a3s/bin/ds41_generate", "--serve",
            "-m", "/models/joined.gguf", "--engram", "/models/engram", "--rank", "/models/rank-1017-v3.bin",
            "--backend", "cuda", "--experts", "gpu", *bridge.EXACT_ROUTE_ARGS], "engine_env": {"CUDA_VISIBLE_DEVICES": "0"},
            "host": "127.0.0.1", "port": 8150}


def decode(value):
    return bridge.decode_launch(io.BytesIO(json.dumps(value).encode()))


class Boundary(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "linux", "Linux descriptor handoff")
    def test_ready_channel_reaches_eof_while_adapter_remains_alive(self):
        code = """import sys,time
import power_xolotl_bridge as b
b.publish_ready(sys.stdout, {'schema':b.READY_SCHEMA,'nonce':'exact-test'})
time.sleep(30)
"""
        child = subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, cwd=Path(bridge.__file__).parent)
        try:
            self.assertEqual(json.loads(child.stdout.readline()),
                             {'schema':bridge.READY_SCHEMA,'nonce':'exact-test'})
            self.assertTrue(select.select([child.stdout], [], [], 1)[0],
                            "Power requires EOF without ending the resident adapter")
            self.assertEqual(child.stdout.read(1), b"")
            self.assertIsNone(child.poll())
        finally:
            child.terminate()
            child.wait(timeout=5)
            child.stdout.close()
            child.stderr.close()

    @unittest.skipUnless(sys.platform == "linux", "Linux systemd socket regression")
    def test_engine_log_remains_seekable_with_journal_socket_stderr(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right = socket.socketpair()
            try:
                code = """import errno,sys
from pathlib import Path
import power_xolotl_bridge as b
try:
    open('/dev/stderr', 'a')
except OSError as error:
    assert error.errno == errno.ENXIO
else:
    raise AssertionError('fixture did not reproduce the journal socket')
path=b.engine_log_path(Path(sys.argv[1])/'model.gguf')
with open(path, 'a', encoding='utf-8') as stream:
    stream.write('native startup diagnostic')
with open(path, 'r', encoding='utf-8') as stream:
    stream.seek(0)
    assert stream.read()=='native startup diagnostic'
"""
                result = subprocess.run([sys.executable, "-c", code, directory],
                                        stderr=right, stdout=subprocess.PIPE,
                                        cwd=Path(bridge.__file__).parent, timeout=5)
                self.assertEqual(result.returncode, 0)
                self.assertEqual((Path(directory)/"xolotl-engine.log").read_text(),
                                 "native startup diagnostic")
            finally:
                left.close()
                right.close()

    @unittest.skipUnless(sys.platform == "linux", "Linux file validation")
    def test_engine_log_refuses_symlink_and_fifo_without_touching_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root/"foreign.log"
            target.write_text("preserve")
            log = root/"xolotl-engine.log"
            log.symlink_to(target)
            with self.assertRaises(OSError):
                bridge.engine_log_path(root/"model.gguf")
            self.assertEqual(target.read_text(), "preserve")
            log.unlink()
            os.mkfifo(log)
            with self.assertRaises(OSError):
                bridge.engine_log_path(root/"model.gguf")

    def test_admitted_single_card_and_eight_card_records_round_trip(self):
        value = launch()
        self.assertEqual(decode(value), value)
        value["engine_env"]["CUDA_VISIBLE_DEVICES"] = "GPU-511088e2-3e4c-2a27-42c4-fd6659c7fbfc"
        self.assertEqual(decode(value), value)
        value["engine_argv"][12:12] = ["--layer-split", "5,10,15,20,25,30,35", "--split-device", "0,1,2,3,4,5,6,7"]
        value["engine_env"]["CUDA_VISIBLE_DEVICES"] = "0,1,2,3,4,5,6,7"
        self.assertEqual(decode(value), value)

    def test_one_million_context_is_closed_and_actual_ready_must_match(self):
        value = launch()
        tail = ["--ctx", "1048576", "--threads", "16", "--arena-gib", "120", "--prompt-batch", "8192", "--prefill-chunk", "512", "--pcie", "0.7500", "--tier-batch-pcie", "1.0000"]
        value["engine_argv"][-6:-6] = tail
        self.assertEqual(decode(value), value)
        self.assertEqual(bridge.context_proof(1048576, value["engine_argv"]), 1048576)
        for actual in [4096, 1048575, True, "1048576"]:
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                bridge.context_proof(actual, value["engine_argv"])
        for index, invalid in [(1, "2097152"), (1, "01048576"), (3, "0"), (5, "193"), (7, "16384"), (9, "513"), (11, "nan"), (13, "1.0001"), (0, "--mock")]:
            changed = copy.deepcopy(value); changed["engine_argv"][12+index] = invalid
            with self.subTest(index=index, invalid=invalid), self.assertRaises(ValueError):
                decode(changed)
        value["engine_argv"][12:12] = ["--layer-split", "5,10,15,20,25,30,35", "--split-device", "0,1,2,3,4,5,6,7"]
        value["engine_env"]["CUDA_VISIBLE_DEVICES"] = "0,1,2,3,4,5,6,7"
        self.assertEqual(decode(value), value)

    def test_fp4_kv_keeps_context_proof_and_device_count_closed(self):
        for split in (False, True):
            for tuned in (False, True):
                value = launch()
                if split:
                    value["engine_argv"][12:12] = ["--layer-split", "5,10,15,20,25,30,35", "--split-device", "0,1,2,3,4,5,6,7"]
                    value["engine_env"]["CUDA_VISIBLE_DEVICES"] = "0,1,2,3,4,5,6,7"
                if tuned:
                    value["engine_argv"][-6:-6] = ["--ctx", "1048576", "--threads", "16", "--arena-gib", "120", "--prompt-batch", "8192", "--prefill-chunk", "512", "--pcie", "0.7500", "--tier-batch-pcie", "1.0000"]
                value["engine_argv"].append("--kv-fp4")
                with self.subTest(split=split, tuned=tuned):
                    self.assertEqual(decode(value), value)
                    expected = 1048576 if tuned else 4096
                    self.assertEqual(bridge.context_proof(expected, value["engine_argv"]), expected)
                    for extra in (["--kv-fp4"], ["--kv-q8"], ["--mock"]):
                        changed = copy.deepcopy(value)
                        changed["engine_argv"] += extra
                        with self.assertRaises(ValueError):
                            decode(changed)

    def test_exact_routing_cannot_be_omitted_or_changed_by_the_launch_record(self):
        for suffix in [[], ["--route-bias", "1", "--route-bias-file", "0", "--route-bias-remote", "0"],
                       ["--route-bias", "0", "--route-bias-file", "0.6", "--route-bias-remote", "0"]]:
            value=launch();value["engine_argv"][-6:]=suffix
            with self.subTest(suffix=suffix), self.assertRaisesRegex(ValueError,"exact expert routing"):
                decode(value)

    def test_unknown_duplicate_and_oversized_records_refuse_before_import(self):
        value = launch()
        value["engine"] = "mock"
        with self.assertRaises(ValueError):
            decode(value)
        with self.assertRaises(ValueError):
            bridge.decode_launch(io.BytesIO(b'{"schema":"one","schema":"two"}'))
        class BoundedInput:
            requested = None
            def read(self, size):
                self.requested = size
                return b"x" * size
        stream = BoundedInput()
        with self.assertRaisesRegex(ValueError, "IPC byte bound"):
            bridge.decode_launch(stream)
        self.assertEqual(stream.requested, bridge.MAX_IPC_BYTES + 1)

    def test_device_authority_cannot_expand_or_alias_an_index(self):
        for env in [{"CUDA_VISIBLE_DEVICES": "0,1"}, {"CUDA_VISIBLE_DEVICES": "00"},
                    {"CUDA_VISIBLE_DEVICES": "4294967296"}, {"CUDA_VISIBLE_DEVICES": "0", "LD_PRELOAD": "/tmp/plugin"}]:
            value = launch(); value["engine_env"] = env
            with self.subTest(env=env), self.assertRaises(ValueError):
                decode(value)

    def test_mock_cpu_relative_and_rewritten_split_plans_refuse(self):
        for field, replacement in [(1, "--mock"), (3, "relative.gguf"), (5, "relative-engram"),
                                   (9, "cpu"), (11, "cpu")]:
            value = launch(); value["engine_argv"][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                decode(value)
        value = launch(); value["engine_argv"] += ["--layer-split", "auto", "--split-device", "0,1"]
        with self.assertRaises(ValueError):
            decode(value)

    def test_adapter_cannot_bind_a_public_interface_or_ambiguous_identity(self):
        for key, replacement in [("host", "0.0.0.0"), ("port", 65536), ("port", True),
                                 ("parent_pid", True), ("nonce", "x\nforged"), ("server", "relative.py")]:
            value = launch(); value[key] = replacement
            with self.subTest(key=key), self.assertRaises(ValueError):
                decode(value)

    def test_parent_death_binding_rechecks_identity_after_prctl(self):
        class Libc:
            called = False
            def prctl(self, *args):
                self.called = True
                return 0
        libc = Libc()
        with patch.object(bridge.sys, "platform", "linux"), patch.object(bridge.os, "getppid", side_effect=[123, 1]):
            with self.assertRaisesRegex(ValueError, "ownership was lost"):
                bridge.bind_parent(123, libc)
        self.assertTrue(libc.called)

    def test_unknown_parent_refuses_before_setting_a_death_signal(self):
        class Libc:
            def prctl(self, *args):
                raise AssertionError("foreign parent must never reach prctl")
        with patch.object(bridge.sys, "platform", "linux"), patch.object(bridge.os, "getppid", return_value=1):
            with self.assertRaisesRegex(ValueError, "ownership was lost"):
                bridge.bind_parent(123, Libc())


if __name__ == "__main__":
    unittest.main()
