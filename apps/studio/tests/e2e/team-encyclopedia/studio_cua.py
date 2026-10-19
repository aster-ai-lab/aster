"""Team encyclopedia retrieval in Studio (E12-E14), on one owned window in background mode.

The window gets its own instance (`towork-cua-enc-e2e`) signed in as the department member
of the org that `api_e2e.py` built, so no other session's window is touched. Every action is
an accessibility action with `delivery_mode: background` (the pointer never moves and the
window is not raised), driven through the `cua_p0` launcher and `Ui` driver.

  E12  the member searches the mounted team encyclopedia in Chinese and opens the source file
  E13  an agent answer cites a team passage; the citation is a link to the drive file
  E14  a finance-only question returns the empty state to the department member

Usage:
  python3 studio_cua.py <team name> [run|dump|press <label> [role]] [evidence-dir]
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "team-admin"))
sys.path.insert(0, str(HERE.parent / "team-workbench"))
import cua_p0  # noqa: E402
from cua_p0 import SECRET, T, Ui, dump, norm, wait_api  # noqa: E402
from two_account_gui import SUPPORT as APPS  # noqa: E402

MEMBER_APP = "toyou-fork2"
MEMBER_NAME = "Leon"
cua_p0.INSTANCE = "enc-e2e"
cua_p0.STORAGE = f"{cua_p0.TENANT}-{cua_p0.INSTANCE}"
cua_p0.SUPPORT = cua_p0.SUPPORT.parent / cua_p0.STORAGE
cua_p0.SESSION = cua_p0.SUPPORT / "member-session.json"
cua_p0.EXECUTABLE = cua_p0.SUPPORT / "ToWork.app/Contents/MacOS/ToWork"

SEARCH = T("knowledge.encyclopedia.search_passages")
OPEN = T("knowledge.encyclopedia.open_source")
EMPTY = T("knowledge.encyclopedia.passages_empty")
E12_QUERY = "出差回来多久要把票据交上去"
E12_HEADING = "差旅票据提交时限"
E12_FILE = "差旅报销制度"
NOT_FOUND = "This page could not be found"
E13_PROMPT = "请只用团队百科检索回答：出差回来多久要交票据？回答时引用来源。"
E14_QUERY = "公司基本户开在哪家银行"
E14_HIDDEN = ("基本户开户行", "年度调薪预算", "星河项目回款计划", "招商银行")


class Run:
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        self.ui = ui
        self.team = team
        self.evidence = evidence

    def record(self, case: str) -> None:
        if self.evidence is not None:
            self.evidence.mkdir(parents=True, exist_ok=True)
            (self.evidence / f"{case}.ax.txt").write_text("\n".join(self.ui.snapshot()) + "\n")

    def enter_team(self) -> None:
        """Switch the window to the org through the account menu; the page may have reloaded."""
        ui = self.ui
        ui.refresh()
        teams = T("sidebar.center.teams")
        if ui.items(self.team, "AXHeading"):
            return
        if ui.items(teams, "AXButton") and not ui.items(self.team, "AXMenuItem", exact=False):
            ui.press(teams, "AXButton")
            ui.refresh()
            if ui.items(self.team, "AXHeading"):
                return
        if not ui.items(self.team, "AXMenuItem", exact=False):
            if not ui.items(teams, "AXRadioButton"):
                ui.press(MEMBER_NAME, "AXPopUpButton")
            ui.press(teams, "AXRadioButton")
        ui.press(self.team, "AXMenuItem", exact=False)
        ui.press(teams, "AXButton")
        ui.seen(self.team, "AXHeading", exact=True)

    def search(self, query: str) -> None:
        ui = self.ui
        ui.refresh()
        # Re-entering the drive remounts the search, so the field starts empty.
        ui.nav(T("org.nav.knowledge"))
        ui.seen(SEARCH, "AXTextField")
        ui.type_into(SEARCH, query)
        ui.hit(lambda: ui.items(SEARCH, "AXTextField")[:1], "search field", "press_key", {"key": "return", "delivery_mode": "background"})

    def e12_search_and_open(self) -> None:
        ui = self.ui
        self.search(E12_QUERY)
        ui.seen(E12_HEADING, timeout=30)
        self.record("e12_hits")
        ui.press(OPEN, "AXLink", exact=False)
        self.opened_drive_file()
        self.record("e12_opened")

    def opened_drive_file(self) -> None:
        """The files tab shows the org drive with the cited file; a page load would land on not-found."""
        ui = self.ui
        ui.seen(T("drive.org_space"), "AXButton", exact=True, timeout=20)
        ui.seen(E12_FILE, "AXButton", exact=True)
        ui.absent(NOT_FOUND, exact=False)

    def e14_member_sees_nothing_hidden(self) -> None:
        ui = self.ui
        self.search(E14_QUERY)
        # Vector neighbours from visible files may answer; the finance passages never may.
        ui.wait(lambda: ui.items(EMPTY, exact=False) or ui.items(OPEN, "AXLink", exact=False), "results", 30)
        for hidden in E14_HIDDEN:
            ui.absent(hidden, exact=False)
        self.record("e14_no_hidden")

    def e13_agent_citation(self) -> None:
        """A task started in the team center searches the team encyclopedia; the hit cites the passage."""
        ui = self.ui
        composer = T("workbench.composer.placeholder")
        ui.press(T("title.work"), "AXButton")
        ui.press("日常办公", "AXRadioButton")
        ui.close_siblings()
        # Once typed, the text area is labelled by its value, so it is found by role.
        area = lambda: ui.items(role="AXTextArea")[:1]  # noqa: E731
        # The composer re-renders on its first character and can drop the rest: clear and retype.
        for attempt in range(3):
            if norm(ui.wait(area, "composer")[0].get("value")) not in ("", composer):
                ui.hit(area, "clear composer", "set_value", {"value": ""})
                time.sleep(0.4)
            ui.hit(area, "composer", "type_text", {"text": E13_PROMPT, "delivery_mode": "background", "delay_ms": 20})
            try:
                ui.wait(lambda: [e for e in area() if norm(e.get("value")) == E13_PROMPT], "typed prompt", timeout=10)
                break
            except Exception:
                if attempt == 2:
                    raise
        ui.hit(area, "composer", "press_key", {"key": "return", "delivery_mode": "background"})
        ui.seen(E12_HEADING, "AXDisclosureTriangle", timeout=240)
        # The tool card remounts collapsed when the answer settles; the message toolbar marks that.
        ui.seen(T("chat.save.wiki.title"), "AXButton", timeout=240)
        # A press that lands during a late re-render is lost: press again.
        for attempt in range(4):
            ui.press(E12_HEADING, "AXDisclosureTriangle", exact=False)
            try:
                ui.seen(OPEN, "AXLink", exact=False, timeout=15)
                break
            except Exception:
                if attempt == 3:
                    raise
        self.record("e13_cited")
        ui.press(OPEN, "AXLink", exact=False)
        self.opened_drive_file()
        self.record("e13_opened")


CASES = ["e12_search_and_open", "e14_member_sees_nothing_hidden", "e13_agent_citation"]


def run(ui: Ui, team: str, evidence: Path | None) -> int:
    session = Run(ui, team, evidence)
    failed = 0
    lines = []
    for case in CASES:
        started = time.time()
        ui.trace.clear()
        try:
            wait_api()
            ui.close_siblings()
            ui.expose()
            session.enter_team()
            getattr(session, case)()
        except Exception as exc:
            reason = SECRET.sub("<redacted>", str(exc))[:300]
            print(f"  {case}: {reason} after [{' | '.join(ui.trace[-6:])}]", file=sys.stderr)
            session.record(f"{case}_failure")
            line = f"FAIL {case} {reason}"
            failed += 1
        else:
            line = f"PASS {case} {time.time() - started:.1f}s"
        print(line)
        lines.append(line)
        sys.stdout.flush()
    routes = sorted({f"{name}:{route or '-'}:{mode or '-'}" for name, route, mode in ui.routes})
    print("routes " + ", ".join(routes))
    print(f"done passed={len(CASES) - failed} failed={failed}")
    if evidence is not None:
        (evidence / "console.txt").write_text("\n".join(lines + ["routes " + ", ".join(routes)]) + "\n")
    return 1 if failed else 0


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    team = sys.argv[1]
    command = sys.argv[2] if len(sys.argv) > 2 else "run"
    pid, window = cua_p0.launch(APPS / MEMBER_APP / "member-session.json")
    print(f"owned pid={pid} window={window} instance={cua_p0.STORAGE}")
    ui = Ui(pid, window)
    try:
        if command == "dump":
            dump(ui)
            return
        if command == "press":
            ui.refresh()
            ui.press(sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None, exact=False)
            time.sleep(0.8)
            dump(ui)
            return
        if command == "team":
            Run(ui, team, None).enter_team()
            dump(ui)
            return
        raise SystemExit(run(ui, team, Path(sys.argv[3]) if len(sys.argv) > 3 else None))
    finally:
        ui.close()


if __name__ == "__main__":
    main()
