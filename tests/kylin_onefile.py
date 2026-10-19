"""Exercise the offline installer's real extraction path without installing software."""

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


if __name__ == "__main__":
    unittest.main()
