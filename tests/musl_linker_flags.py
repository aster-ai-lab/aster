"""Compile tiny static ELF programs with the repository's actual musl flags."""
import pathlib
import struct
import subprocess
import tempfile
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


def program_headers(data):
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01":
        raise ValueError("expected a complete little-endian ELF64 header")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    if size != 56 or not 0 < count <= 4096 or offset + size * count > len(data):
        raise ValueError("invalid ELF64 program-header table")
    return [struct.unpack_from("<II", data, offset + size * i) for i in range(count)]


class MuslLinkerFlags(unittest.TestCase):
    def test_generated_static_elf_retains_relro_and_a_non_executable_stack(self):
        flags = tomllib.loads((ROOT / ".cargo/config.toml").read_text())["target"]
        sysroot = pathlib.Path(subprocess.check_output(["rustc", "--print", "sysroot"], cwd=ROOT, text=True).strip())
        version = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
        host = next(line[6:] for line in version.splitlines() if line.startswith("host: "))
        linker = sysroot / "lib/rustlib" / host / "bin/rust-lld"
        self.assertTrue(linker.is_file(), "pinned Rust linker is required")
        for target, machine in [("aarch64-unknown-linux-musl", 183), ("x86_64-unknown-linux-musl", 62)]:
            with self.subTest(target=target), tempfile.TemporaryDirectory(prefix="aster-musl-link-") as directory:
                self.assertTrue((sysroot / "lib/rustlib" / target / "lib").is_dir(), f"install the explicit qualification target {target}")
                output = pathlib.Path(directory) / "probe"
                command = ["rustc", "--target", target, "--crate-name", "musl_link_probe", *flags[target]["rustflags"], "-C", f"linker={linker}", "-o", str(output), "-"]
                result = subprocess.run(command, cwd=ROOT, input="fn main() {}\n", text=True, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr[-8192:])
                data = output.read_bytes()
                self.assertEqual(struct.unpack_from("<H", data, 18)[0], machine)
                headers = program_headers(data)
                stacks = [value for kind, value in headers if kind == 0x6474E551]
                self.assertEqual(len(stacks), 1)
                self.assertEqual(stacks[0] & 1, 0, "PT_GNU_STACK must not be executable")
                self.assertIn(0x6474E552, [kind for kind, _ in headers], "PT_GNU_RELRO must exist")
                self.assertNotIn(3, [kind for kind, _ in headers], "static guest must not need a PT_INTERP loader")

    def test_truncated_or_unsupported_elf_tables_are_refused(self):
        valid = bytearray(120)
        valid[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<Q", valid, 32, 64)
        struct.pack_into("<HH", valid, 54, 56, 1)
        self.assertEqual(len(program_headers(valid)), 1)
        unsupported = bytearray(valid)
        unsupported[4] = 1
        for data in [b"", valid[:63], valid[:-1], unsupported]:
            with self.subTest(length=len(data)), self.assertRaises(ValueError):
                program_headers(data)


if __name__ == "__main__":
    unittest.main()
