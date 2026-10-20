"""Exercise the offline installer's real extraction path without installing software."""

import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class KylinOnefileTest(unittest.TestCase):
    def test_versioned_installer_extracts_and_rejects_corruption(self):
        with tempfile.TemporaryDirectory(prefix="aster-kylin-onefile-") as temporary:
            work = Path(temporary)
            scripts = work / "scripts/kylin-flatpak"
            scripts.mkdir(parents=True)
            script = scripts / "pack-onefile.sh"
            shutil.copyfile(ROOT / "apps/work/scripts/kylin-flatpak/pack-onefile.sh", script)
            (work / "src-tauri").mkdir()
            (work / "src-tauri/tauri.conf.json").write_text(json.dumps({"version": "0.1.0"}))
            dist = work / "dist/kylin"
            dist.mkdir(parents=True)
            archive = dist / "aster_0.1.0_kylin-v10_x86_64.tar.gz"
            payload = b'printf "installed\\n" > "$KYLIN_ONEFILE_MARKER"\n'
            with tarfile.open(archive, "w:gz") as bundle:
                entry = tarfile.TarInfo("install-on-kylin.sh")
                entry.size = len(payload)
                bundle.addfile(entry, io.BytesIO(payload))
            subprocess.run(["bash", str(script)], check=True, capture_output=True, text=True)
            installer = dist / "aster_0.1.0_kylin-v10_x86_64.run"
            self.assertTrue(os.access(installer, os.X_OK))
            marker = work / "installed"
            # The fixture installer has no privileged operations, including in root CI.
            binaries = work / "bin"
            binaries.mkdir()
            fake_id = binaries / "id"
            fake_id.write_text("#!/bin/sh\nprintf '1000\\n'\n")
            fake_id.chmod(0o755)
            fake_df = binaries / "df"
            fake_df.write_text("#!/bin/sh\nprintf 'Filesystem 1024-blocks Used Available Capacity Mounted on\\nfixture 99999999 0 99999999 0%% /fixture\\n'\n")
            fake_df.chmod(0o755)
            env = {**os.environ, "PATH": str(binaries) + os.pathsep + os.environ["PATH"],
                   "KYLIN_ONEFILE_MARKER": str(marker)}
            installed = subprocess.run(["bash", str(installer)], env=env, capture_output=True, text=True)
            self.assertEqual(installed.returncode, 0, installed.stdout + installed.stderr)
            self.assertEqual(marker.read_text(), "installed\n")
            marker.unlink()
            with installer.open("r+b") as file:
                file.truncate(installer.stat().st_size - 12)
            refused = subprocess.run(["bash", str(installer)], env=env, capture_output=True)
            self.assertNotEqual(refused.returncode, 0)
            self.assertFalse(marker.exists())

    def test_insufficient_extraction_space_refuses_before_running_installer(self):
        with tempfile.TemporaryDirectory(prefix="aster-kylin-onefile-space-") as temporary:
            work = Path(temporary)
            archive = work / "payload.tar.gz"
            marker = work / "installed"
            payload = b'printf "installed\\n" > "$KYLIN_ONEFILE_MARKER"\n'
            with tarfile.open(archive, "w:gz") as bundle:
                for name, data in (("install-on-kylin.sh", payload), ("compressed-data", bytes(8 * 1024 * 1024))):
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    bundle.addfile(entry, io.BytesIO(data))
            installer = work / "install.run"
            subprocess.run(["bash", str(ROOT / "apps/work/scripts/kylin-flatpak/pack-onefile.sh"),
                            str(archive), str(installer)], check=True, capture_output=True, text=True)
            binaries = work / "bin"
            binaries.mkdir()
            extraction_marker = work / "extracted"
            for name, script in {
                "id": "printf '1000\\n'",
                "df": "printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\\ntmpfs 8192 4096 4096 50%% /tmp\\n'",
                "tar": 'printf "called\\n" > "$KYLIN_ONEFILE_EXTRACTION_MARKER"; exit 77',
            }.items():
                executable = binaries / name
                executable.write_text("#!/bin/sh\n" + script + "\n")
                executable.chmod(0o755)
            env = {**os.environ, "PATH": str(binaries) + os.pathsep + os.environ["PATH"],
                   "KYLIN_ONEFILE_MARKER": str(marker), "TMPDIR": str(work),
                   "KYLIN_ONEFILE_EXTRACTION_MARKER": str(extraction_marker)}
            refused = subprocess.run(["bash", str(installer)], env=env, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0, refused.stdout + refused.stderr)
            self.assertIn("解压空间不足", refused.stderr)
            self.assertFalse(marker.exists())
            self.assertFalse(extraction_marker.exists())


