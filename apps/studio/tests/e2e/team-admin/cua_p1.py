"""Team admin P1 acceptance on the owned ToWork window, cua-driver background mode only.

Chain: create a repository → issue a write token → clone with it → push a branch →
require one approval on main → open a change request → a second member approves →
merge, all driven from Studio except the second member's approval and the `git`
commands, which a person would type in a terminal.

Fixtures (command `setup`, idempotent):
  * The team's code namespace and seats are operator facts. Member-plane teams have no
    operator command that grants a namespace yet, so setup writes both into the dev
    database named by A3S_E2E_DATABASE_URL. Nothing else touches the database.
  * The second member registers with the dev SMS echo, joins through a phone invite, and
    holds a custom role whose only rule is `review Repository`. Its session is kept in the
    owned instance's support directory with mode 0600.

Secrets: member sessions, the reviewer password and the issued access token stay in
memory or 0600 files. Logs and evidence are redacted with the P0 pattern.

Usage:
  A3S_E2E_SESSION_FROM=<member-session.json> A3S_E2E_TEAM=<team> \\
  A3S_E2E_DATABASE_URL=<dev postgres url> python3 cua_p1.py setup
  A3S_E2E_SESSION_FROM=... A3S_E2E_TEAM=... python3 cua_p1.py run [evidence-dir]
"""

from __future__ import annotations

import json
import os
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cua_p0 import SECRET, SESSION, STORAGE, SUPPORT, T, Run, Ui, fail_closed, launch, norm, wait_api  # noqa: E402

ORIGIN = "http://127.0.0.1:3000"
REVIEWER = SUPPORT / "p1-reviewer.json"
REVIEW_ROLE = "仓库审阅（E2E）"


def creator_token() -> str:
    return json.loads(SESSION.read_text())["token"]


def call(path: str, method: str = "GET", body: Any = None, token: str | None = None) -> Any:
    """One member-plane request. Errors carry the status and the server message, never the token."""
    data = None if body is None else json.dumps(body).encode()
    headers = {"content-type": "application/json"} if data is not None else {}
    if token:
        headers["authorization"] = f"Bearer {token}"
    request = urllib.request.Request(ORIGIN + path, data=data, method=method, headers=headers)
    deadline = time.time() + 180
    while True:
        try:
            with urllib.request.urlopen(request, timeout=15) as answer:
                raw = answer.read()
            break
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode(errors="replace")[:600]
            raise RuntimeError(f"{method} {path} -> {exc.code} {SECRET.sub('<redacted>', detail)}") from None
        except urllib.error.URLError:
            if time.time() > deadline:
                raise
            time.sleep(3)
    payload = json.loads(raw) if raw else None
    return payload.get("data", payload) if isinstance(payload, dict) and "data" in payload else payload


def team_id(team: str) -> str:
    return next(org["id"] for org in call("/v1/api/org/members/orgs", token=creator_token()) if org.get("name") == team)


def write_private(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value))
    os.chmod(temporary, 0o600)
    temporary.replace(path)


# ── fixtures ────────────────────────────────────────────────────────


def operator_fixture(org_id: str) -> str:
    """Stand-in for operator provisioning: one org namespace and room for a second seat."""
    url = os.environ.get("A3S_E2E_DATABASE_URL", "").strip()
    if not url:
        raise SystemExit("FAIL closed: setup needs A3S_E2E_DATABASE_URL")
    import psycopg2  # type: ignore[import-not-found]

    connection = psycopg2.connect(url)
    connection.autocommit = True
    cursor = connection.cursor()
    cursor.execute("select name from package_namespaces where owner_kind = 'org' and owner_id = %s", (org_id,))
    row = cursor.fetchone()
    if row:
        namespace = row[0]
        print(f"fixture namespace={namespace} (existing)")
    else:
        # Namespaces outlive dissolved teams; one per team id never collides.
        namespace = f"cua-{org_id[:8]}"
        cursor.execute(
            "insert into package_namespaces (name, owner_kind, owner_id) values (%s, 'org', %s)",
            (namespace, org_id),
        )
        print(f"fixture namespace={namespace} (granted)")
    # A team that never changed plan has no row; the owner reads it as Free with one seat.
    cursor.execute("insert into org_subscriptions (org_id) values (%s) on conflict (org_id) do nothing", (org_id,))
    cursor.execute("select plan_id, seats from org_subscriptions where org_id = %s", (org_id,))
    plan, seats = cursor.fetchone()
    if seats < 3:
        cursor.execute("update org_subscriptions set plan_id = 'team', seats = 3 where org_id = %s", (org_id,))
        print(f"fixture seats {plan}/{seats} -> team/3")
    else:
        print(f"fixture seats {plan}/{seats} (enough)")
    connection.close()
    return namespace


