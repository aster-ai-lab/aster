"""Team admin P2 runs acceptance on the owned ToWork window, cua-driver background mode only.

Chain: ledger rows for the team (the record a3s-api writes when a member starts a Cloud agent
run) → 智能体 / 运行 lists them newest first with the starter, a former member and the Cloud
status → 加载更早的运行 follows the cursor to the end → 刷新 picks up a newer run → a member
without `read Run` and a whole-team scope gets not-found for the list, the logs and stop.

The local Cloud has no released agent for this team, so the fixture runs are ledger rows that
Cloud does not know: the page shows "Cloud 中已无此运行", the logs button is disabled and no stop
is offered. Owner-side stop of such a run passes the ledger and gets Cloud's not-found. A stop
that Cloud accepts is proved with the fake Cloud in the api unit tests and in vitest
(`OrgRuns.test.tsx`).

Fixture rows carry one fixed agent asset id and are removed when the run ends. They are written
with psql inside the local Postgres container; no connection string leaves this process.

Usage:
  A3S_E2E_SESSION_FROM=<member-session.json> A3S_E2E_TEAM=<team> python3 cua_p2_runs.py run [evidence-dir]
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cua_p0 import SECRET, T, Run, Ui, fail_closed, launch, wait_api  # noqa: E402
from cua_p1 import REVIEWER, call, creator_token  # noqa: E402

PAGE = 20
FIXTURE_RUNS = 24
FIXTURE_ASSET = "e2e00000-0000-4000-8000-0000000000f1"
CONTAINER = os.environ.get("A3S_E2E_POSTGRES_CONTAINER", "a3s-postgres")


def psql(sql: str) -> str:
    answer = subprocess.run(
        ["docker", "exec", "-i", CONTAINER, "psql", "-U", "a3s_api", "-d", "a3s_api", "-v", "ON_ERROR_STOP=1", "-At"],
        input=sql,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if answer.returncode != 0:
        raise AssertionError(f"psql failed: {SECRET.sub('<redacted>', answer.stderr.strip())[:300]}")
    return answer.stdout


def uid(value: str) -> str:
    """Only canonical UUIDs reach the SQL text."""
    return str(uuid.UUID(value))


def refused(path: str, method: str = "GET", body: Any = None, token: str | None = None) -> str:
    try:
        call(path, method, body, token)
    except RuntimeError as exc:
        if " -> 404 " in str(exc):
            return str(exc)
        raise
    raise AssertionError(f"{method} {path} answered instead of not-found")


class Runs(Run):
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        super().__init__(ui, team, evidence)
        self.owner_member = ""
        self.owner_name = ""
        self.reviewer: dict[str, Any] = {}
        self.fixture: list[str] = []
        self.conversations: dict[str, str] = {}
        self.newer = ""

    def api(self, path: str, method: str = "GET", body: Any = None) -> Any:
        return call(path, method, body, creator_token())

    def all_runs(self) -> list[dict[str, Any]]:
        runs: list[dict[str, Any]] = []
        query = f"limit={PAGE}"
        for _ in range(50):
            page = self.api(f"/v1/api/org/members/orgs/{self.org_id}/runs?{query}")
            runs += page["runs"]
            if page["next"] is None:
                return runs
            cursor = page["next"]
            query = f"limit={PAGE}&before_at={cursor['before_at']}&before_id={cursor['before_id']}"
        raise AssertionError("the runs cursor never ends")

    def reset_fixture(self) -> None:
        psql(f"delete from cloud_agent_run_owners where org_id = '{uid(self.org_id)}' and agent_asset_id = '{FIXTURE_ASSET}';")

    def ledger(self, execution: str, starter: str, minutes_ago: int) -> None:
        conversation = str(uuid.uuid4())
        self.conversations[execution] = conversation
        psql(
            "insert into cloud_agent_run_owners "
            "(execution_id, org_id, conversation_id, agent_asset_id, started_by_member, started_at) values "
            f"('{uid(execution)}', '{uid(self.org_id)}', '{conversation}', '{FIXTURE_ASSET}', '{uid(starter)}', "
            f"now() - interval '{int(minutes_ago)} minutes');"
        )

    # ── page helpers ────────────────────────────────────────────────

    def open_runs(self) -> None:
        """A dev-server reload lands on the work page; re-enter the team and try again."""
        ui = self.ui
        for attempt in range(3):
            ui.refresh()
            if ui.items(T("org.nav.runs"), "AXHeading"):
                return
            try:
                self.enter_team()
                ui.nav(T("org.nav.runs"))
                ui.seen(T("org.nav.runs"), "AXHeading", exact=True)
                return
            except AssertionError:
                if attempt == 2:
                    raise
                time.sleep(2)

    def fresh_runs(self) -> None:
        """Leaving the page drops the loaded pages; coming back reads page one again."""
        ui = self.ui
        ui.refresh()
        if ui.items(T("org.nav.runs"), "AXHeading"):
            ui.nav(T("org.nav.members"))
            ui.gone(T("org.nav.runs"), "AXHeading")
        self.open_runs()

    def loaded(self, count: int, timeout: float = 15) -> None:
        self.ui.seen(T("org.runs.loaded", count=count), None, exact=True, timeout=timeout)

    def row(self, execution: str, timeout: float = 12) -> dict[str, Any]:
        """A row is found by its conversation cell: the logs button of a run Cloud lost is
        disabled, and the driver does not list disabled controls."""
        return self.ui.seen(self.conversations[execution][:8], None, exact=True, timeout=timeout)

    def no_actions(self, execution: str) -> None:
        """Neither an enabled logs button nor a stop button for a run Cloud does not have."""
        ui = self.ui
        ui.refresh()
        logs = [e for e in ui.items(f"{T('org.runs.logs')} {execution[:8]}", "AXButton") if e.get("enabled") is not False]
        if logs:
            raise AssertionError("logs are offered for a run Cloud does not have")
        ui.absent(f"{T('org.runs.stop')} {execution[:8]}", "AXButton")

    # ── cases ───────────────────────────────────────────────────────

    def c01_team_center(self) -> None:
        orgs = self.api("/v1/api/org/members/orgs")
        self.org_id = next(org["id"] for org in orgs if org.get("name") == self.team)
        me = self.api("/v1/api/org/members/me")
        self.owner_member = str(me.get("id") or me.get("member", {}).get("id") or "")
        rows = self.api(f"/v1/api/org/members/orgs/{self.org_id}/members")
        rows = rows.get("items", rows) if isinstance(rows, dict) else rows
        self.owner_name = next(row["name"] for row in rows if row.get("member_id") == self.owner_member)
        if not REVIEWER.is_file():
            raise AssertionError("no second member; run cua_p1.py setup first")
        self.reviewer = json.loads(REVIEWER.read_text())
        if not any(row.get("member_id") == self.reviewer["member_id"] for row in rows):
            raise AssertionError("the reviewer is not in the team; run cua_p1.py setup first")
        self.enter_team()

    def c02_ledger(self) -> None:
        """Newest first: a former member's run, the reviewer's run, then the owner's."""
        self.reset_fixture()
        before = len(self.all_runs())
        former = str(uuid.uuid4())
        self.fixture = [str(uuid.uuid4()) for _ in range(FIXTURE_RUNS)]
        for index, execution in enumerate(self.fixture):
            starter = former if index == 0 else self.reviewer["member_id"] if index == 1 else self.owner_member
            self.ledger(execution, starter, index + 1)
        runs = self.all_runs()
        if len(runs) != before + FIXTURE_RUNS:
            raise AssertionError(f"owner reads {len(runs)} runs, expected {before + FIXTURE_RUNS}")
        head = [run["execution_id"] for run in runs[:3]]
        if head != self.fixture[:3]:
            raise AssertionError("the owner's list is not newest first")
        mine = {run["execution_id"]: run for run in runs if run["execution_id"] in set(self.fixture)}
        if mine[self.fixture[0]]["started_by"]["name"] is not None:
            raise AssertionError("a former member still carries a name")
        if mine[self.fixture[2]]["started_by"]["name"] != self.owner_name:
            raise AssertionError("the owner's run lost the owner's name")
        if any(run["status"] is not None for run in mine.values()):
            raise AssertionError("Cloud reports a status for a run it never started")
        print(f"  ledger: {before} runs before, {FIXTURE_RUNS} fixture rows added")

    def c03_list(self) -> None:
        ui = self.ui
        self.fresh_runs()
        ui.seen(T("org.nav.group.agents"), None, exact=True)
        total = len(self.all_runs())
        if self.loaded_now() < min(PAGE, total):
            raise AssertionError("the first page is short")
        ui.seen(T("org.runs.status.missing"), None, exact=True)
        ui.seen(T("org.runs.formerMember"), None, exact=True)
        ui.seen(self.owner_name, None, exact=True)
        self.row(self.fixture[0])
        self.no_actions(self.fixture[0])
        self.record("c03_list")
        self.grab("c03_list")

    def c04_load_more(self) -> None:
        ui = self.ui
        self.fresh_runs()
        total = len(self.all_runs())
        shown = self.loaded_now()
        while shown < total:
            ui.press(T("org.runs.more"), "AXButton", exact=True)
            shown = min(shown + PAGE, total)
            self.loaded(shown, timeout=20)
        ui.seen(T("org.runs.end"), None, exact=True)
        ui.absent(T("org.runs.more"), "AXButton")
        self.row(self.fixture[-1])
        self.record("c04_load_more")
        self.grab("c04_load_more")
        print(f"  loaded {shown} of {total}")

    def loaded_now(self) -> int:
        """The count line of the open page. The page keeps every page it loaded while the team
        center stays open, so it is 20 on a first visit and the full list after 加载更多."""
        ui = self.ui

        def read() -> list[int]:
            return [n for n in range(1, FIXTURE_RUNS + 8) if ui.items(T("org.runs.loaded", count=n), None, exact=True)]

        return ui.wait(read, "the loaded count", timeout=15)[0]

    def c05_refresh(self) -> None:
        """A run started while the page is open shows after 刷新, on top, without dropping
        the pages already loaded."""
        ui = self.ui
        self.open_runs()
        before = self.loaded_now()
        inserted = not self.newer
        if inserted:
            self.newer = str(uuid.uuid4())
            self.fixture.append(self.newer)
            self.ledger(self.newer, self.owner_member, 0)
        runs = self.all_runs()
        if runs[0]["execution_id"] != self.newer:
            raise AssertionError("the owner's list does not start with the newer run")
        if inserted:
            ui.absent(self.conversations[self.newer][:8], None)
        ui.press(T("common.refresh"), "AXButton", exact=True)
        self.row(self.newer)
        self.loaded(len(runs) if before >= len(runs) - 1 else before)
        self.record("c05_refresh")
        self.grab("c05_refresh")

    def c06_scope(self) -> None:
        """Ledger first: the reviewer has neither read nor stop Run, and the owner's stop of a run
        Cloud lost passes the ledger and ends at Cloud's not-found."""
        token = self.reviewer["token"]
        base = f"/v1/api/org/members/orgs/{self.org_id}/runs"
        owner_run = self.fixture[2]
        refused(base, token=token)
        refused(f"{base}/{owner_run}/logs", token=token)
        refused(f"{base}/{owner_run}/stop", "POST", {"request_id": str(uuid.uuid4())}, token)
        mine = call("/v1/api/org/members/orgs", token=token)
        team = next(org for org in mine if org.get("id") == self.org_id)
        if any(rule.get("subject") == "Run" for rule in team.get("rules") or []):
            raise AssertionError("the reviewer role carries a Run rule")
        # The owner passes the ledger, so the stop reaches Cloud. Cloud answers not-found for a
        # run it never had, unless this deployment's Cloud token lacks execution:write; then
        # Cloud refuses before the lookup and a3s-api reports it as an internal error.
        try:
            call(f"{base}/{owner_run}/stop", "POST", {"request_id": str(uuid.uuid4())}, creator_token())
        except RuntimeError as exc:
            lost = str(exc)
        else:
            raise AssertionError("Cloud stopped a run it never started")
        if " -> 404 " in lost and "agent run not found" in lost:
            print("  owner stop of a lost run: Cloud not-found")
        elif " -> 500 " in lost and "missing required scope: execution:write" in lost:
            print("  owner stop of a lost run: reached Cloud; this deployment's Cloud token lacks execution:write")
        else:
            raise AssertionError(f"owner stop of a lost run reads {lost[:160]}")
        refused(f"{base}/{uuid.uuid4()}/stop", "POST", {"request_id": str(uuid.uuid4())}, creator_token())
        try:
            self.api(f"{base}/{owner_run}/stop", "POST", {"request_id": str(uuid.UUID(int=0))})
        except RuntimeError as exc:
            if " -> 400 " not in str(exc):
                raise
        else:
            raise AssertionError("a nil stop request id was accepted")
        self.record("c06_scope")


CASES = ["c01_team_center", "c02_ledger", "c03_list", "c04_load_more", "c05_refresh", "c06_scope"]


def run(ui: Ui, team: str, evidence: Path | None) -> int:
    session = Runs(ui, team, evidence)
    lines: list[str] = []
    failed = 0
    try:
        for case in CASES:
            started = time.time()
            line = ""
            # The shared dev server reloads the page whenever any session edits the renderer, so a
            # case gets one retry. c02 resets the fixture first and never stacks rows.
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
            session.reset_fixture()
            print("fixture rows removed")
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
