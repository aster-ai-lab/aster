"""Path regimes in the constitution and the development standards.

The check reads those two documents plus the root, api, and cloud
manifests. A restored exclusion sentence fails the same function on
an in-memory copy. The files on disk are the positive case.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONSTITUTION = ROOT / ".specify" / "memory" / "constitution.md"
STANDARDS = ROOT / "docs" / "development-standards.md"

EXCLUSIONS = (
    "不是本仓库原则",
    "are not principles of this repository",
    "Do not import its obligations",
)

CONTROL_TOKENS = ("dual-adapter", "CLOUD_MIGRATION_COUNT", "#[controller]")


class RegimeError(Exception):
    """A governance document or workspace manifest broke a regime rule."""


def workspace_members(text):
    """Return the root workspace member list. Fail closed on a bad read."""
    in_workspace = False
    in_members = False
    members = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]") and not in_members:
            in_workspace = stripped == "[workspace]"
            continue
        if not in_workspace:
            continue
        if not in_members:
            if stripped.startswith("members") and "[" in stripped:
                in_members = "]" not in stripped.split("[", 1)[1]
                body = stripped.split("[", 1)[1].split("]", 1)[0]
                members.extend(_quoted(body))
            continue
        if "]" in stripped:
            members.extend(_quoted(stripped.split("]", 1)[0]))
            break
        members.extend(_quoted(stripped))
    if not members:
        raise RegimeError("workspace members were not read")
    return members


def _quoted(text):
    found = []
    rest = text
    while True:
        start = rest.find('"')
        if start < 0:
            return found
        end = rest.find('"', start + 1)
        if end < 0:
            raise RegimeError("unclosed member string")
        found.append(rest[start + 1 : end])
        rest = rest[end + 1 :]


def has_workspace_table(text):
    return any(line.strip() == "[workspace]" for line in text.splitlines())


def section(text, heading):
    marker = f"### {heading}\n"
    start = text.find(marker)
    if start < 0:
        raise RegimeError(f"missing section {heading}")
    rest = text[start + len(marker) :]
    nxt = rest.find("\n### ")
    if nxt < 0:
        nxt = rest.find("\n## ")
    return rest if nxt < 0 else rest[:nxt]


def check_regimes(text):
    """Reject an exclusion and require both regimes in one document."""
    for phrase in EXCLUSIONS:
        if phrase in text:
            raise RegimeError(phrase)
    product = section(text, "Product regime")
    control = section(text, "Control-plane regime")
    if "CLOUD_MIGRATION_COUNT" in product:
        raise RegimeError("CLOUD_MIGRATION_COUNT is a product obligation")
    missing = [token for token in CONTROL_TOKENS if token not in control]
    if missing:
        raise RegimeError("control-plane regime missing " + ", ".join(missing))


def check_workspace_split(root_text, api_text, cloud_text):
    """api and cloud stay out of the root workspace and keep their own."""
    members = workspace_members(root_text)
    if "apps/api" in members:
        raise RegimeError("apps/api is a root workspace member")
    if any(member == "apps/cloud" or member.startswith("apps/cloud/") for member in members):
        raise RegimeError("apps/cloud is a root workspace member")
    if not has_workspace_table(api_text):
        raise RegimeError("apps/api has no workspace table")
    if not has_workspace_table(cloud_text):
        raise RegimeError("apps/cloud has no workspace table")


class GovernanceRegimeTest(unittest.TestCase):
    def test_constitution_and_standards_state_both_regimes(self):
        for path in (CONSTITUTION, STANDARDS):
            check_regimes(path.read_text(encoding="utf-8"))

    def test_restored_exclusion_is_rejected(self):
        for path in (CONSTITUTION, STANDARDS):
            text = path.read_text(encoding="utf-8")
            check_regimes(text)
            for phrase in EXCLUSIONS:
                with self.assertRaises(RegimeError) as caught:
                    check_regimes(text + "\n" + phrase + "\n")
                self.assertIn(phrase, str(caught.exception))

    def test_obligation_regressions_are_rejected(self):
        for path in (CONSTITUTION, STANDARDS):
            text = path.read_text(encoding="utf-8")
            product_marker = "### Product regime\n"
            obliged = text.replace(
                product_marker,
                product_marker + "Obligations include CLOUD_MIGRATION_COUNT.\n",
                1,
            )
            with self.assertRaises(RegimeError) as caught:
                check_regimes(obliged)
            self.assertIn("product obligation", str(caught.exception))
            for token in CONTROL_TOKENS:
                stripped = text.replace(token, "removed-token", 1)
                with self.assertRaises(RegimeError) as caught:
                    check_regimes(stripped)
                self.assertIn(token, str(caught.exception))

    def test_api_and_cloud_keep_separate_workspaces(self):
        root = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        api = (ROOT / "apps/api/Cargo.toml").read_text(encoding="utf-8")
        cloud = (ROOT / "apps/cloud/Cargo.toml").read_text(encoding="utf-8")
        check_workspace_split(root, api, cloud)
        with self.assertRaises(RegimeError):
            check_workspace_split(
                root.replace('"apps/aster",', '"apps/api",', 1),
                api,
                cloud,
            )
        with self.assertRaises(RegimeError):
            check_workspace_split(
                root.replace(
                    '"apps/aster",',
                    '"apps/aster",\n    "apps/cloud/crates/control-plane",',
                    1,
                ),
                api,
                cloud,
            )
        dropped = "\n".join(line for line in api.splitlines() if line.strip() != "[workspace]")
        with self.assertRaises(RegimeError):
            check_workspace_split(root, dropped, cloud)
        dropped_cloud = "\n".join(
            line for line in cloud.splitlines() if line.strip() != "[workspace]"
        )
        with self.assertRaises(RegimeError):
            check_workspace_split(root, api, dropped_cloud)


if __name__ == "__main__":
    unittest.main()
