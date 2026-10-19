"""Team admin P2 capabilities acceptance on the owned ToWork window, cua-driver background mode only.

Chain: 团队能力 opens on 全部允许 with an empty list → 仅限下方清单 + 添加 picks one connector →
保存 writes revision + 1 → the owner, a member of that team, is refused another connector on
工作台 / 连接器 with the team-rule toast and may install the listed one → the audit feed and page
carry 修改团队能力 → a member without `manage Capability` gets not-found for the policy, is
refused the unlisted connector with 403 and may install the listed one; a stale revision is a
409 → 全部允许 again lifts the rule.

Every install made here is removed again, and the policy is reset to open before and after.

Usage:
  A3S_E2E_SESSION_FROM=<member-session.json> A3S_E2E_TEAM=<team> python3 cua_p2_capabilities.py run [evidence-dir]
"""

from __future__ import annotations

import json
import sys
import time
import urllib.parse
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cua_p0 import SECRET, T, Run, Ui, fail_closed, launch, norm, wait_api  # noqa: E402
from cua_p1 import REVIEWER, call, creator_token  # noqa: E402
from cua_p2_runs import refused  # noqa: E402

# Catalog names are unique for these two; two different connectors are both called 时间服务,
# which is why the listed one is the fetch connector.
LISTED = {"kind": "connector", "slug": "mcp-modelcontextprotocol-fetch", "name": "Fetch网页内容抓取"}
UNLISTED = {"kind": "connector", "slug": "mcp-modelcontextprotocol-time", "name": "时间服务"}
CATALOG = "/v1/api/org/capabilities"


def status_of(path: str, method: str, body: Any, token: str) -> tuple[int, str]:
    try:
        call(path, method, body, token)
    except RuntimeError as exc:
        text = str(exc)
        for code in (400, 403, 404, 409, 422, 500):
            if f" -> {code} " in text:
                return code, text
        raise
    return 200, ""


