"""Team admin P0 cases on one owned ToWork window, driven by cua-driver in background mode.

The window gets its own instance (`towork-cua-admin-e2e`), so the ToWork suite's
`fork1` window, the team-workbench pair (`towork-cua-teamadmin`) and the primary
window are never touched. Every press is an
accessibility action with `delivery_mode: background`: the system pointer does
not move and the window is not brought to the front. The launch itself shows
the window once; the previously frontmost app is activated again right away.

The run signs in by copying a member session file named in the environment. It
never prints the file, the bearer token, or an issued access token.

Usage:
  A3S_E2E_SESSION_FROM=<member-session.json> A3S_E2E_TEAM=<team name> \\
    python3 cua_p0.py [run|dump] [evidence-dir]
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "towork"))
from driver import Drive  # noqa: E402

STUDIO = Path(__file__).resolve().parents[3]
TENANT = "towork-cua"
INSTANCE = "admin-e2e"
STORAGE = f"{TENANT}-{INSTANCE}"
SUPPORT = Path.home() / "Library/Application Support/dev.a3s.studio" / STORAGE
SESSION = SUPPORT / "member-session.json"
EXECUTABLE = SUPPORT / "ToWork.app/Contents/MacOS/ToWork"
LAUNCHER = STUDIO / "src-tauri/target/debug/Studio"


def fail_closed() -> tuple[Path, str]:
    source = os.environ.get("A3S_E2E_SESSION_FROM", "").strip()
    team = os.environ.get("A3S_E2E_TEAM", "").strip()
    missing = [name for name, value in (("A3S_E2E_SESSION_FROM", source), ("A3S_E2E_TEAM", team)) if not value]
    if missing:
        print("FAIL closed: missing " + ",".join(missing))
        raise SystemExit(2)
    path = Path(source).expanduser()
    if not path.is_file():
        print("FAIL closed: A3S_E2E_SESSION_FROM is not a file")
        raise SystemExit(2)
    return path, team


def seed_session(source: Path) -> None:
    """Copy the session byte for byte with mode 0600. The contents stay unread."""
    SUPPORT.mkdir(parents=True, exist_ok=True)
    temporary = SESSION.with_name(SESSION.name + ".tmp")
    temporary.write_bytes(source.read_bytes())
    os.chmod(temporary, 0o600)
    temporary.replace(SESSION)


def frontmost_bundle() -> str:
    script = 'tell application "System Events" to get bundle identifier of first process whose frontmost is true'
    done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def give_back_focus(bundle: str) -> None:
    if bundle and not bundle.startswith("dev.a3s.studio."):
        subprocess.run(["osascript", "-e", f'tell application id "{bundle}" to activate'], capture_output=True)


def owned_pid() -> int | None:
    done = subprocess.run(["pgrep", "-f", str(EXECUTABLE)], capture_output=True, text=True)
    pids = [int(line) for line in done.stdout.split() if line.isdigit()]
    return pids[0] if pids else None


def window_for(drive_call: Callable[[str, dict[str, Any]], dict[str, Any]], pid: int) -> int | None:
    result = drive_call("list_windows", {"pid": pid})
    rows = (result.get("structuredContent") or {}).get("windows") or []
    rows = [row for row in rows if row.get("title") == "ToWork"]
    if not rows:
        return None
    best = max(rows, key=lambda row: (row.get("bounds") or {}).get("width", 0) * (row.get("bounds") or {}).get("height", 0))
    return int(best["window_id"])


class Probe(Drive):
    """A driver bound to no window yet, for list_windows during launch."""

    def __init__(self) -> None:
        super().__init__(0, 0)


def launch(source: Path) -> tuple[int, int]:
    probe = Probe()
    try:
        pid = owned_pid()
        if pid is None:
            if not LAUNCHER.is_file():
                raise RuntimeError("no Studio shell build; run cargo build in src-tauri first")
            seed_session(source)
            previous = frontmost_bundle()
            env = os.environ.copy()
            env.pop("A3S_STUDIO_FORK_ALLOC", None)
            env.update(
                {
                    "A3S_TENANT_SLUG": TENANT,
                    "A3S_STUDIO_INSTANCE": INSTANCE,
                    "STUDIO_RUNTIME_FILE": f"/tmp/a3s-studio-runtime-{STORAGE}.json",
                }
            )
            subprocess.Popen(
                [str(LAUNCHER)],
                cwd=STUDIO,
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            deadline = time.time() + 60
            while time.time() < deadline and pid is None:
                time.sleep(0.5)
                pid = owned_pid()
            if pid is None:
                raise RuntimeError("owned window did not start")
            window = None
            while time.time() < deadline and window is None:
                window = window_for(probe.call, pid)
                if window is None:
                    time.sleep(0.5)
            give_back_focus(previous)
            if window is None:
                raise RuntimeError("owned window did not appear")
            return pid, window
        window = window_for(probe.call, pid)
        if window is None:
            raise RuntimeError("owned process has no window")
        return pid, window
    finally:
        probe.close()


def dump(ui: Drive) -> None:
    ui.refresh()
    for element in ui.elements:
        if element.get("role") in ("AXMenuItem", "AXMenuBarItem", "AXMenu"):
            continue
        label = " ".join(str(element.get("label") or element.get("value") or "").split())
        if label:
            frame = element.get("frame") or {}
            print(f"{element.get('role')}\t{label[:80]}\t{int(frame.get('x', 0))},{int(frame.get('y', 0))}")


def main() -> None:
    source, team = fail_closed()
    command = sys.argv[1] if len(sys.argv) > 1 else "run"
    pid, window = launch(source)
    print(f"owned pid={pid} window={window} instance={STORAGE}")
    ui = Ui(pid, window)
    try:
        if command == "dump":
            dump(ui)
            return
        if command == "press":
            ui.refresh()
            ui.press(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None, exact=False)
            time.sleep(0.8)
            dump(ui)
            return
        raise SystemExit(run(ui, team, Path(sys.argv[2]) if len(sys.argv) > 2 else None))
    finally:
        ui.close()


LOCALE = json.loads((STUDIO / "src/renderer/i18n/locales/zh-cn.json").read_text())
SECRET = re.compile(r"a3s_[a-z]+_[0-9A-Za-z]+|[0-9a-f]{32,}")
API = "http://127.0.0.1:3000/v1/api/org/members"


def T(key: str, **values: object) -> str:
    text = LOCALE[key]
    for name, value in values.items():
        text = text.replace("{{" + name + "}}", str(value))
    return text


def norm(value: object) -> str:
    return " ".join(str(value or "").split())


class Ui(Drive):
    """Accessibility-only driver. Every action records the route the driver reports."""

    def __init__(self, pid: int, window_id: int) -> None:
        super().__init__(pid, window_id)
        self.routes: list[tuple[str, str, str]] = []
        self.trace: list[str] = []

    def act(self, name: str, arguments: dict[str, Any]) -> None:
        try:
            result = self.call(name, {"pid": self.pid, "window_id": self.window_id, **arguments})
        except RuntimeError as exc:
            # -25202: the pressed element left the tree while the press ran (a menu row
            # closes its menu). The caller proves the effect with the next wait.
            if name != "click" or "-25202" not in str(exc):
                raise
            self.routes.append((name, "accessibility", "background"))
            return
        structured = result.get("structuredContent") or {}
        delivery = structured.get("delivery") or {}
        self.routes.append((name, str(structured.get("route") or ""), str(delivery.get("mode") or arguments.get("delivery_mode") or "")))

    def items(self, label: str | None = None, role: str | None = None, exact: bool = True) -> list[dict[str, Any]]:
        found = []
        for element in self.elements:
            if role and element.get("role") != role:
                continue
            if element.get("role") in ("AXMenuItem", "AXMenuBarItem") and not (element.get("frame") or {}).get("x"):
                continue
            text = norm(element.get("label") or element.get("value"))
            if label is not None and (text != label if exact else label not in text):
                continue
            found.append(element)
        return found

    def wait(self, probe: Callable[[], Any], what: str, timeout: float = 12) -> Any:
        deadline = time.time() + timeout
        while True:
            self.refresh()
            value = probe()
            if value:
                return value
            if time.time() > deadline:
                raise AssertionError(f"timed out waiting for {what}")
            time.sleep(0.4)

    def seen(self, label: str, role: str | None = None, exact: bool = False, timeout: float = 12) -> dict[str, Any]:
        return self.wait(lambda: self.items(label, role, exact)[:1], f"{role or 'text'} {label}", timeout)[0]

    def gone(self, label: str, role: str | None = None, exact: bool = False, timeout: float = 12) -> None:
        self.wait(lambda: not self.items(label, role, exact), f"{label} to leave", timeout)

    def absent(self, label: str, role: str | None = None, exact: bool = True) -> None:
        self.refresh()
        if self.items(label, role, exact):
            raise AssertionError(f"unexpected {role or 'text'} {label}")

    def hit(self, resolve: Callable[[], Any], what: str, name: str = "click", arguments: dict[str, Any] | None = None) -> None:
        """Resolve an element from a fresh snapshot and act on it.

        A snapshot can be superseded between the read and the action (the page re-renders, or another
        driver client snapshots). The driver refuses such a token before acting, so re-resolving is safe."""
        for attempt in range(5):
            found = self.wait(resolve, what)
            element = found[0] if isinstance(found, list) else found
            try:
                self.act(name, {"element_token": element["element_token"], **(arguments or {"action": "press", "delivery_mode": "background"})})
                return
            except RuntimeError as exc:
                if attempt == 4 or not any(sign in str(exc) for sign in ("stale", "element_outside_target_window")):
                    raise
                time.sleep(0.4)

    def press(self, label: str, role: str | None = "AXButton", exact: bool = True, index: int = 0) -> None:
        self.trace.append(f"press {role} {label}")
        self.hit(lambda: (self.items(label, role, exact)[index:index + 1] or [None])[0], f"{role} {label}")
        time.sleep(0.6)

    def choose(self, trigger: str, option: str) -> None:
        """Open a SearchSelect by its label and press the option row's button."""
        self.trace.append(f"choose {trigger} -> {option}")

        def inside(row: dict[str, Any], box: dict[str, Any]) -> bool:
            frame = row.get("frame") or {}
            x, y = float(frame.get("x", -1)), float(frame.get("y", -1))
            return box["x"] <= x < box["x"] + box["w"] and box["y"] <= y < box["y"] + box["h"]

        def rows() -> list[dict[str, Any]]:
            # WebKit flattens the listbox's option buttons into full-width static text inside an AXList.
            lists = [e.get("frame") for e in self.elements if e.get("role") == "AXList" and e.get("frame")]
            found = [e for e in self.items(option) if e.get("role") in ("AXButton", "AXMenuItem", "AXCell", "AXRow")]
            found += [e for e in self.items(option, "AXStaticText") if any(inside(e, box) for box in lists)]
            return found[-1:]

        self.refresh()
        if not rows():
            self.press(trigger, None)
        self.hit(rows, f"option {option}")
        time.sleep(0.6)

    def close_siblings(self) -> None:
        """Close other on-screen windows of the owned process (a mini app opened by a stray press).

        A second visible window makes background typing ambiguous, so the driver refuses it."""
        listed = self.call("list_windows", {})
        windows = (listed.get("structuredContent") or listed).get("windows") or []
        for window in windows:
            if window.get("pid") != self.pid or window.get("window_id") == self.window_id:
                continue
            if not window.get("is_on_screen") or not window.get("title"):
                continue
            sibling = Drive(self.pid, int(window["window_id"]))
            try:
                sibling.refresh()
                close = [e for e in sibling.elements if e.get("role") == "AXButton" and norm(e.get("label")) == "关闭"]
                if close:
                    sibling.call(
                        "click",
                        {
                            "pid": self.pid,
                            "window_id": int(window["window_id"]),
                            "element_token": close[0]["element_token"],
                            "action": "press",
                            "delivery_mode": "background",
                        },
                    )
                    print(f"closed stray window {window.get('title')!r}", file=sys.stderr)
            finally:
                sibling.close()

    def nav(self, label: str) -> None:
        """Press a team sidebar entry. The app rail has same-named buttons (设置) further left."""
        self.trace.append(f"nav {label}")
        def pick() -> dict[str, Any] | None:
            rows = self.items(label, "AXButton")
            if len(rows) > 1:
                rail = min(float((row.get("frame") or {}).get("x", 0)) for row in rows)
                rows = [row for row in rows if float((row.get("frame") or {}).get("x", 0)) > rail + 30]
            return rows[0] if rows else None

        self.hit(pick, f"sidebar {label}")
        time.sleep(0.6)

    def check(self, label: str, on: bool = True) -> None:
        self.trace.append(f"check {label}={on}")
        element = self.seen(label, "AXCheckBox")
        if (str(element.get("value")) == "1") != on:
            self.hit(lambda: self.items(label, "AXCheckBox", exact=False)[:1], f"checkbox {label}")
            self.wait(
                lambda: [e for e in self.items(label, "AXCheckBox", exact=False) if (str(e.get("value")) == "1") == on],
                f"{label} checked={on}",
            )

    def field_value(self, label: str) -> str:
        return norm(self.seen(label, "AXTextField", exact=True).get("value"))

    def type_into(self, label: str, text: str) -> None:
        self.trace.append(f"type {label}")
        # Process-scoped key events are refused while a sibling window of this pid is visible.
        self.close_siblings()
        self.hit(
            lambda: self.items(label, "AXTextField")[:1],
            f"field {label}",
            "type_text",
            {"text": text, "delivery_mode": "background", "delay_ms": 0},
        )
        time.sleep(0.3)

    def snapshot(self) -> list[str]:
        self.refresh()
        rows = []
        for element in self.items():
            text = norm(element.get("label") or element.get("value"))
            if text:
                rows.append(f"{element.get('role')}\t{SECRET.sub('<redacted>', text)}")
        return rows

    def expose(self) -> None:
        """Keep a strip of the window uncovered. WebKit pauses painting and CSS
        animations for a fully covered window, so a closing dialog never unmounts.
        The frame is set through accessibility; nothing is activated or clicked."""
        rows = (self.call("list_windows", {"on_screen_only": True}).get("structuredContent") or {}).get("windows") or []
        mine = next((row for row in rows if row.get("window_id") == self.window_id), None)
        if mine is None:
            return
        bounds = mine.get("bounds") or {}
        height = float(bounds.get("height", 920))
        screen = (self.call("get_screen_size", {}).get("structuredContent") or {})
        screen_h = float(screen.get("height", 1080))
        z = mine.get("z_index")
        above = [
            row
            for row in rows
            if row.get("pid") != self.pid
            and row.get("app_name") != "Cua Driver"
            and isinstance(row.get("z_index"), int)
            and isinstance(z, int)
            and row["z_index"] > z
        ]
        covered = max(((row.get("bounds") or {}).get("y", 0) + (row.get("bounds") or {}).get("height", 0) for row in above), default=0)
        if covered + 24 > screen_h:
            print("WARN every edge of the owned window is covered; WebKit may pause rendering")
            return
        if float(bounds.get("y", 0)) + height >= screen_h - 2:
            return
        self.act(
            "set_window_frame",
            {"x": bounds.get("x", 240), "y": max(40.0, screen_h - height), "width": bounds.get("width", 1440), "height": height},
        )
        time.sleep(0.8)


