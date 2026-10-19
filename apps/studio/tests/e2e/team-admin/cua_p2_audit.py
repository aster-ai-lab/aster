"""Team admin P2 audit-log acceptance on the owned ToWork window, cua-driver background mode only.

Chain: the owner makes fresh team facts (a repository, a protection edit, a phone invite
and its revocation) → 审计日志 lists them newest first with names and a masked phone →
the kind, actor and window filters each ask the owner again → 加载更早的记录 follows the
cursor to the end → a member without `read Audit` and a whole-team scope gets not-found.

Facts are made through the member-plane API with the copied owner session, as a second
browser tab would. Every number the page shows is compared with the owner's own answer
for the same filter. The export button is checked but not pressed: the shell's folder
panel activates the app, and this run never takes focus. The CSV body and the save
bridge are proved in vitest (`OrgAudit.test.tsx`).

Secrets: the session, the reviewer token and invite codes stay in memory or 0600 files.
Logs and evidence are redacted with the P0 pattern.

Usage:
  A3S_E2E_SESSION_FROM=<member-session.json> A3S_E2E_TEAM=<team> python3 cua_p2_audit.py run [evidence-dir]
"""

from __future__ import annotations

import json
import secrets
import sys
import time
import urllib.parse
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cua_p0 import SECRET, T, Run, Ui, fail_closed, launch, norm, wait_api  # noqa: E402
from cua_p1 import REVIEWER, call, creator_token  # noqa: E402

PAGE = 50


class AuditUi(Ui):
    """Three loaded pages hold ~1,500 table cells; the driver's default walk stops at 2,000 nodes,
    before the footer with 加载更早的记录 and 没有更早的记录."""

    def _window_state(self) -> dict[str, Any]:
        return self.call(
            "get_window_state",
            {"pid": self.pid, "window_id": self.window_id, "include_screenshot": False, "max_elements": 8000},
        )


def masked(phone: str) -> str:
    return f"{phone[:3]}****{phone[7:]}"


