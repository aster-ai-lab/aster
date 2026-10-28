from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
WORKFLOW = WORKFLOWS / "release-code.yml"
QUALIFY = WORKFLOWS / "qualify-code.yml"
CI = WORKFLOWS / "ci.yml"


class CodeReleaseWorkflowTest(unittest.TestCase):
    def test_core_acp_release_does_not_build_test_or_package_product_apps(self):
        text = WORKFLOW.read_text()
        self.assertIn("cargo build --locked --release -p a3s-code-acp --bin a3s-code-acp", text)
        self.assertNotIn("cargo test", text)
        self.assertNotIn("cargo fmt", text)
        self.assertNotIn("cargo build --locked --release -p aster", text)
        self.assertNotIn("package-aster", text)
        self.assertNotIn("release-aster-desktop", text)
        self.assertNotIn("release-aster-cli", text)

    def test_tag_publish_waits_for_qualification_and_all_cached_code_lanes(self):
        text = WORKFLOW.read_text()
        self.assertIn('tags:\n      - "v[0-9]+.[0-9]+.[0-9]+"', text)
        self.assertIn("needs: qualified", text)
        self.assertIn("needs: build", text)
        self.assertIn("actions/workflows/qualify-code.yml/runs?head_sha=${RELEASE_SHA}&event=push&status=completed", text)
        self.assertIn(".head_branch !=", text)
        self.assertIn("if: github.ref_type == 'tag'", text)
        for platform in ["macos", "linux", "windows"]:
            self.assertIn(f"platform: {platform}", text)
        self.assertIn("shared-key: code-release-${{ matrix.platform }}", text)
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 1)
        self.assertIn("expected 3 ACP release archives", text)
        self.assertIn("--verify-tag", text)
        self.assertEqual(text.count("CARGO_HOME=D:\\c"), 1)

    def test_premerge_qualification_retains_coverage_and_ignores_release_tags(self):
        for workflow in [CI, QUALIFY]:
            self.assertIn('push:\n    branches: ["**"]\n  pull_request:', workflow.read_text())
        text = QUALIFY.read_text()
        self.assertIn("name: Test ACP unit and wire contracts", text)
        self.assertIn("cargo test --locked -p a3s-code-acp --lib -- --test-threads=1", text)
        self.assertIn("cargo test --locked -p a3s-code-acp --test acp_conformance_stdio", text)
        self.assertIn("Core/ACP portable tests (windows-latest)", text)
        self.assertIn("agent::tests::unsupported_prompt_content_fails_closed_before_a_turn", text)
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 2)

    def test_desktop_has_no_ci_build_and_release_workflow(self):
        self.assertFalse((WORKFLOWS / "release-aster-desktop.yml").exists())


if __name__ == "__main__":
    unittest.main()