def reviewer_session() -> dict[str, Any]:
    if REVIEWER.is_file():
        saved = json.loads(REVIEWER.read_text())
        try:
            call("/v1/api/org/members/me", token=saved["token"])
            return saved
        except RuntimeError:
            pass
    phone = "1390001" + time.strftime("%M%S")
    code = call("/v1/api/org/members/phone-codes", "POST", {"phone": phone}).get("dev_code")
    if not code:
        raise SystemExit("FAIL closed: the dev API does not echo SMS codes")
    auth = call(
        "/v1/api/org/members/registrations",
        "POST",
        {"phone": phone, "code": code, "password": "Cua!" + secrets.token_urlsafe(12), "display_name": "CUA 审阅人", "computer_id": "cua-e2e-reviewer"},
    )
    saved = {"phone": phone, "token": auth["token"], "member_id": auth["member"]["id"]}
    write_private(REVIEWER, saved)
    print(f"fixture reviewer registered phone={phone[:3]}****{phone[7:]}")
    return saved


def reviewer_role(org_id: str) -> dict[str, Any]:
    token = creator_token()
    roles = call(f"/v1/api/org/members/orgs/{org_id}/roles", token=token)
    existing = next((role for role in roles if role.get("name") == REVIEW_ROLE), None)
    if existing:
        return existing
    role = call(
        f"/v1/api/org/members/orgs/{org_id}/roles",
        "POST",
        {
            "name": REVIEW_ROLE,
            "description": "",
            "rules": [{"action": "review", "subject": "Repository"}],
            "data_scope": {"kind": "self", "department_ids": []},
        },
        token,
    )
    print(f"fixture role {REVIEW_ROLE} created")
    return role


def join_reviewer(org_id: str, reviewer: dict[str, Any], role: dict[str, Any]) -> None:
    """Phone invites carry built-in roles only, so the custom role is assigned after joining."""
    mine = call("/v1/api/org/members/orgs", token=reviewer["token"])
    if any(org.get("id") == org_id and org.get("membership") != "pending" for org in mine):
        print("fixture reviewer already in the team")
    else:
        invite = call(
            "/v1/api/org/members/invites", "POST", {"org_id": org_id, "phone": reviewer["phone"]}, creator_token()
        )
        call(f"/v1/api/org/members/invite-codes/{invite['code']}/memberships", "POST", None, reviewer["token"])
        print("fixture reviewer joined through a phone invite")
    members = call(f"/v1/api/org/members/orgs/{org_id}/members", token=creator_token())
    members = members.get("items", []) if isinstance(members, dict) else members
    row = next(row for row in members if row.get("member_id") == reviewer["member_id"])
    if row.get("role_code") != role["code"]:
        call(
            f"/v1/api/org/members/orgs/{org_id}/members/{row['id']}/role",
            "PUT",
            {"role_id": role["id"]},
            creator_token(),
        )
        print(f"fixture reviewer role -> {REVIEW_ROLE}")


def setup(team: str) -> None:
    org_id = team_id(team)
    namespace = operator_fixture(org_id)
    reviewer = reviewer_session()
    join_reviewer(org_id, reviewer, reviewer_role(org_id))
    answer = call(f"/v1/api/org/repository-namespaces/{org_id}", token=reviewer["token"])
    if answer.get("namespace") != namespace or not answer["permissions"]["review"] or answer["permissions"]["write"]:
        raise SystemExit(f"FAIL reviewer grant {answer}")
    print(f"fixture ready namespace={namespace} reviewer=read+review")


# ── git, as a person would type it ──────────────────────────────────


