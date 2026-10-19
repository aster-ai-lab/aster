"""Seed the office catalog on the live API. Tokens stay in a mode-0600 file.

The script prints org ids and display names only. It does not print passwords,
session tokens, or SMS codes.
"""

from __future__ import annotations

import json
import os
import secrets
import urllib.error
import urllib.request
import uuid
from pathlib import Path

API = "http://127.0.0.1:3000/v1/api/org/members"
ENV_FILE = Path("/Users/roylin/code/a3s/apps/api/.env")
SESSION_FILE = Path("/Users/roylin/Library/Caches/a3s-studio-office-e2e-sessions.json")
PASSWORD_FILE = Path("/Users/roylin/Library/Caches/a3s-studio-office-e2e-password")

PEOPLE = [
    ("owner", "组织所有者"),
    ("applicant", "申请人"),
    ("manager", "直属主管"),
    ("finance", "财务"),
    ("finance_lead", "财务负责人"),
    ("gm", "总经理"),
    ("hr", "HR"),
    ("admin", "行政"),
    ("leader", "部门负责人"),
    ("rd", "研发负责人"),
    ("security", "安全"),
    ("fulfill", "资源执行人"),
    ("bystander", "旁观者"),
    ("short", "短余额"),
    ("probation", "试用期"),
    ("empty", "空主管"),
]

ROLE_CODES = {
    "manager": "direct_manager",
    "leader": "department_head",
    "rd": "rd_lead",
    "finance": "finance",
    "finance_lead": "finance_lead",
    "gm": "gm",
    "hr": "hr",
    "admin": "admin_affairs",
    "security": "security",
    "fulfill": "fulfillment",
}


def database_url() -> str:
    for line in ENV_FILE.read_text().splitlines():
        if line.startswith("DATABASE_URL="):
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    raise SystemExit("DATABASE_URL is missing")


def request(method: str, path: str, body: dict | None = None, token: str | None = None) -> tuple[int, dict | list | None, str]:
    data = None if body is None else json.dumps(body).encode()
    headers = {"content-type": "application/json"}
    if token:
        headers["authorization"] = f"Bearer {token}"
    req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            raw = response.read().decode()
            status = response.status
    except urllib.error.HTTPError as error:
        raw = error.read().decode()
        status = error.code
    try:
        payload = json.loads(raw) if raw else None
    except json.JSONDecodeError:
        payload = {"message": raw[:180]}
    message = ""
    if isinstance(payload, dict):
        message = str(payload.get("message") or payload.get("error") or "")
        if "data" in payload and payload.get("success") is True:
            payload = payload["data"]
        elif payload.get("success") is False:
            payload = None
    return status, payload, message


def register(phone: str, password: str, name: str, computer_id: str) -> dict:
    status, sent, message = request("POST", "/phone-codes", {"phone": phone})
    auth = None
    if status == 200 and isinstance(sent, dict) and sent.get("dev_code"):
        code = str(sent["dev_code"])
        status, auth, message = request(
            "POST",
            "/registrations",
            {"phone": phone, "code": code, "password": password, "computer_id": computer_id},
        )
    if not isinstance(auth, dict) or "token" not in auth:
        status, auth, message = request(
            "POST",
            "/sessions",
            {"identifier": phone, "password": password, "computer_id": computer_id},
        )
        if status != 200 or not isinstance(auth, dict) or "token" not in auth:
            raise SystemExit(f"sign-in failed status={status} message={message}")
    token = str(auth["token"])
    status, _profile, message = request("PUT", "/me", {"display_name": name}, token)
    if status != 200:
        raise SystemExit(f"profile failed status={status} message={message}")
    member = auth["member"]
    return {
        "id": str(member["id"]).lower(),
        "token": token,
        "phone": phone,
        "name": name,
    }


def ensure_org(owner: dict, name: str) -> str:
    status, rows, message = request("GET", "/orgs", token=owner["token"])
    if status != 200 or not isinstance(rows, list):
        raise SystemExit(f"org list failed status={status} message={message}")
    for row in rows:
        if isinstance(row, dict) and row.get("name") == name:
            return str(row["id"])
    status, org, message = request("POST", "/orgs", {"name": name}, owner["token"])
    if status != 201 or not isinstance(org, dict):
        raise SystemExit(f"org failed status={status} message={message}")
    return str(org["id"])