def member_api(path: str, method: str = "GET") -> Any:
    """Owner-side read-back with the copied session. Only states are printed."""
    token = json.loads(SESSION.read_text())["token"]
    request = urllib.request.Request(API + path, method=method, headers={"authorization": f"Bearer {token}"})
    deadline = time.time() + 180
    while True:
        try:
            body = json.load(urllib.request.urlopen(request, timeout=10))
            break
        except urllib.error.URLError as exc:
            # The shared dev API restarts whenever another session edits it; only a refused connection waits.
            if isinstance(exc, urllib.error.HTTPError) or time.time() > deadline:
                raise
            time.sleep(3)
    return body.get("data", body) if isinstance(body, dict) and "data" in body else body


def wait_api(timeout: float = 240) -> None:
    deadline = time.time() + timeout
    while True:
        try:
            with urllib.request.urlopen(API.split("/v1/")[0] + "/health", timeout=5) as answer:
                if answer.status == 200:
                    return
        except (urllib.error.URLError, OSError):
            pass
        if time.time() > deadline:
            raise AssertionError("API on :3000 did not come back")
        time.sleep(3)


class Run:
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        self.ui = ui
        self.team = team
        self.evidence = evidence
        self.org_id = ""
        stamp = time.strftime("%H%M%S")
        self.token_name = f"cua-e2e-{stamp}"
        self.phone = f"1390000{stamp[-4:]}"

    def record(self, case: str) -> None:
        if self.evidence is None:
            return
        self.evidence.mkdir(parents=True, exist_ok=True)
        (self.evidence / f"{case}.ax.txt").write_text("\n".join(self.ui.snapshot()) + "\n")

    def grab(self, case: str) -> None:
        if self.evidence is not None:
            self.ui.grab(str((self.evidence / f"{case}.png").resolve()))

    # ── cases ────────────────────────────────────────────────────────

    def c01_team_center(self) -> None:
        """Switch the window to the target team and open 团队."""
        orgs = member_api("/orgs")
        self.org_id = next(org["id"] for org in orgs if org.get("name") == self.team)
        self.enter_team()

    def enter_team(self) -> None:
        """The shared dev server reloads the page when any session edits the renderer.
        Each case re-enters the team page instead of trusting the previous case's state."""
        ui = self.ui
        rail = T("sidebar.center.teams")
        ui.refresh()
        if ui.items(self.team, "AXHeading"):
            return
        for leftover in ("Close", T("common.cancel")):
            if ui.items(leftover, "AXButton"):
                ui.press(leftover)
                ui.refresh()
                if ui.items(self.team, "AXHeading"):
                    return
        if ui.items(rail, "AXButton"):
            ui.press(rail, "AXButton")
            try:
                ui.seen(self.team, "AXHeading", exact=True, timeout=5)
                return
            except AssertionError:
                pass
        ui.press("林书豪", "AXPopUpButton")
        ui.press(rail, "AXRadioButton")
        ui.press(self.team, "AXMenuItem", exact=False)
        ui.refresh()
        if not ui.items(self.team, "AXHeading"):
            ui.press(rail, "AXButton")
        ui.seen(self.team, "AXHeading", exact=True)

    def c02_navigation(self) -> None:
        """研发 holds repositories and tokens; 系统 holds usage, plan, settings, in that order."""
        ui = self.ui
        ui.refresh()
        order = [
            T("org.nav.group.dev"),
            T("org.nav.repositories"),
            T("org.nav.tokens"),
            T("org.nav.group.admin"),
            T("org.nav.usage"),
            T("org.nav.plan"),
            T("org.nav.settings"),
        ]
        ys = []
        for label in order:
            rows = [e for e in ui.items(label) if e.get("role") in ("AXButton", "AXStaticText")]
            rows = [e for e in rows if float((e.get("frame") or {}).get("x", 9999)) < 560]
            if not rows:
                raise AssertionError(f"sidebar is missing {label}")
            ys.append(float(rows[0]["frame"]["y"]))
        if ys != sorted(ys):
            raise AssertionError(f"sidebar order {list(zip(order, ys))}")
        self.record("c02_navigation")

    def c03_plan(self) -> None:
        ui = self.ui
        ui.nav(T("org.nav.plan"))
        ui.seen(T("org.plan.current"), "AXHeading", exact=True)
        subscription = member_api(f"/orgs/{self.org_id}/subscription")
        ui.seen(T(f"org.plan.name.{subscription['plan_id']}"))
        if subscription.get("self_serve"):
            ui.seen(T("org.plan.change"), "AXButton", exact=True)
        else:
            ui.seen(T("org.plan.selfServeOff"))
            ui.absent(T("org.plan.change"), "AXButton")
        self.record("c03_plan")
        self.grab("c03_plan")

    def c04_repositories(self) -> None:
        ui = self.ui
        ui.nav(T("org.nav.repositories"))
        ui.seen(T("org.repos.noNamespaceTitle"))
        self.record("c04_repositories")

    def c05_tokens(self) -> None:
        """Create a read token, see the secret once, confirm saving, then revoke it."""
        ui = self.ui
        self.token_name = f"cua-e2e-{time.strftime('%H%M%S')}"
        ui.nav(T("org.nav.tokens"))
        ui.press(T("org.tokens.create"))
        ui.seen(T("org.tokens.create"), "AXHeading", exact=True)
        ui.type_into(T("org.tokens.col.name"), self.token_name)
        typed = ui.field_value(T("org.tokens.col.name"))
        if typed.lower() != self.token_name.lower():
            raise AssertionError(f"name field shows {typed!r}")
        # The input method may capitalise the first letter; the shown value is the name.
        self.token_name = typed
        ui.check(T("org.tokens.scope.repository_read"))
        ui.check(T("org.tokens.scope.repository_write"), on=False)
        ui.press(T("org.tokens.create"), "AXButton")
        ui.seen(T("org.tokens.issuedTitle"), "AXHeading", exact=True)
        ui.refresh()
        done = ui.items(T("org.tokens.done"), "AXButton")
        if done and done[0].get("enabled") is not False:
            raise AssertionError("done is enabled before the secret is confirmed saved")
        ui.check(T("org.tokens.savedConfirm"))
        ui.press(T("org.tokens.done"))
        ui.gone(T("org.tokens.issuedTitle"), "AXHeading")
        ui.seen(self.token_name)
        ui.seen(T("org.tokens.state.active"))
        self.record("c05_tokens_created")
        ui.press(f"{T('org.tokens.revoke')} {self.token_name}")
        ui.press(T("org.tokens.revoke"))
        ui.seen(T("org.tokens.state.revoked"))
        row = next(token for token in member_api("/access-tokens") if token.get("name") == self.token_name)
        if not row.get("revoked_at"):
            raise AssertionError("owner still lists the token as live")
        if row.get("scopes") != ["repository:read"]:
            raise AssertionError(f"owner scopes {row.get('scopes')}")
        self.record("c05_tokens_revoked")
        self.grab("c05_tokens_revoked")

    def c06_members(self) -> None:
        """Status filter defaults to 在职; the creator row offers no status change."""
        ui = self.ui
        ui.nav(T("org.nav.members"))
        # The page remembers the last segment; c07 leaves it on 邀请.
        ui.press(T("org.members.filter.members"), "AXRadioButton", exact=False)
        trigger = T("org.members.statusFilter")
        ui.seen(trigger, None, exact=True)
        ui.choose(trigger, T("org.members.statusFilter.disabled"))
        ui.seen(T("org.members.noMatch"))
        ui.choose(trigger, T("org.members.statusFilter.active"))
        ui.seen("林书豪")
        for action in ("org.status.disable", "org.status.resign"):
            ui.absent(T(action), "AXMenuItem")
        self.record("c06_members")

    def c07_invite(self) -> None:
        """Seat line, phone invite, the invites segment, and revoke."""
        ui = self.ui
        self.phone = f"1390000{time.strftime('%M%S')}"
        ui.nav(T("org.nav.members"))
        subscription = member_api(f"/orgs/{self.org_id}/subscription")
        members = member_api(f"/orgs/{self.org_id}/members")
        members = members.get("items", []) if isinstance(members, dict) else members
        invites = member_api(f"/invites?org_id={self.org_id}")
        used = sum(row.get("status") == "active" for row in members) + sum(
            row.get("status") == "pending" for row in invites
        )
        ui.press(T("org.invite.open"))
        ui.seen(T("org.invites.phoneTitle"))
        ui.seen(f"{used} / {subscription['seats']}", exact=False)
        ui.type_into(T("org.invites.phone"), self.phone)
        ui.press(T("org.invites.send"))
        ui.seen(T("org.invites.sent"), timeout=10)
        self.record("c07_invite_sent")
        ui.press("Close")
        ui.gone(T("org.invites.phoneTitle"))
        ui.press(T("org.members.filter.invites"), "AXRadioButton", exact=False)
        masked = f"{self.phone[:3]}****{self.phone[7:]}"
        ui.seen(masked)
        ui.press(f"{T('org.invites.revoke')} {masked}")
        ui.press(T("org.invites.revoke"))
        ui.seen(T("org.invites.revoked"), timeout=10)
        ui.gone(f"{T('org.invites.revoke')} {masked}", "AXButton")
        invite = next(row for row in member_api(f"/invites?org_id={self.org_id}") if row.get("phone") == self.phone)
        if invite.get("status") != "revoked":
            raise AssertionError(f"owner invite status {invite.get('status')}")
        self.record("c07_invite_revoked")

    def c08_settings(self) -> None:
        """Three cards, the verification state machine for a creator, and the danger zone order."""
        ui = self.ui
        ui.nav(T("org.nav.settings"))
        ui.seen(T("org.settings.profile"))
        ui.seen(T("org.verification.title"))
        ui.seen(T("org.settings.danger"))
        status = member_api(f"/orgs/{self.org_id}/verification")["status"]
        ui.seen(T(f"org.verification.status.{status}"))
        personal = member_api("/verification")["status"]
        ui.seen(T("org.settings.transfer"))
        ui.seen(T("org.settings.dissolve"))
        ui.absent(T("org.settings.leave"))
        self.record("c08_settings")
        self.grab("c08_settings")
        if status in ("unverified", "rejected") and personal != "verified":
            ui.seen(T("org.verification.personalRequired") if personal != "pending" else T("org.verification.personalPending"))
            if personal != "pending":
                ui.press(T("org.verification.goPersonal"))
                ui.seen(T("org.verification.personalTitle"), "AXHeading", exact=True)
                ui.type_into(T("org.verification.realName"), "测试")
                ui.type_into(T("org.verification.idNumber"), "12345")
                ui.press(T("org.verification.submit"))
                ui.seen(T("org.verification.idInvalid"))
                self.record("c08_personal_invalid")
                ui.press(T("common.cancel"))
                ui.gone(T("org.verification.personalTitle"), "AXHeading")
                if member_api("/verification")["status"] != personal:
                    raise AssertionError("a local validation failure reached the owner")