class Git:
    def __init__(self) -> None:
        self.root = Path(tempfile.mkdtemp(prefix="cua-p1-"))
        self.askpass = self.root / "askpass.sh"
        self.askpass.write_text('#!/bin/sh\nprintf "%s" "$A3S_E2E_GIT_TOKEN"\n')
        os.chmod(self.askpass, 0o700)
        self.token = ""

    def run(self, *args: str, cwd: Path | None = None) -> str:
        env = {
            **os.environ,
            "GIT_ASKPASS": str(self.askpass),
            "GIT_TERMINAL_PROMPT": "0",
            "A3S_E2E_GIT_TOKEN": self.token,
            "GIT_AUTHOR_NAME": "CUA E2E",
            "GIT_AUTHOR_EMAIL": "cua-e2e@example.invalid",
            "GIT_COMMITTER_NAME": "CUA E2E",
            "GIT_COMMITTER_EMAIL": "cua-e2e@example.invalid",
        }
        done = subprocess.run(
            ["git", "-c", "credential.helper=", *args],
            cwd=cwd or self.root,
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
        )
        if done.returncode != 0:
            raise AssertionError(f"git {args[0]} failed: {SECRET.sub('<redacted>', done.stderr.strip())[:300]}")
        return done.stdout.strip()

    def close(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


# ── cases ───────────────────────────────────────────────────────────


class P1(Run):
    def __init__(self, ui: Ui, team: str, evidence: Path | None) -> None:
        super().__init__(ui, team, evidence)
        stamp = time.strftime("%H%M%S")
        self.repo = f"cua-{stamp}"
        self.branch = f"feature/cua-{stamp}"
        self.title = f"CUA change {stamp}"
        self.namespace = ""
        self.clone_url = ""
        self.number = 0
        self.token_name = ""
        self.git = Git()

    def api(self, path: str, method: str = "GET", body: Any = None) -> Any:
        return call(path, method, body, creator_token())

    def repository_exists(self) -> bool:
        try:
            self.api(self.repo_path())
        except RuntimeError as exc:
            if " -> 404 " in str(exc):
                return False
            raise
        return True

    def open_request_number(self) -> int:
        rows = self.api(self.repo_path("/change-requests?state=open"))
        return next((row["number"] for row in rows if row["title"] == self.title), 0)

    def repo_path(self, rest: str = "") -> str:
        return f"/v1/api/org/repositories/{self.namespace}/{self.repo}{rest}"

    def to_list(self) -> None:
        """The repositories page restores its last view; the breadcrumb (right of the sidebar) leads back."""
        ui = self.ui
        ui.nav(T("org.nav.repositories"))
        description = T("org.repos.description", namespace=self.namespace)

        def settled() -> bool:
            """The list, twice in a row: a restored view can replace it right after the first paint."""
            ui.refresh()
            if not ui.items(description):
                return False
            time.sleep(0.8)
            ui.refresh()
            return bool(ui.items(description))

        ui.seen(T("org.nav.repositories"), "AXButton")
        if settled():
            return
        ui.hit(lambda: sorted(ui.items(T("org.nav.repositories"), "AXButton"), key=lambda e: float((e.get("frame") or {}).get("x", 0)))[-1:], "breadcrumb")
        ui.wait(settled, f"text {description}")

    def open_repository(self) -> None:
        """Back to this repository's page whatever the shared dev server did to the view."""
        ui = self.ui
        ui.refresh()
        if ui.items(self.repo, "AXHeading", exact=False):
            return
        self.to_list()
        ui.press(self.repo, None, exact=True)
        ui.seen(self.repo, "AXHeading", exact=False)

    def tab(self, key: str) -> None:
        self.ui.press(T(key), "AXRadioButton", exact=True) if self.ui.items(T(key), "AXRadioButton") else self.ui.press(T(key), None)

    def c01_team_center(self) -> None:
        orgs = self.api("/v1/api/org/members/orgs")
        self.org_id = next(org["id"] for org in orgs if org.get("name") == self.team)
        self.namespace = self.api(f"/v1/api/org/repository-namespaces/{self.org_id}")["namespace"] or ""
        if not self.namespace:
            raise AssertionError("the team has no namespace; run setup first")
        self.enter_team()

    def c02_create_repository(self) -> None:
        """W13 list, the admin-only create dialog, and the new repository's code tab with its README."""
        ui = self.ui
        # A retry after a page reset resumes: the repository the first attempt created is opened, not created twice.
        if self.repository_exists():
            self.open_repository()
        else:
            self.to_list()
            ui.press(T("org.repos.create"))
            ui.seen(T("org.repos.create"), "AXHeading", exact=True)
            ui.type_into(T("org.repos.col.name"), self.repo)
            typed = ui.field_value(T("org.repos.col.name"))
            if typed != self.repo:
                raise AssertionError(f"name field shows {typed!r}")
            ui.press(T("org.repos.create"), "AXButton", index=len(ui.items(T("org.repos.create"), "AXButton")) - 1)
            ui.seen(self.repo, "AXHeading", exact=False, timeout=20)
        ui.seen("README.md")
        detail = self.api(self.repo_path())
        if detail["default_branch"] != "main" or not detail["permissions"]["admin"]:
            raise AssertionError(f"owner detail {detail['default_branch']} {detail['permissions']}")
        self.record("c02_repository_created")
        self.grab("c02_repository_created")

    def c03_token(self) -> None:
        """A write token, shown once; the harness keeps it in memory for git."""
        ui = self.ui
        # A token whose secret this run never captured is useless; revoke it before issuing the next one.
        for token in self.api("/v1/api/org/members/access-tokens"):
            if self.token_name and token.get("name") == self.token_name and not token.get("revoked_at"):
                self.api(f"/v1/api/org/members/access-tokens/{token['id']}", "DELETE")
        self.git.token = ""
        name = f"cua-p1-{time.strftime('%H%M%S')}"
        self.token_name = name
        ui.nav(T("org.nav.tokens"))
        ui.press(T("org.tokens.create"))
        ui.seen(T("org.tokens.create"), "AXHeading", exact=True)
        ui.type_into(T("org.tokens.col.name"), name)
        self.token_name = ui.field_value(T("org.tokens.col.name"))
        if self.token_name != name:
            raise AssertionError(f"name field shows {self.token_name!r}")
        ui.check(T("org.tokens.scope.repository_read"))
        ui.check(T("org.tokens.scope.repository_write"))
        ui.press(T("org.tokens.create"), "AXButton")
        ui.seen(T("org.tokens.issuedTitle"), "AXHeading", exact=True)
        secret = norm(ui.seen(T("org.tokens.secret"), "AXTextField", exact=True).get("value"))
        if not secret.startswith("a3s_pat_"):
            raise AssertionError("the issued dialog shows no access token")
        self.git.token = secret
        ui.check(T("org.tokens.savedConfirm"))
        ui.press(T("org.tokens.done"))
        ui.gone(T("org.tokens.issuedTitle"), "AXHeading")
        tokens = self.api("/v1/api/org/members/access-tokens")
        row = next((token for token in tokens if token.get("name") == self.token_name), None)
        if row is None:
            raise AssertionError(f"no token named {self.token_name!r}; newest {tokens[0].get('name') if tokens else None!r}")
        if sorted(row.get("scopes") or []) != ["repository:read", "repository:write"]:
            raise AssertionError(f"owner scopes {row.get('scopes')}")
        self.record("c03_token")

    def c04_clone_and_push(self) -> None:
        """The clone panel's URL, then clone, branch, commit and push with the token as password."""
        ui = self.ui
        self.open_repository()
        ui.press(T("org.repos.clone"), "AXPopUpButton", exact=False)
        url = norm(ui.seen(T("org.repos.cloneUrl"), "AXTextField", exact=True).get("value"))
        ui.seen(T("org.repos.cloneHint"))
        ui.seen(T("org.repos.goTokens"), "AXButton", exact=False)
        self.record("c04_clone_panel")
        self.grab("c04_clone_panel")
        expected = f"/git/{self.namespace}/{self.repo}.git"
        if not url.endswith(expected):
            raise AssertionError(f"clone URL {url!r} does not end with {expected}")
        self.clone_url = url
        ui.press(T("org.repos.clone"), "AXPopUpButton", exact=False)
        work = self.git.root / "work"
        shutil.rmtree(work, ignore_errors=True)
        self.git.run("clone", url, str(work))
        if not (work / "README.md").is_file():
            raise AssertionError("the clone has no README.md")
        if self.branch in [branch["name"] for branch in self.api(self.repo_path("/branches"))]:
            self.git.run("switch", self.branch, cwd=work)
        else:
            self.git.run("switch", "-c", self.branch, cwd=work)
            (work / "hello.txt").write_text("hello from the P1 acceptance run\n")
            self.git.run("add", "hello.txt", cwd=work)
            self.git.run("commit", "-m", "Add hello", cwd=work)
            self.git.run("push", "-u", "origin", self.branch, cwd=work)
        if not (work / "hello.txt").is_file():
            raise AssertionError("the pushed branch has no hello.txt")
        branches = [branch["name"] for branch in self.api(self.repo_path("/branches"))]
        if self.branch not in branches:
            raise AssertionError(f"owner branches {branches}")

    def c05_protect_main(self) -> None:
        """W14 refs tab; the admin raises main to one required approval (G5)."""
        ui = self.ui
        self.open_repository()
        self.tab("org.repos.tab.refs")
        ui.seen(self.branch, None, exact=True)
        ui.seen("refs/heads/main", None, exact=True)
        saved = next(rule for rule in self.api(self.repo_path("/protections")) if rule["pattern"] == "refs/heads/main")
        if saved["require_approvals"] == 1:
            ui.seen(T("org.repos.protect.approvalsN", count=1))
            self.record("c05_protections")
            self.grab("c05_protections")
            return
        ui.press(T("org.repos.protect.edit"))
        ui.seen(T("org.repos.protect.title"), "AXHeading", exact=True)
        label = T("org.repos.protect.approvals", max=10)
        patterns = [norm(e.get("value")) for e in ui.items(T("org.repos.protect.pattern"), "AXTextField")]
        main = patterns.index("refs/heads/main")
        ui.trace.append(f"type approvals row {main}")
        ui.close_siblings()
        # The field holds "0"; typing appends, and "01" saves as 1.
        ui.hit(
            lambda: ui.items(label, "AXTextField")[main:main + 1],
            "approvals field",
            "type_text",
            {"text": "1", "delivery_mode": "background", "delay_ms": 0},
        )
        ui.press(T("common.save"))
        ui.gone(T("org.repos.protect.hint"))
        ui.seen(T("org.repos.protect.approvalsN", count=1))
        rule = next(rule for rule in self.api(self.repo_path("/protections")) if rule["pattern"] == "refs/heads/main")
        if rule["require_approvals"] != 1 or not rule["no_force_push"]:
            raise AssertionError(f"owner protection {rule}")
        self.record("c05_protections")
        self.grab("c05_protections")

    def c06_open_change_request(self) -> None:
        """W14 requests tab and the open dialog; W15 then refuses to merge without an approval."""
        ui = self.ui
        if self.open_request_number():
            self.reopen_request()
        else:
            self.open_repository()
            self.tab("org.repos.tab.requests")
            ui.press(T("org.repos.cr.new"))
            ui.seen(T("org.repos.cr.new"), "AXHeading", exact=True)
            ui.choose(T("org.repos.cr.source"), self.branch)
            ui.type_into(T("org.repos.cr.titleLabel"), self.title)
            ui.press(T("org.repos.cr.submit"))
            ui.seen(self.title, "AXHeading", exact=False, timeout=20)
        self.number = self.open_request_number()
        if not self.number:
            raise AssertionError(f"no open change request titled {self.title!r}")
        ui.seen(T("org.repos.cr.needsApprovals", required=1, count=0))
        # The write actions are on screen; the driver lists a disabled button not at all.
        ui.seen(T("org.repos.cr.close"), "AXButton", exact=True)
        merge = ui.items(T("org.repos.cr.merge"), "AXButton")
        if merge and merge[0].get("enabled") is not False:
            raise AssertionError("merge is enabled before the approval")
        ui.absent(T("org.repos.cr.approve"), "AXButton")
        ui.seen(T("org.repos.cr.authorCannotApprove"))
        self.record("c06_change_request_open")
        self.grab("c06_change_request_open")

    def c07_review(self) -> None:
        """The second member approves; the author comments and sees the gate open."""
        ui = self.ui
        reviewer = json.loads(REVIEWER.read_text())
        if not self.api(self.repo_path(f"/change-requests/{self.number}"))["gate"]["approvals"]:
            call(self.repo_path(f"/change-requests/{self.number}/reviews"), "POST", {"verdict": "approved"}, reviewer["token"])
        self.reopen_request()
        ui.seen(T("org.repos.cr.approvals", count=1, required=1))
        review = self.api(self.repo_path(f"/change-requests/{self.number}"))["reviews"][-1]
        rows = self.api(f"/v1/api/org/members/orgs/{self.org_id}/members")
        rows = rows.get("items", rows) if isinstance(rows, dict) else rows
        reviewer_name = next(row["name"] for row in rows if row.get("member_id") == review["reviewer_member"])
        line = T("org.repos.cr.reviewLine.approved", who=reviewer_name, oid=review["reviewed_oid"][:7])
        ui.seen(line, "AXStaticText", exact=True)
        note = "Looks good, merging."
        comments = self.api(self.repo_path(f"/change-requests/{self.number}")).get("comments") or []
        if not any(comment.get("body") == note for comment in comments):
            ui.close_siblings()
            ui.hit(
                lambda: ui.items(T("org.repos.cr.commentLabel"), "AXTextArea")[:1],
                "comment box",
                "type_text",
                {"text": note, "delivery_mode": "background", "delay_ms": 0},
            )
            ui.press(T("org.repos.cr.comment"), "AXButton", exact=True)
        ui.seen(note, "AXStaticText", exact=True)
        merge = ui.seen(T("org.repos.cr.merge"), "AXButton", exact=True)
        if merge.get("enabled") is False:
            raise AssertionError("merge stays disabled after the approval")
        self.record("c07_reviewed")

    def reopen_request(self) -> None:
        ui = self.ui
        self.open_repository()
        self.tab("org.repos.tab.requests")
        ui.press(self.title, None, exact=False)
        ui.seen(self.title, "AXHeading", exact=False)

    def c08_merge(self) -> None:
        ui = self.ui
        ui.refresh()
        if not ui.items(self.title, "AXHeading", exact=False):
            self.reopen_request()
        if self.api(self.repo_path(f"/change-requests/{self.number}"))["state"] == "open":
            ui.press(T("org.repos.cr.merge"), "AXButton", exact=True)
        ui.seen(T("org.repos.cr.state.merged"), exact=True, timeout=20)
        detail = self.api(self.repo_path(f"/change-requests/{self.number}"))
        if detail["state"] != "merged" or not detail.get("merge"):
            raise AssertionError(f"owner state {detail['state']}")
        head = self.git.run("ls-remote", self.clone_url, "refs/heads/main").split()[0]
        if head != detail["merge"]["merged_oid"]:
            raise AssertionError("main does not point at the merge commit")
        ui.seen(T("org.repos.cr.mergedAs", oid=head[:7]), exact=False)
        self.record("c08_merged")
        self.grab("c08_merged")

    def c09_revoke(self) -> None:
        """Revoke the token in Studio; the same clone URL then refuses it."""
        ui = self.ui
        ui.nav(T("org.nav.tokens"))
        row = next(token for token in self.api("/v1/api/org/members/access-tokens") if token.get("name") == self.token_name)
        if not row.get("revoked_at"):
            ui.press(f"{T('org.tokens.revoke')} {self.token_name}")
            ui.press(T("org.tokens.revoke"))
        ui.seen(T("org.tokens.state.revoked"))
        try:
            self.git.run("ls-remote", self.clone_url)
        except AssertionError:
            pass
        else:
            raise AssertionError("a revoked token still reads the repository")
        self.git.token = ""
        self.record("c09_revoked")


CASES = [
    "c01_team_center",
    "c02_create_repository",
    "c03_token",
    "c04_clone_and_push",
    "c05_protect_main",
    "c06_open_change_request",
    "c07_review",
    "c08_merge",
    "c09_revoke",
]

def run(ui: Ui, team: str, evidence: Path | None) -> int:
    session = P1(ui, team, evidence)
    lines: list[str] = []
    failed = 0
    try:
        for case in CASES:
            started = time.time()
            line = ""
            # The shared dev server reloads the page whenever any session edits the renderer, so a case
            # gets one retry. Cases that create state read the server first and resume, never act twice.
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
        session.git.close()
    routes = sorted({f"{name}:{route or '-'}:{mode or '-'}" for name, route, mode in ui.routes})
    print("routes " + ", ".join(routes))
    print(f"done passed={len(lines) - failed} failed={failed} of {len(CASES)}")
    if evidence is not None:
        evidence.mkdir(parents=True, exist_ok=True)
        (evidence / "console.txt").write_text("\n".join(lines + ["routes " + ", ".join(routes)]) + "\n")
    return 1 if failed or len(lines) < len(CASES) else 0


def main() -> None:
    source, team = fail_closed()
    command = sys.argv[1] if len(sys.argv) > 1 else "run"
    if command == "setup":
        setup(team)
        return
    pid, window = launch(source)
    print(f"owned pid={pid} window={window} instance={STORAGE}")
    ui = Ui(pid, window)
    try:
        raise SystemExit(run(ui, team, Path(sys.argv[2]) if len(sys.argv) > 2 else None))
    finally:
        ui.close()


if __name__ == "__main__":
    main()