def raise_seats(org_id: str) -> None:
    connection = connect()
    try:
        with connection:
            with connection.cursor() as cursor:
                cursor.execute(
                    """
                    insert into org_subscriptions (org_id, plan_id, status, seats)
                    values (%s, 'team', 'active', 20)
                    on conflict (org_id) do update
                    set plan_id = 'team', status = 'active', seats = 20, updated_at = now()
                    """,
                    (org_id,),
                )
    finally:
        connection.close()


def pending_invite_code(owner: dict, org_id: str, phone: str) -> str | None:
    status, rows, message = request("GET", f"/invites?org_id={org_id}", token=owner["token"])
    if status != 200 or not isinstance(rows, list):
        raise SystemExit(f"invite list failed status={status} message={message}")
    for row in rows:
        if not isinstance(row, dict):
            continue
        if row.get("phone") == phone and row.get("status") == "pending" and row.get("code"):
            return str(row["code"])
    return None


def ensure_roles_exist(owner: dict, org_id: str) -> None:
    connection = connect()
    try:
        with connection:
            with connection.cursor() as cursor:
                cursor.execute(
                    """
                    insert into org_roles (
                        id, org_id, code, name, description, built_in, sort_order, data_scope, permissions
                    )
                    select gen_random_uuid(), %s, source.code, source.name, source.description,
                           source.built_in, source.sort_order, source.data_scope, source.permissions
                    from org_roles source
                    where source.org_id = (
                        select org_id from org_roles where code = 'member' and org_id <> %s limit 1
                    )
                      and source.code in ('owner', 'admin', 'member')
                      and not exists (
                        select 1 from org_roles existing
                        where existing.org_id = %s and existing.code = source.code
                      )
                    """,
                    (org_id, org_id, org_id),
                )
                cursor.execute(
                    """
                    update users
                    set org_role_id = (select id from org_roles where org_id = %s and code = 'owner')
                    where org_id = %s and member_id = %s
                      and (
                        org_role_id is null
                        or org_role_id not in (select id from org_roles where org_id = %s)
                      )
                    """,
                    (org_id, org_id, owner["id"], org_id),
                )
                cursor.execute(
                    "select count(*) from org_roles where org_id = %s and code = 'member'",
                    (org_id,),
                )
                count = cursor.fetchone()[0]
    finally:
        connection.close()
    if count != 1:
        raise SystemExit("member role was not copied")
    status, _rows, message = request("GET", f"/work-items/approvers?org={org_id}", token=owner["token"])
    if status != 200:
        raise SystemExit(f"role seed failed status={status} message={message}")


def connect():
    import psycopg2

    return psycopg2.connect(database_url())


def seed_roles(org_id: str, people: dict[str, dict]) -> None:
    connection = connect()
    try:
        with connection:
            with connection.cursor() as cursor:
                for key, code in ROLE_CODES.items():
                    cursor.execute(
                        """
                        insert into org_roles (
                            id, org_id, code, name, description, built_in, sort_order, data_scope, permissions
                        )
                        select gen_random_uuid(), org_id, %s, %s, description, false, 40, data_scope, permissions
                        from org_roles
                        where org_id = %s and code = 'admin'
                          and not exists (
                            select 1 from org_roles existing
                            where existing.org_id = %s and existing.code = %s
                          )
                        """,
                        (code, people[key]["name"], org_id, org_id, code),
                    )
                    cursor.execute(
                        """
                        update users
                        set org_role_id = (
                            select id from org_roles where org_id = %s and code = %s
                        )
                        where org_id = %s and member_id = %s
                        """,
                        (org_id, code, org_id, people[key]["id"]),
                    )
    finally:
        connection.close()


