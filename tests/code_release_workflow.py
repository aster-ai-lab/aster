from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release-code.yml"


class CodeReleaseWorkflowTest(unittest.TestCase):
    def test_core_acp_release_does_not_build_or_package_product_apps(self):
        text = WORKFLOW.read_text()
        self.assertIn("cargo build --locked --release -p a3s-code-acp --bin a3s-code-acp", text)
        self.assertIn("cargo test --locked -p a3s-code-core", text)
        self.assertIn("cargo test --locked -p a3s-code-acp", text)
        self.assertNotIn("cargo build --locked --release -p aster", text)
        self.assertNotIn("package-aster", text)
        self.assertNotIn("release-aster-desktop", text)
        self.assertNotIn("release-aster-cli", text)

    def test_publish_waits_for_all_code_lanes_and_a_version_tag(self):
        text = WORKFLOW.read_text()
        self.assertIn("needs: [test, build]", text)
        self.assertIn("if: github.ref_type == 'tag'", text)
        self.assertIn("os: [ubuntu-latest, macos-latest, windows-latest]", text)
        self.assertIn("expected 3 ACP release archives", text)
        self.assertIn("--verify-tag", text)
        self.assertEqual(text.count("CARGO_HOME=D:\\c"), 2)
        self.assertIn("protobuf-compiler cmake pkg-config libssl-dev bubblewrap", text)


if __name__ == "__main__":
    unittest.main()