class Audit(Run):
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        super().__init__(ui, team, evidence)
        stamp = time.strftime("%H%M%S")
        self.repo = f"cua-audit-{stamp}"
        self.phone = "139" + f"{secrets.randbelow(10**8):08d}"
        self.namespace = ""
        self.owner_member = ""
        self.owner_name = ""

    def api(self, path: str, method: str = "GET", body: Any = None) -> Any:
        return call(path, method, body, creator_token())

    def feed(self, days: int, kind: str | None = None, actor: str | None = None, pages: int = 1) -> tuple[list[dict[str, Any]], bool]:
        """The owner's answer for one filter: entries over `pages` pages, and whether more remain."""
        entries: list[dict[str, Any]] = []
        cursor: dict[str, str] | None = None
        for _ in range(pages):
            query = {"days": days, "limit": PAGE}
            if kind:
                query["kind"] = kind
            if actor:
                query["actor"] = actor
            if cursor:
                query.update(cursor)
            page = self.api(f"/v1/api/org/members/orgs/{self.org_id}/audit?{urllib.parse.urlencode(query)}")
            entries += page["entries"]
            cursor = page["next"]
            if cursor is None:
                return entries, False
        return entries, True

    # ── page helpers ────────────────────────────────────────────────

    def open_audit(self) -> None:
        ui = self.ui
        ui.refresh()
        if not ui.items(T("org.nav.audit"), "AXHeading"):
            ui.nav(T("org.nav.audit"))
        ui.seen(T("org.nav.audit"), "AXHeading", exact=True)

    def fresh_audit(self) -> None:
        """Filters live as long as the page; leaving the page drops them back to the defaults."""
        ui = self.ui
        ui.refresh()
        if ui.items(T("org.nav.audit"), "AXHeading"):
            ui.nav(T("org.nav.members"))
            ui.gone(T("org.nav.audit"), "AXHeading")
        self.open_audit()

    def loaded(self, count: int, timeout: float = 15) -> None:
        self.ui.seen(T("org.audit.loaded", count=count), None, exact=True, timeout=timeout)

    def trigger(self, label: str) -> dict[str, Any]:
        rows = [e for e in self.ui.items(label) if e.get("role") in ("AXPopUpButton", "AXButton", "AXComboBox")]
        if not rows:
            raise AssertionError(f"no filter {label}")
        return rows[0]

    def pick(self, label: str, option: str) -> None:
        """Open one SearchSelect and press an option inside its open list (never a table cell or the sidebar)."""
        ui = self.ui
        ui.trace.append(f"pick {label} -> {option}")

        def inside(element: dict[str, Any], box: dict[str, Any]) -> bool:
            frame = element.get("frame") or {}
            x, y = float(frame.get("x", -1)), float(frame.get("y", -1))
            return box["x"] <= x < box["x"] + box["w"] and box["y"] <= y < box["y"] + box["h"]

        def rows() -> list[dict[str, Any]]:
            lists = [e.get("frame") for e in ui.elements if e.get("role") == "AXList" and e.get("frame")]
            return [e for e in ui.items(option) if e.get("role") != "AXHeading" and any(inside(e, box) for box in lists)][-1:]

        ui.refresh()
        if not rows():
            ui.hit(lambda: [self.trigger(label)], f"filter {label}")
        ui.hit(rows, f"option {option}")
        time.sleep(0.6)

    def clear_filters(self) -> None:
        """清除筛选 shows while a kind or actor is chosen. The select's own clear mark is nested in
        its trigger and has no accessibility node of its own."""
        ui = self.ui
        ui.refresh()
        if ui.items(T("org.audit.clearFilters"), "AXButton"):
            ui.press(T("org.audit.clearFilters"), "AXButton", exact=True)
            ui.gone(T("org.audit.clearFilters"), "AXButton")

    def shows(self, text: str) -> bool:
        self.ui.refresh()
        return bool(self.ui.items(text, None, exact=False))

    # ── cases ───────────────────────────────────────────────────────

    def c01_team_center(self) -> None:
        orgs = self.api("/v1/api/org/members/orgs")
        self.org_id = next(org["id"] for org in orgs if org.get("name") == self.team)
        self.namespace = self.api(f"/v1/api/org/repository-namespaces/{self.org_id}")["namespace"] or ""
        if not self.namespace:
            raise AssertionError("the team has no namespace; run cua_p1.py setup first")
        me = self.api("/v1/api/org/members/me")
        self.owner_member = str(me.get("id") or me.get("member", {}).get("id") or "")
        rows = self.api(f"/v1/api/org/members/orgs/{self.org_id}/members")
        rows = rows.get("items", rows) if isinstance(rows, dict) else rows
        self.owner_name = next(row["name"] for row in rows if row.get("member_id") == self.owner_member)
        self.enter_team()

    def invite_and_revoke(self, phone: str) -> None:
        invite = self.api("/v1/api/org/members/invites", "POST", {"org_id": self.org_id, "phone": phone})
        self.api(f"/v1/api/org/members/invites/{invite['id']}", "DELETE", {"expected_revision": invite["revision"]})

    def c02_facts(self) -> None:
        """Enough history for a second page, then this run's own facts, newest last."""
        entries, _ = self.feed(90, pages=2)
        before = len(entries)
        topped = 0
        while len(entries) < PAGE + 6:
            self.invite_and_revoke("139" + f"{secrets.randbelow(10**8):08d}")
            topped += 1
            entries, _ = self.feed(90, pages=2)
            if topped > 40:
                raise AssertionError("the feed does not grow with invites")
        print(f"  history: {before} entries in two pages, {topped} invite pairs added")
        try:
            self.api(f"/v1/api/org/repositories/{self.namespace}/{self.repo}")
        except RuntimeError as exc:
            if " -> 404 " not in str(exc):
                raise
            self.api(
                "/v1/api/org/repositories",
                "POST",
                {"namespace": self.namespace, "name": self.repo, "visibility": "private", "seed_readme": True},
            )
        rules = self.api(f"/v1/api/org/repositories/{self.namespace}/{self.repo}/protections")
        for rule in rules:
            if rule["pattern"] == "refs/heads/main":
                rule["require_approvals"] = 1
        self.api(f"/v1/api/org/repositories/{self.namespace}/{self.repo}/protections", "PUT", {"protections": rules})
        self.invite_and_revoke(self.phone)
        newest = [row["action"] for row in self.feed(30)[0][:4]]
        expected = ["invite.revoked", "invite.created", "repository.protections.updated", "repository.created"]
        if newest != expected:
            raise AssertionError(f"owner feed starts {newest}")

    def c03_feed(self) -> None:
        """Default window 30 days: the owner's first page, newest first, with names and a masked phone."""
        ui = self.ui
        self.fresh_audit()
        first, _ = self.feed(30)
        self.loaded(len(first))
        ui.seen(masked(self.phone), None, exact=False)
        if self.phone in " ".join(ui.snapshot()):
            raise AssertionError("the full invite phone is on screen")
        ui.seen(f"{self.namespace}/{self.repo}", None, exact=False)
        for key in ("invite_revoked", "invite_created", "repository_protections_updated", "repository_created"):
            ui.seen(T(f"org.audit.action.{key}"), None, exact=True)
        ui.seen(self.owner_name, None, exact=True)
        rows = [e for e in ui.items(None, "AXRow")]
        if rows:
            head = " ".join(norm(e.get("label")) for e in rows[1:2])
            if head and T("org.audit.action.invite_revoked") not in head and masked(self.phone) not in head:
                raise AssertionError(f"newest row reads {head[:120]!r}")
        export = ui.seen(T("org.audit.export"), "AXButton", exact=False)
        if export.get("enabled") is False:
            raise AssertionError("export is disabled with entries on screen")
        self.record("c03_feed")
        self.grab("c03_feed")

    def c04_filter_kind(self) -> None:
        ui = self.ui
        self.fresh_audit()
        self.pick(T("org.audit.filter.kind"), T("org.audit.kind.repository"))
        repo, _ = self.feed(30, kind="repository")
        self.loaded(len(repo))
        ui.seen(f"{self.namespace}/{self.repo}", None, exact=False)
        ui.wait(lambda: not self.shows(masked(self.phone)), "the invite rows to leave")
        self.record("c04_kind_repository")
        self.grab("c04_kind_repository")
        self.pick(T("org.audit.filter.kind"), T("org.audit.kind.invitation"))
        invites, _ = self.feed(30, kind="invitation")
        self.loaded(len(invites))
        ui.seen(masked(self.phone), None, exact=False)
        ui.wait(lambda: not self.shows(f"{self.namespace}/{self.repo}"), "the repository rows to leave")
        ui.seen(T("org.audit.clearFilters"), "AXButton", exact=True)
        self.clear_filters()
        everything, _ = self.feed(30)
        self.loaded(len(everything))
        # Both kinds again: the count alone cannot tell a full page of one kind from all kinds.
        ui.seen(f"{self.namespace}/{self.repo}", None, exact=False)
        ui.seen(masked(self.phone), None, exact=False)

    def c05_filter_actor_days(self) -> None:
        ui = self.ui
        self.fresh_audit()
        self.pick(T("org.audit.filter.actor"), self.owner_name)
        mine, _ = self.feed(30, actor=self.owner_member)
        self.loaded(len(mine))
        if any(row["actor"]["member_id"] != self.owner_member for row in mine):
            raise AssertionError("the owner's actor filter returned someone else")
        self.pick(T("org.audit.filter.days"), T("org.audit.window", count=7))
        week, _ = self.feed(7, actor=self.owner_member)
        self.loaded(len(week))
        self.record("c05_actor_week")
        self.clear_filters()
        # The trigger names only its label to accessibility; the kept window is proved in vitest.
        week_all, _ = self.feed(7)
        self.loaded(len(week_all))

    def c06_load_more(self) -> None:
        """90 days, every kind: page one, then the cursor until the end marker."""
        ui = self.ui
        self.fresh_audit()
        self.pick(T("org.audit.filter.days"), T("org.audit.window", count=90))
        total, _ = self.feed(90, pages=20)
        self.loaded(PAGE)
        shown = PAGE
        while shown < len(total):
            ui.press(T("org.audit.more"), "AXButton", exact=True)
            shown = min(shown + PAGE, len(total))
            self.loaded(shown, timeout=20)
        ui.seen(T("org.audit.end"), None, exact=True)
        ui.absent(T("org.audit.more"), "AXButton")
        ui.refresh()
        kinds = {row["kind"] for row in total}
        if "repository" in kinds:
            ui.seen(f"{self.namespace}/{self.repo}", None, exact=False)
        if "role" in kinds:
            ui.seen(T("org.audit.action.role_created"), None, exact=True)
        # One header row, when WebKit exposes the table as rows at all.
        on_screen = len(ui.items(None, "AXRow"))
        if on_screen and on_screen not in (len(total), len(total) + 1):
            raise AssertionError(f"{on_screen} table rows on screen, owner has {len(total)} entries")
        self.record("c06_load_more")
        self.grab("c06_load_more")
        print(f"  loaded {shown} of {len(total)} in 90 days")

    def c07_scope(self) -> None:
        """A member whose role cannot read Audit gets the not-found answer, not an empty feed."""
        if not REVIEWER.is_file():
            raise AssertionError("no second member; run cua_p1.py setup first")
        reviewer = json.loads(REVIEWER.read_text())
        try:
            call(f"/v1/api/org/members/orgs/{self.org_id}/audit?days=30", token=reviewer["token"])
        except RuntimeError as exc:
            if " -> 404 " not in str(exc):
                raise
        else:
            raise AssertionError("a member without read Audit reads the team audit")
        mine = call("/v1/api/org/members/orgs", token=reviewer["token"])
        team = next(org for org in mine if org.get("id") == self.org_id)
        if any(rule.get("subject") == "Audit" for rule in team.get("rules") or []):
            raise AssertionError("the reviewer role carries an Audit rule")
        self.record("c07_scope")