def seed_department(org_id: str, owner: dict, people: dict[str, dict]) -> str:
    status, rows, message = request("GET", f"/orgs/{org_id}/departments", token=owner["token"])
    if status != 200 or not isinstance(rows, list) or not rows:
        raise SystemExit(f"departments failed status={status} message={message}")
    existing = next((row for row in rows if row.get("name") == "综合部"), None)
    if existing:
        department_id = str(existing["id"])
    else:
        root = next((row for row in rows if not row.get("parent_id")), rows[0])
        status, created, message = request(
            "POST",
            f"/orgs/{org_id}/departments",
            {"parent_id": root["id"], "name": "综合部"},
            owner["token"],
        )
        if status != 201 or not isinstance(created, dict):
            raise SystemExit(f"department create failed status={status} message={message}")
        department_id = str(created["id"])
    connection = connect()
    try:
        with connection:
            with connection.cursor() as cursor:
                cursor.execute(
                    """
                    update departments
                    set leader_user_id = (
                        select id from users where org_id = %s and member_id = %s
                    )
                    where id = %s
                    """,
                    (org_id, people["leader"]["id"], department_id),
                )
                for key in ("applicant", "probation", "short"):
                    cursor.execute(
                        """
                        update users set primary_department_id = %s
                        where org_id = %s and member_id = %s
                        """,
                        (department_id, org_id, people[key]["id"]),
                    )
                cursor.execute(
                    """
                    update users
                    set employment_status = 'probation', hire_date = %s
                    where org_id = %s and member_id = %s
                    """,
                    ("2026-07-01", org_id, people["probation"]["id"]),
                )
                cursor.execute(
                    """
                    insert into org_leave_balances (org_id, member_id, leave_kind, halves)
                    values (%s, %s, '年假', 4)
                    on conflict (org_id, member_id, leave_kind) do update set halves = 4
                    """,
                    (org_id, people["short"]["id"]),
                )
    finally:
        connection.close()
    return department_id


def reporting_line(owner: dict, org_id: str, member_id: str, manager_id: str) -> None:
    status, _payload, message = request(
        "POST",
        "/work-items/reporting-lines",
        {"org_id": org_id, "member_id": member_id, "manager_member_id": manager_id},
        owner["token"],
    )
    if status != 200:
        raise SystemExit(f"reporting line failed status={status} message={message}")


def main() -> None:
    if SESSION_FILE.is_file():
        print(f"seed reused {SESSION_FILE.name}")
        return
    if PASSWORD_FILE.is_file():
        password = PASSWORD_FILE.read_text().strip()
    else:
        password = secrets.token_hex(16)
        PASSWORD_FILE.write_text(password)
        os.chmod(PASSWORD_FILE, 0o600)
    people: dict[str, dict] = {}
    for index, (key, name) in enumerate(PEOPLE):
        phone = f"199000033{index:02d}"
        people[key] = register(phone, password, name, f"office-e2e-{index:02d}")
        print(f"registered {name}")
    org_id = ensure_org(people["owner"], "办公目录甲")
    empty_org_id = ensure_org(people["empty"], "空主管组织")
    raise_seats(org_id)
    ensure_roles_exist(people["owner"], org_id)
    for key, person in people.items():
        if key in ("owner", "empty"):
            continue
        status, orgs, message = request("GET", "/orgs", token=person["token"])
        if status == 200 and isinstance(orgs, list) and any(row.get("id") == org_id for row in orgs if isinstance(row, dict)):
            continue
        status, invite, message = request(
            "POST",
            "/invites",
            {"org_id": org_id, "phone": person["phone"]},
            people["owner"]["token"],
        )
        if status == 201 and isinstance(invite, dict) and invite.get("code"):
            code = str(invite["code"])
        elif "pending invite" in message:
            code = pending_invite_code(people["owner"], org_id, person["phone"])
            if not code:
                raise SystemExit(f"invite {person['name']} failed status={status} message={message}")
        else:
            raise SystemExit(f"invite {person['name']} failed status={status} message={message}")
        status, _accepted, message = request(
            "POST",
            f"/invite-codes/{code}/memberships",
            token=person["token"],
        )
        if status != 200:
            raise SystemExit(f"accept {person['name']} failed status={status} message={message}")
    seed_roles(org_id, people)
    department_id = seed_department(org_id, people["owner"], people)
    for key in ("applicant", "short", "probation"):
        reporting_line(people["owner"], org_id, people[key]["id"], people["manager"]["id"])
    stored = {
        "org_id": org_id,
        "empty_org_id": empty_org_id,
        "department_id": department_id,
        "people": people,
    }
    temporary = SESSION_FILE.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(stored))
    os.chmod(temporary, 0o600)
    temporary.replace(SESSION_FILE)
    os.chmod(SESSION_FILE, 0o600)
    print(f"seeded org={org_id} empty_org={empty_org_id} department={department_id} members={len(people)}")


if __name__ == "__main__":
    main()
