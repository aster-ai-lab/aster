"""The pinned Studio resolver trusts only the declared checkout, even in a container."""
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]


class StudioCheckout(unittest.TestCase):
    def test_revision_and_build_git_trust_only_the_runner_checkout(self):
        lines = (ROOT / ".github/actions/checkout-studio/action.yml").read_text().splitlines()
        start = lines.index("      id: revision")
        start = lines.index("      run: |", start) + 1
        end = start
        while end < len(lines) and lines[end].startswith("        "):
            end += 1
        script = textwrap.dedent("\n".join(lines[start:end]))
        self.assertTrue(script)
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="studio-checkout-") as directory:
            temporary = Path(directory)
            config = temporary / "global.gitconfig"
            original = "[safe]\n\tdirectory = /unrelated-checkout\n"
            config.write_text(original)
            env = {
                "PATH": os.environ["PATH"], "HOME": directory,
                "GIT_CONFIG_GLOBAL": str(config), "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_TEST_ASSUME_DIFFERENT_OWNER": "1",
                "GITHUB_WORKSPACE": str(ROOT), "GITHUB_OUTPUT": str(temporary / "output"),
            }
            refused = subprocess.run(["git", "ls-tree", "HEAD", "apps/studio"], cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("dubious ownership", refused.stderr)
            accepted = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            output = dict(line.split("=", 1) for line in (temporary / "output").read_text().splitlines())
            self.assertEqual(set(output), {"sha", "power_sha"})
            for key, path in [("sha", "apps/studio"), ("power_sha", "crates/power")]:
                expected = subprocess.check_output(["git", "rev-parse", "HEAD:" + path], cwd=ROOT, env=env, text=True).strip()
                self.assertEqual(output[key], expected)
            elsewhere = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=temporary, env=env, text=True, capture_output=True, timeout=10)
            self.assertEqual(elsewhere.returncode, 0, elsewhere.stderr)
            downstream = subprocess.run(["git", "ls-files", "--error-unmatch", "apps/aster/Cargo.toml"], cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
            self.assertEqual(downstream.returncode, 0, downstream.stderr)
            safe = subprocess.check_output(["git", "config", "--global", "--get-all", "safe.directory"], env=env, text=True).splitlines()
            self.assertEqual(set(safe), {"/unrelated-checkout", str(ROOT)})
            config.write_text(original)
            env["GITHUB_WORKSPACE"] = str(temporary)
            wrong_checkout = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
            self.assertNotEqual(wrong_checkout.returncode, 0)
            safe = subprocess.check_output(["git", "config", "--global", "--get-all", "safe.directory"], env=env, text=True).splitlines()
            self.assertEqual(set(safe), {"/unrelated-checkout", str(temporary)})
            config.write_text(original)
            env["GITHUB_WORKSPACE"] = ""
            missing = subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
            self.assertNotEqual(missing.returncode, 0)
            self.assertEqual(config.read_text(), original)


if __name__ == "__main__":
    unittest.main()