class Capabilities(Run):
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        super().__init__(ui, team, evidence)
        self.reviewer: dict[str, Any] = {}
        self.saved_revision = -1

    # ── API helpers ─────────────────────────────────────────────────

    def policy_path(self) -> str:
        return f"/v1/api/org/members/orgs/{self.org_id}/capability-policy"

    def policy(self) -> dict[str, Any]:
        return call(self.policy_path(), token=creator_token())

    def reset_policy(self) -> None:
        current = self.policy()
        if current["restricted"] or current["allowed"]:
            call(
                self.policy_path(),
                "PUT",
                {"restricted": False, "allowed": [], "expected_revision": current["revision"]},
                creator_token(),
            )

    def installed(self, slug: str, token: str) -> bool:
        query = urllib.parse.urlencode({"kind": "connector", "page": 1, "pageSize": 50, "q": slug})
        page = call(f"{CATALOG}?{query}", token=token)
        rows = page.get("items", page) if isinstance(page, dict) else page
        return any(row.get("slug") == slug and row.get("installed") for row in rows)

    def uninstall(self, slug: str, token: str) -> None:
        code, text = status_of(f"{CATALOG}/installations/{slug}", "DELETE", None, token)
        if code not in (200, 404):
            raise AssertionError(f"uninstall {slug} answered {code}: {SECRET.sub('<redacted>', text)[:160]}")

    def clean_installs(self) -> None:
        for token in (creator_token(), self.reviewer.get("token")):
            if token:
                for item in (LISTED, UNLISTED):
                    self.uninstall(item["slug"], token)

    # ── page helpers ────────────────────────────────────────────────

    def open_panel(self) -> None:
        """A failed attempt can leave the picker open or a draft unsaved; both are dropped."""
        ui = self.ui
        for attempt in range(3):
            ui.refresh()
            if ui.items(T("org.capabilities.picker.title"), None, exact=True):
                ui.press(T("org.capabilities.picker.done"), "AXButton", exact=True)
                ui.gone(T("org.capabilities.picker.title"), None, exact=True)
            if ui.items(T("org.capabilities.discard"), "AXButton", exact=True):
                ui.press(T("org.capabilities.discard"), "AXButton", exact=True)
                ui.gone(T("org.capabilities.unsaved"), None, exact=True)
            if ui.items(T("org.capabilities.modeTitle"), None, exact=True):
                return
            try:
                self.enter_team()
                ui.nav(T("org.nav.capabilities"))
                ui.seen(T("org.capabilities.modeTitle"), None, exact=True)
                return
            except AssertionError:
                if attempt == 2:
                    raise
                time.sleep(2)

    def mode(self, key: str) -> dict[str, Any]:
        return self.ui.seen(T(key), "AXRadioButton", exact=False)

    def checked(self, key: str) -> bool:
        return str(self.mode(key).get("value")) == "1"

    def open_connectors(self) -> None:
        ui = self.ui
        ui.refresh()
        if not ui.items(T("workbench.nav.connectors"), "AXButton"):
            ui.press(T("org.nav.workbench"), "AXButton")
        ui.nav(T("workbench.nav.connectors"))
        self.search_field()

    def search_field(self) -> dict[str, Any]:
        placeholder = T("capabilities.search.connectors")
        ui = self.ui

        def find() -> list[dict[str, Any]]:
            fields = [e for e in ui.elements if e.get("role") == "AXTextField"]
            named = [e for e in fields if placeholder in " ".join(norm(e.get(k)) for k in ("label", "value", "placeholder", "help", "title"))]
            return named[:1] or [e for e in fields if float((e.get("frame") or {}).get("x", 0)) > 300][:1]

        return ui.wait(find, "the connector search field")[0]

    def search(self, text: str) -> None:
        ui = self.ui
        ui.trace.append(f"search {text}")
        ui.close_siblings()
        field = self.search_field()
        if norm(field.get("value")) and norm(field.get("value")) != T("capabilities.search.connectors"):
            ui.hit(lambda: [self.search_field()], "clear search", "set_value", {"value": ""})
            time.sleep(0.4)
        ui.hit(lambda: [self.search_field()], "connector search", "type_text", {"text": text, "delivery_mode": "background", "delay_ms": 0})
        ui.seen(text, "AXHeading", exact=True, timeout=20)
        time.sleep(0.8)

    def card_button(self, name: str, label: str) -> dict[str, Any] | None:
        """The `label` button on the row of the first card titled `name`."""
        ui = self.ui
        heads = ui.items(name, "AXHeading", exact=True)
        buttons = [e for e in ui.items(label, "AXButton", exact=True) if e.get("enabled") is not False]
        if not heads or not buttons:
            return None
        head = heads[0].get("frame") or {}
        top = float(head.get("y", 0))
        on_row = [b for b in buttons if abs(float((b.get("frame") or {}).get("y", 0)) - top) < 40]
        if not on_row:
            return None
        return min(on_row, key=lambda b: abs(float((b.get("frame") or {}).get("y", 0)) - top))

    def install_from_card(self, name: str) -> None:
        self.ui.trace.append(f"install {name}")
        self.ui.hit(lambda: self.card_button(name, T("capabilities.install")), f"install on {name}")
        time.sleep(0.6)

    # ── cases ───────────────────────────────────────────────────────

    def c01_team_center(self) -> None:
        orgs = call("/v1/api/org/members/orgs", token=creator_token())
        self.org_id = next(org["id"] for org in orgs if org.get("name") == self.team)
        rows = call(f"/v1/api/org/members/orgs/{self.org_id}/members", token=creator_token())
        rows = rows.get("items", rows) if isinstance(rows, dict) else rows
        if not REVIEWER.is_file():
            raise AssertionError("no second member; run cua_p1.py setup first")
        self.reviewer = json.loads(REVIEWER.read_text())
        if not any(row.get("member_id") == self.reviewer["member_id"] for row in rows):
            raise AssertionError("the reviewer is not in the team; run cua_p1.py setup first")
        self.reset_policy()
        self.clean_installs()
        self.enter_team()

    def c02_open(self) -> None:
        """Open by default: 全部允许 is checked and the list is empty and marked inactive."""
        ui = self.ui
        self.open_panel()
        ui.seen(T("org.nav.group.admin"), None, exact=True)
        if not self.checked("org.capabilities.mode.open") or self.checked("org.capabilities.mode.restricted"):
            raise AssertionError("a team without a policy does not read 全部允许")
        ui.seen(T("org.capabilities.empty"), None, exact=True)
        ui.seen(T("org.capabilities.listInactive"), None, exact=True)
        ui.absent(T("org.capabilities.unsaved"), None)
        self.record("c02_open")
        self.grab("c02_open")

    def c03_restrict(self) -> None:
        """仅限下方清单, pick one connector in the picker, save; the API reads it back."""
        ui = self.ui
        self.open_panel()
        before = self.policy()["revision"]
        ui.hit(lambda: [self.mode("org.capabilities.mode.restricted")], "仅限下方清单")
        ui.wait(lambda: self.checked("org.capabilities.mode.restricted"), "仅限下方清单 checked")
        ui.seen(T("org.capabilities.emptyRestricted"), None, exact=True)
        ui.seen(T("org.capabilities.unsaved"), None, exact=True)
        ui.press(T("org.capabilities.add"), "AXButton", exact=True)
        ui.seen(T("org.capabilities.picker.title"), None, exact=True)
        ui.type_into(T("org.capabilities.picker.search"), LISTED["name"])
        # WebKit exposes an aria-pressed toggle as a checkbox.
        pick = T("org.capabilities.picker.pick", name=LISTED["name"])
        ui.seen(pick, "AXCheckBox", exact=True, timeout=20)
        ui.press(pick, "AXCheckBox", exact=True)
        ui.seen(T("org.capabilities.picker.unpick", name=LISTED["name"]), "AXCheckBox", exact=True)
        self.grab("c03_picker")
        ui.press(T("org.capabilities.picker.done"), "AXButton", exact=True)
        ui.gone(T("org.capabilities.picker.title"), None, exact=True)
        ui.seen(T("org.capabilities.remove", name=LISTED["name"]), "AXButton", exact=True)
        ui.seen(T("org.capabilities.listTitle", count=1), None, exact=True)
        ui.press(T("org.capabilities.save"), "AXButton", exact=True)
        ui.gone(T("org.capabilities.unsaved"), None, exact=True, timeout=15)
        policy = self.policy()
        allowed = [{"kind": a["kind"], "slug": a["slug"]} for a in policy["allowed"]]
        if not policy["restricted"] or allowed != [{"kind": LISTED["kind"], "slug": LISTED["slug"]}]:
            raise AssertionError(f"the saved policy reads restricted={policy['restricted']} allowed={allowed}")
        if policy["revision"] != before + 1:
            raise AssertionError(f"revision {before} -> {policy['revision']}")
        self.saved_revision = policy["revision"]
        print(f"  saved revision {self.saved_revision}")
        self.record("c03_restrict")
        self.grab("c03_restrict")

    def c04_install_gate(self) -> None:
        """The owner is a member of the team: the unlisted connector is refused with the team
        toast, the listed one installs."""
        ui = self.ui
        token = creator_token()
        self.open_connectors()
        self.search(UNLISTED["name"])
        self.install_from_card(UNLISTED["name"])
        ui.seen(T("capabilities.install_team_refused", name=UNLISTED["name"]), None, exact=False, timeout=20)
        if self.installed(UNLISTED["slug"], token):
            raise AssertionError("the unlisted connector was installed")
        self.record("c04_refused")
        self.grab("c04_refused")
        self.search(LISTED["name"])
        self.install_from_card(LISTED["name"])
        ui.wait(lambda: self.installed(LISTED["slug"], token), f"{LISTED['name']} installed", timeout=60)
        ui.wait(lambda: self.card_button(LISTED["name"], T("capabilities.install")) is None, "the install button to leave")
        self.record("c04_listed")
        self.grab("c04_listed")
        self.uninstall(LISTED["slug"], token)

    def c05_audit(self) -> None:
        """The change is one audit entry: 全部允许 → 仅限清单（1 项）."""
        ui = self.ui
        feed = call(f"/v1/api/org/members/orgs/{self.org_id}/audit?days=1&limit=20", token=creator_token())
        entries = feed["entries"]
        change = next((e for e in entries if e.get("action") == "org.capabilities.changed"), None)
        if change is None:
            raise AssertionError("no org.capabilities.changed entry in the audit feed")
        to = change.get("to") or {}
        if to.get("kind") != "capabilities" or to.get("code") != "restricted" or to.get("count") != 1:
            raise AssertionError(f"the audit entry reads to={to}")
        self.enter_team()
        ui.nav(T("org.nav.audit"))
        ui.seen(T("org.audit.action.org_capabilities_changed"), None, exact=True, timeout=20)
        ui.seen(T("org.audit.capabilitiesRestricted", count=1), None, exact=False)
        self.record("c05_audit")
        self.grab("c05_audit")

    def c06_scope(self) -> None:
        """A member without `manage Capability` gets not-found for the policy and is bound by it;
        a stale revision is a conflict."""
        token = self.reviewer["token"]
        refused(self.policy_path(), token=token)
        refused(self.policy_path(), "PUT", {"restricted": False, "allowed": [], "expected_revision": self.saved_revision}, token)
        code, text = status_of(f"{CATALOG}/installations/{UNLISTED['slug']}", "PUT", None, token)
        if code != 403 or "does not allow" not in text:
            raise AssertionError(f"the reviewer's unlisted install answered {code}")
        code, text = status_of(f"{CATALOG}/installations/{LISTED['slug']}", "PUT", None, token)
        if code != 200:
            raise AssertionError(f"the reviewer's listed install answered {code}: {SECRET.sub('<redacted>', text)[:160]}")
        self.uninstall(LISTED["slug"], token)
        stale = {"restricted": False, "allowed": [], "expected_revision": self.saved_revision - 1}
        code, _ = status_of(self.policy_path(), "PUT", stale, creator_token())
        if code != 409:
            raise AssertionError(f"a stale revision answered {code}")
        if self.policy()["revision"] != self.saved_revision:
            raise AssertionError("a refused write moved the revision")
        self.record("c06_scope")

    def c07_reopen(self) -> None:
        """全部允许 again: the rule is lifted for every member."""
        ui = self.ui
        self.open_panel()
        ui.hit(lambda: [self.mode("org.capabilities.mode.open")], "全部允许")
        ui.wait(lambda: self.checked("org.capabilities.mode.open"), "全部允许 checked")
        ui.seen(T("org.capabilities.listInactive"), None, exact=True)
        ui.press(T("org.capabilities.save"), "AXButton", exact=True)
        ui.gone(T("org.capabilities.unsaved"), None, exact=True, timeout=15)
        policy = self.policy()
        if policy["restricted"] or policy["revision"] != self.saved_revision + 1:
            raise AssertionError(f"reopen reads restricted={policy['restricted']} revision={policy['revision']}")
        token = self.reviewer["token"]
        code, text = status_of(f"{CATALOG}/installations/{UNLISTED['slug']}", "PUT", None, token)
        if code != 200:
            raise AssertionError(f"after reopening, the reviewer's install answered {code}: {SECRET.sub('<redacted>', text)[:160]}")
        self.uninstall(UNLISTED["slug"], token)
        self.record("c07_reopen")
        self.grab("c07_reopen")