CASES = ["c01_team_center", "c02_facts", "c03_feed", "c04_filter_kind", "c05_filter_actor_days", "c06_load_more", "c07_scope"]


def run(ui: Ui, team: str, evidence: Path | None) -> int:
    session = Audit(ui, team, evidence)
    lines: list[str] = []
    failed = 0
    for case in CASES:
        started = time.time()
        line = ""
        # The shared dev server reloads the page whenever any session edits the renderer, so a case
        # gets one retry. c02 reads the owner first and never makes the same fact twice.
        for attempt in (1, 2):
            ui.trace.clear()
            try:
                wait_api()
                ui.close_siblings()
                ui.expose()
                if case != "c01_team_center":
                    session.enter_team()
                getattr(session, case)()
            except Exception as exc:
                reason = SECRET.sub("<redacted>", str(exc))[:300]
                steps = " | ".join(ui.trace[-6:])
                print(f"  {case} attempt {attempt}: {reason} after [{steps}]", file=sys.stderr)
                if evidence is not None:
                    session.record(f"{case}_failure_{attempt}")
                line = f"FAIL {case} {reason}"
                if case == "c02_facts":
                    break
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
    ui = AuditUi(pid, window)
    try:
        raise SystemExit(run(ui, team, Path(sys.argv[2]) if len(sys.argv) > 2 else None))
    finally:
        ui.close()


if __name__ == "__main__":
    main()