CASES = ["c01_team_center", "c02_navigation", "c03_plan", "c04_repositories", "c05_tokens", "c06_members", "c07_invite", "c08_settings"]


def run(ui: Drive, team: str, evidence: Path | None) -> int:
    assert isinstance(ui, Ui)
    session = Run(ui, team, evidence)
    failed = 0
    lines = []
    for case in CASES:
        started = time.time()
        line = ""
        # A case gets one retry: the shared dev server reloads the page whenever any session edits
        # the renderer, which resets the team view. Retries are reported, never hidden.
        for attempt in (1, 2):
            ui.trace.clear()
            try:
                wait_api()
                ui.close_siblings()
                ui.expose()
                if case != "c01_team_center" or attempt > 1:
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
        if line.startswith("FAIL"):
            failed += 1
        print(line)
        lines.append(line)
        sys.stdout.flush()
    routes = sorted({f"{name}:{route or '-'}:{mode or '-'}" for name, route, mode in ui.routes})
    print("routes " + ", ".join(routes))
    print(f"done passed={len(CASES) - failed} failed={failed}")
    if evidence is not None:
        (evidence / "console.txt").write_text("\n".join(lines + ["routes " + ", ".join(routes)]) + "\n")
    return 1 if failed else 0


if __name__ == "__main__":
    main()