CASES = ["c01_team_center", "c02_open", "c03_restrict", "c04_install_gate", "c05_audit", "c06_scope", "c07_reopen"]


def run(ui: Ui, team: str, evidence: Path | None) -> int:
    session = Capabilities(ui, team, evidence)
    lines: list[str] = []
    failed = 0
    try:
        for case in CASES:
            started = time.time()
            line = ""
            # The shared dev server reloads the page whenever any session edits the renderer, so a
            # case gets one retry.
            for attempt in (1, 2):
                ui.trace.clear()
                try:
                    wait_api()
                    ui.close_siblings()
                    ui.expose()
                    if case not in ("c01_team_center", "c04_install_gate", "c06_scope"):
                        session.enter_team()
                    getattr(session, case)()
                except Exception as exc:
                    reason = SECRET.sub("<redacted>", str(exc))[:300]
                    steps = " | ".join(ui.trace[-6:])
                    print(f"  {case} attempt {attempt}: {reason} after [{steps}]", file=sys.stderr)
                    if evidence is not None:
                        session.record(f"{case}_failure_{attempt}")
                    line = f"FAIL {case} {reason}"
                else:
                    retried = " (passed on retry after a page reset)" if attempt > 1 else ""
                    line = f"PASS {case} {time.time() - started:.1f}s{retried}"
                    break
            print(line)
            lines.append(line)
            sys.stdout.flush()
            if line.startswith("FAIL"):
                failed += 1
                break
    finally:
        if session.org_id:
            session.reset_policy()
            session.clean_installs()
            print("policy reset to open; test installs removed")
    routes = sorted({f"{name}:{route or '-'}:{mode or '-'}" for name, route, mode in ui.routes})
    print("routes " + ", ".join(routes))
    print(f"done passed={len(lines) - failed} failed={failed} of {len(CASES)}")
    if evidence is not None:
        evidence.mkdir(parents=True, exist_ok=True)
        (evidence / "console.txt").write_text("\n".join(lines + ["routes " + ", ".join(routes)]) + "\n")
    return 1 if failed or len(lines) < len(CASES) else 0


def main() -> None:
    source, team = fail_closed()
    pid, window = launch(source)
    print(f"owned pid={pid} window={window}")
    ui = Ui(pid, window)
    try:
        raise SystemExit(run(ui, team, Path(sys.argv[2]) if len(sys.argv) > 2 else None))
    finally:
        ui.close()


if __name__ == "__main__":
    main()