class KylinInstallerSpaceTest(unittest.TestCase):
    def run_preflight(self, package_free_kb, destination_free_kb, nested_destination=False, custom_flatpak_dir=False):
        with tempfile.TemporaryDirectory(prefix="aster-kylin-install-space-") as temporary:
            work = Path(temporary)
            package = work / "extracted-payload"
            package.mkdir()
            data = work / "desktop-data"
            data.mkdir()
            destination = data / "not-created/yet" if nested_destination else data
            probe = data
            if custom_flatpak_dir:
                probe = work / "custom-flatpak-volume"
                probe.mkdir()
            script = package / "install-on-kylin.sh"
            shutil.copyfile(ROOT / "apps/work/scripts/kylin-flatpak/install-on-kylin.sh", script)
            hashes = []
            for name in ("gnome-platform-50.flatpak", "gl-default-25.08.flatpak", "dev.a3s.AsterDream.flatpak"):
                payload = b"hermetic bundle fixture\n"
                (package / name).write_bytes(payload)
                hashes.append(hashlib.sha256(payload).hexdigest() + "  " + name)
            (package / "SHA256SUMS").write_text("\n".join(hashes) + "\n")
            binaries = work / "bin"
            binaries.mkdir()
            for name, body in {
                "id": "printf '1000\\n'",
                "uname": "printf 'x86_64\\n'",
                "flatpak": "printf 'Flatpak 1.6.5\\n'",
                "unshare": "exit 0",
                "df": """printf '%s\\n' "$2" >> "$KYLIN_SPACE_PATH_LOG"
case "$2" in
  "$KYLIN_SPACE_DESTINATION") free="$KYLIN_SPACE_DESTINATION_FREE" ;;
  .|"$KYLIN_SPACE_PACKAGE") free="$KYLIN_SPACE_PACKAGE_FREE" ;;
  *) printf 'unexpected disk probe: %s\\n' "$2" >&2; exit 1 ;;
esac
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\\nfixture 99999999 0 %s 0%% /fixture\\n' "$free"
""",
            }.items():
                executable = binaries / name
                executable.write_text("#!/bin/sh\n" + body + "\n")
                executable.chmod(0o755)
            path_log = work / "disk-probes"
            env = {"PATH": str(binaries) + os.pathsep + os.environ["PATH"],
                   "XDG_DATA_HOME": str(destination), "KYLIN_INSTALL_DRY_RUN": "1",
                   "KYLIN_SPACE_DESTINATION": str(probe), "KYLIN_SPACE_PACKAGE": str(package),
                   "KYLIN_SPACE_DESTINATION_FREE": str(destination_free_kb),
                   "KYLIN_SPACE_PACKAGE_FREE": str(package_free_kb), "KYLIN_SPACE_PATH_LOG": str(path_log)}
            if custom_flatpak_dir:
                env["FLATPAK_USER_DIR"] = str(probe / "flatpak")
            result = subprocess.run(["bash", str(script)], env=env, capture_output=True, text=True)
            probes = path_log.read_text().splitlines() if path_log.exists() else []
            return result, probes, str(probe)

    def test_small_temporary_volume_does_not_block_ample_destination(self):
        result, probes, destination = self.run_preflight(1500000, 20000000)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("DRY_RUN", result.stdout)
        self.assertEqual(probes, [destination])

    def test_full_destination_refuses_despite_ample_temporary_volume(self):
        result, probes, destination = self.run_preflight(20000000, 1500000)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("安装目录所在磁盘剩余不足", result.stderr)
        self.assertEqual(probes, [destination])

    def test_uncreated_data_directory_checks_its_existing_parent(self):
        result, probes, destination = self.run_preflight(20000000, 20000000, nested_destination=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(probes, [destination])

    def test_custom_flatpak_user_directory_takes_precedence(self):
        result, probes, destination = self.run_preflight(20000000, 20000000, custom_flatpak_dir=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(probes, [destination])

    def test_unreadable_capacity_refuses_instead_of_skipping_check(self):
        result, probes, destination = self.run_preflight(20000000, "unavailable")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("无法确认安装目录所在磁盘的剩余空间", result.stderr)
        self.assertEqual(probes, [destination])


if __name__ == "__main__":
    unittest.main()
