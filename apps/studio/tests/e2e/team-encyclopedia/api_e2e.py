#!/usr/bin/env python3
"""Team encyclopedia retrieval: live API end-to-end cases and golden evaluation.

Live stack: a3s-api :3000 (Tilt resource `api`) with pgvector, gateway :8080,
Studio sessions `towork-cua-teamadmin` (org admin) and `toyou-fork2` (member
of the R&D department). Creates a fresh org per run, seeds the corpus, then runs
E01-E05, E07-E10, E15, observes E06/E11 live, and evaluates the golden set for
the old scorer (rollout off) and the hybrid answer (fast, precise).

    python3 api_e2e.py [results.json]

Tokens and phone numbers never reach the log or the results file.
"""

from __future__ import annotations

import json
import statistics
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "team-workbench"))
from two_account_gui import API, api, log, session  # noqa: E402

import corpus  # noqa: E402

ADMIN_APP = "towork-cua-teamadmin"
MEMBER_APP = "toyou-fork2"
INDEX = "/v1/api/org/encyclopedia-search"
RESULTS: dict = {"cases": {}, "evaluation": {}}


def ok(case: str, passed: bool, **detail) -> None:
    RESULTS["cases"][case] = {"passed": passed, **detail}
    log(f"{case} {'PASS' if passed else 'FAIL'} {json.dumps(detail, ensure_ascii=False)[:400]}")


def psql(sql: str) -> list[str]:
    out = subprocess.run(
        ["docker", "exec", "a3s-postgres", "sh", "-c", f'psql -U "$POSTGRES_USER" -d a3s_api -Atc "{sql}"'],
        capture_output=True, text=True, check=True,
    )
    return [line for line in out.stdout.splitlines() if line]


def multipart(token: str, method: str, path: str, name: str, text: str, fields: dict[str, str]) -> dict:
    boundary = uuid.uuid4().hex
    parts = [
        f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode()
        for key, value in fields.items()
    ]
    parts.append(
        f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\n'
        "Content-Type: text/markdown\r\n\r\n".encode() + text.encode() + b"\r\n"
    )
    parts.append(f"--{boundary}--\r\n".encode())
    request = urllib.request.Request(
        API + path, data=b"".join(parts), method=method,
        headers={"authorization": f"Bearer {token}", "content-type": f"multipart/form-data; boundary={boundary}"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        body = json.loads(response.read())
    return body.get("data", body)


def search(token: str, org: str, query: str, mode: str | None = "fast") -> tuple[dict, int]:
    body = {"query": query} if mode is None else {"query": query, "mode": mode}
    started = time.perf_counter()
    answer = api(token, "POST", f"/v1/api/org/drive/{org}/encyclopedia-passages", body)
    return answer, int((time.perf_counter() - started) * 1000)


def headings(answer: dict) -> list[str]:
    # The heading keeps its markdown marker (`## 标题`); Studio strips it the same way.
    return [hit.get("heading", "").lstrip("#").strip() for hit in answer.get("passages", [])]


def status_of(token: str, method: str, path: str, body: dict) -> tuple[int, str]:
    request = urllib.request.Request(
        API + path, data=json.dumps(body).encode(), method=method,
        headers={"authorization": f"Bearer {token}", "content-type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status, response.read().decode()[:300]
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode(errors="replace")[:300]


# ---------------------------------------------------------------- setup


def setup() -> dict:
    admin, member = session(ADMIN_APP), session(MEMBER_APP)
    if admin["member_id"] == member["member_id"]:
        raise RuntimeError("admin and member sessions are the same account")
    name = f"enc-search-{uuid.uuid4().hex[:6]}"
    org = api(admin["token"], "POST", "/v1/api/org/members/orgs", {"name": name, "industry": "software", "scale": "1-9"})["id"]
    log(f"created org {name} id={org}")
    depts = api(admin["token"], "GET", f"/v1/api/org/members/orgs/{org}/departments")
    root = next(d for d in depts if d.get("parent_id") is None)["id"]
    rnd = api(admin["token"], "POST", f"/v1/api/org/members/orgs/{org}/departments", {"parent_id": root, "name": "研发部"})["id"]
    finance = api(admin["token"], "POST", f"/v1/api/org/members/orgs/{org}/departments", {"parent_id": root, "name": "财务部"})["id"]
    # Seats are an operator fact (same stand-in as team-admin/cua_p1.py).
    psql(f"insert into org_subscriptions (org_id) values ('{org}') on conflict (org_id) do nothing")
    psql(f"update org_subscriptions set plan_id = 'team', seats = 3 where org_id = '{org}' and seats < 3")
    invite = api(admin["token"], "POST", "/v1/api/org/members/invites", {"org_id": org, "phone": member.get("phone") or ""})
    api(member["token"], "POST", f"/v1/api/org/members/invite-codes/{invite['code']}/memberships")
    rows = api(admin["token"], "GET", f"/v1/api/org/members/orgs/{org}/members")
    row = next(r for r in rows if str(r.get("member_id")) == member["member_id"])
    if row.get("admin"):
        raise RuntimeError("the member account must not be an org admin")
    api(admin["token"], "PUT", f"/v1/api/org/members/orgs/{org}/members/{row['id']}/department", {"department_id": rnd})
    log(f"member joined org in 研发部 (admin={row.get('admin')})")
    return {"name": name, "org": org, "admin": admin["token"], "member": member["token"],
            "depts": {"all": None, "rnd": rnd, "finance": finance}}


def upload_corpus(ctx: dict) -> None:
    org, token = ctx["org"], ctx["admin"]
    folder = api(token, "POST", f"/v1/api/org/drive/{org}/nodes", {"name": "归档", "parent_id": None, "dept_id": None})["id"]
    ctx["folder"] = folder
    files: dict[str, str] = {}
    docs = corpus.DOCS + corpus.filler() + [corpus.REGENERATED, corpus.DRAFT]
    for name, dept, text in docs:
        fields = {"dept_id": ctx["depts"][dept]} if ctx["depts"][dept] else {}
        files[name] = multipart(token, "POST", f"/v1/api/org/drive/{org}/files", name, text, fields)["id"]
    name, _, text = corpus.EXCLUDED
    files[name] = multipart(token, "POST", f"/v1/api/org/drive/{org}/files", name, text, {"node_id": folder})["id"]
    ctx["files"] = files
    log(f"uploaded {len(files)} files")
    settle_articles(ctx)


def settle_articles(ctx: dict, seconds: int = 240) -> None:
    """Publish every compiled article except the draft fixture, which is returned to draft.

    The list hides drafts, so the draft fixture counts once it has been returned.
    """
    org, token = ctx["org"], ctx["admin"]
    by_id = {fid: name for name, fid in ctx["files"].items()}
    deadline = time.time() + seconds
    while True:
        articles = api(token, "GET", f"/v1/api/org/drive/{org}/encyclopedia")
        covered: dict[str, dict] = {}
        for article in articles:
            for citation in article.get("citations", []):
                if citation.get("file_id") in by_id:
                    covered[by_id[citation["file_id"]]] = article
        for name, article in covered.items():
            if name == corpus.DRAFT[0]:
                if article["status"] != "draft":
                    api(token, "POST", f"/v1/api/org/drive/{org}/encyclopedia/{article['id']}/return")
                ctx["draft_returned"] = True
            elif article["status"] != "published":
                api(token, "POST", f"/v1/api/org/drive/{org}/encyclopedia/{article['id']}/publish")
        missing = sorted(set(by_id.values()) - set(covered) - {corpus.DRAFT[0]})
        if not missing and ctx.get("draft_returned") and all(
            a["status"] == "published" for n, a in covered.items() if n != corpus.DRAFT[0]
        ):
            log(f"articles settled: {len(covered)} files cited")
            return
        if time.time() > deadline:
            raise RuntimeError(f"articles did not settle; missing {missing[:5]}")
        time.sleep(3)


def progress(ctx: dict) -> dict:
    deadline = time.time() + 300
    while True:
        try:
            return api(ctx["admin"], "GET", f"{INDEX}/{ctx['org']}/index")
        except (RuntimeError, OSError) as error:
            # The gateway answers 5xx while the API restarts (E07, or another session's edit).
            if time.time() > deadline or (isinstance(error, RuntimeError) and " HTTP 5" not in str(error)):
                raise
            time.sleep(5)


def rollout(ctx: dict, value: str) -> None:
    api(ctx["admin"], "PUT", f"{INDEX}/{ctx['org']}/rollout", {"rollout": value})


def wait_indexed(ctx: dict, seconds: int = 1200) -> dict:
    deadline = time.time() + seconds
    while time.time() < deadline:
        state = progress(ctx)["progress"]
        if state["queued"] == 0 and state["vectors_pending"] == 0 and state["indexed"] > 0:
            return state
        time.sleep(5)
    raise RuntimeError(f"index did not drain: {progress(ctx)}")


# ---------------------------------------------------------------- evaluation


def evaluate(ctx: dict, label: str, mode: str) -> dict:
    rows = []
    for gid, kind, asker, query, relevant in corpus.GOLDEN:
        answer, ms = search(ctx[asker], ctx["org"], query, mode)
        got = headings(answer)
        top5 = got[:5]
        recall = sum(1 for h in relevant if h in top5) / len(relevant)
        rank = next((i + 1 for i, h in enumerate(got) if h in relevant), None)
        rows.append({"id": gid, "kind": kind, "recall5": recall, "rr": 1 / rank if rank else 0.0,
                     "empty": not got, "ms": ms, "degraded": answer.get("degraded", []), "top5": top5})
    summary = summarize(rows)
    RESULTS["evaluation"][label] = {"summary": summary, "rows": rows}
    log(f"evaluation {label}: {json.dumps(summary['all'], ensure_ascii=False)}")
    return summary


def summarize(rows: list[dict]) -> dict:
    def stats(subset: list[dict]) -> dict:
        ms = sorted(r["ms"] for r in subset)
        return {
            "n": len(subset),
            "recall5": round(statistics.mean(r["recall5"] for r in subset), 3),
            "mrr": round(statistics.mean(r["rr"] for r in subset), 3),
            "no_result": round(sum(r["empty"] for r in subset) / len(subset), 3),
            "p50_ms": ms[len(ms) // 2],
            "p95_ms": ms[min(len(ms) - 1, int(round(0.95 * (len(ms) - 1))))],
            "degraded": sum(1 for r in subset if r["degraded"]),
        }
    out = {"all": stats(rows)}
    for kind in sorted({r["kind"] for r in rows}):
        out[kind] = stats([r for r in rows if r["kind"] == kind])
    return out


# ---------------------------------------------------------------- cases


def e07_kill_mid_backfill(ctx: dict) -> None:
    """Kill the API while jobs are in flight; after restart every passage is indexed once."""
    deadline = time.time() + 600
    while time.time() < deadline:
        state = progress(ctx)["progress"]
        if state["indexed"] >= 10 and state["queued"] > 10:
            break
        time.sleep(2)
    before = progress(ctx)["progress"]
    pid = subprocess.run(["lsof", "-tiTCP:3000", "-sTCP:LISTEN"], capture_output=True, text=True).stdout.split()
    if not pid:
        raise RuntimeError("no API listener on :3000")
    subprocess.run(["kill", "-9", pid[0]], check=True)
    log(f"killed api pid={pid[0]} mid-backfill at {before}")
    subprocess.run(["tilt", "trigger", "api"], check=True, capture_output=True)
    for _ in range(300):
        try:
            urllib.request.urlopen(API + "/health", timeout=2)
            break
        except Exception:
            time.sleep(2)
    final = wait_indexed(ctx)
    org = ctx["org"]
    jobs = dict(row.split("|") for row in psql(
        f"select status, count(*) from encyclopedia_search_jobs where org_id = '{org}' group by 1"))
    duplicates = psql(
        f"select count(*) - count(distinct (passage_id, ordinal)) from encyclopedia_search_events where org_id = '{org}'")[0]
    reclaimed = psql(f"select count(*) from encyclopedia_search_jobs where org_id = '{org}' and attempts > 1")[0]
    passed = set(jobs) == {"done"} and duplicates == "0" and final["failed"] == 0
    ok("E07", passed, before=before, after=final, jobs=jobs, duplicate_events=int(duplicates),
       reclaimed_jobs=int(reclaimed))


def e08_rollouts(ctx: dict, off_answer: dict) -> None:
    org, token, query = ctx["org"], ctx["admin"], "出差回来多久要交票"
    rollout(ctx, "shadow")
    shadow_before = int(psql(f"select count(*) from encyclopedia_search_shadow_runs where org_id = '{org}'")[0])
    shadow, _ = search(token, org, query)
    time.sleep(3)
    shadow_after = int(psql(f"select count(*) from encyclopedia_search_shadow_runs where org_id = '{org}'")[0])
    rollout(ctx, "on")
    on, _ = search(token, org, query)
    passed = (
        [h["id"] for h in shadow["passages"]] == [h["id"] for h in off_answer["passages"]]
        and shadow_after == shadow_before + 1
        and "差旅票据提交时限" in headings(on)[:5]
    )
    ok("E08", passed, off=headings(off_answer)[:5], shadow_rows=shadow_after - shadow_before, on=headings(on)[:5])


def e01_paraphrase(ctx: dict, off_answer: dict) -> None:
    answer, ms = search(ctx["admin"], ctx["org"], "出差回来多久要交票")
    passages = progress(ctx)["progress"]["indexed"]
    top5 = headings(answer)[:5]
    ok("E01", passages > 200 and "差旅票据提交时限" in top5, indexed=passages, fast_top5=top5,
       off_top5=headings(off_answer)[:5], ms=ms)


def e02_cross_department(ctx: dict) -> None:
    probe = corpus.HIDDEN_PROBES[0]
    member, _ = search(ctx["member"], ctx["org"], probe)
    nothing, _ = search(ctx["member"], ctx["org"], "火星基地的氧气配额是多少")
    admin, _ = search(ctx["admin"], ctx["org"], probe)
    leaked = [h for h in headings(member) if h in corpus.HIDDEN_FROM_MEMBER]
    same_shape = set(member) == set(nothing)
    ok("E02", not leaked and "基本户开户行" in headings(admin)[:5] and same_shape,
       member=headings(member)[:5], admin=headings(admin)[:3], leaked=leaked)


def e03_regenerated(ctx: dict) -> None:
    org, token = ctx["org"], ctx["admin"]
    name, _, _ = corpus.REGENERATED
    multipart(token, "PUT", f"/v1/api/org/drive/{org}/files/{ctx['files'][name]}/content", name, corpus.REGENERATED_V2, {})
    deadline, stale, fresh = time.time() + 300, None, None
    while time.time() < deadline:
        settle_articles(ctx, seconds=120)
        answer, _ = search(token, org, "夜间值班电话是多少")
        bodies = " ".join(h.get("body", "") for h in answer["passages"])
        stale = "400-800-1234" in bodies
        fresh = "400-900-5678" in bodies
        if fresh and not stale:
            break
        time.sleep(5)
    ok("E03", fresh is True and stale is False, stale_generation_returned=stale, new_generation_found=fresh)


def e04_folder_excluded(ctx: dict) -> None:
    org, token, query = ctx["org"], ctx["admin"], "纸质合同原件要保管多少年"
    before, _ = search(token, org, query)
    api(token, "PUT", f"/v1/api/org/drive/{org}/nodes/{ctx['folder']}/encyclopedia", {"excluded": True})
    after, _ = search(token, org, query)
    ok("E04", "纸质档案保管期限" in headings(before)[:5] and "纸质档案保管期限" not in headings(after),
       before=headings(before)[:3], after=headings(after)[:3])


def e05_draft(ctx: dict) -> None:
    seen = []
    for mode in ("fast", "precise"):
        answer, _ = search(ctx["admin"], ctx["org"], "可以带宠物来办公室吗", mode)
        seen += [h for h in headings(answer) if h == "宠物进入办公区"]
    ok("E05", not seen, returned=seen)


def e09_multi_hop(ctx: dict) -> None:
    answer, ms = search(ctx["admin"], ctx["org"], "张三那边的系统部署在哪家云上")
    names = headings(answer)[:5]
    paths = dict(zip(names, (h.get("path", []) for h in answer["passages"][:5])))
    bridged = any("星河项目" in "".join(p) or "张三" in "".join(p) for p in paths.values())
    ok("E09", "星河项目负责人" in names and "星河项目云资源" in names and bridged, top5=names, paths=paths, ms=ms)


def e10_entity_side_channel(ctx: dict) -> None:
    leaks, paths = [], []
    for query in corpus.HIDDEN_PROBES + ["张三跟进哪笔钱"]:
        answer, _ = search(ctx["member"], ctx["org"], query, "precise")
        leaks += [h for h in headings(answer) if h in corpus.HIDDEN_FROM_MEMBER]
        for hit in answer["passages"]:
            paths += hit.get("path", [])
            if any(word in hit.get("body", "") for word in ("尾款", "回款", "开户行", "调薪")):
                leaks.append(hit["heading"])
    ok("E10", not leaks and not any(word in "".join(paths) for word in ("回款", "尾款")), leaks=leaks, paths=sorted(set(paths)))


def e15_invalid_input(ctx: dict) -> None:
    path = f"/v1/api/org/drive/{ctx['org']}/encyclopedia-passages"
    bad_mode = status_of(ctx["admin"], "POST", path, {"query": "报销", "mode": "deep"})
    empty = status_of(ctx["admin"], "POST", path, {"query": "   "})
    extra = status_of(ctx["admin"], "POST", path, {"query": "报销", "limit": 99})
    ok("E15", bad_mode[0] == 400 and "fast or precise" in bad_mode[1] and empty[0] == 400
       and "query is empty" in empty[1] and extra[0] == 400,
       bad_mode=bad_mode, empty=empty, unknown_field=extra[0])


def e06_e11_observed() -> None:
    for case, mode, reasons in (("E06", "fast", {"embedding_unavailable"}),
                                ("E11", "precise", {"rank_model_unavailable"})):
        rows = RESULTS["evaluation"].get(mode, {}).get("rows", [])
        degraded = [r for r in rows if r["degraded"]]
        unexpected = [r for r in degraded if not set(r["degraded"]) <= reasons]
        ok(case, not unexpected, live_degraded=len(degraded), unexpected=unexpected,
           fault_injection="dual-adapter campaign over live PostgreSQL (scenario_tests vectors/precise)")


def evaluate_only(org: str, label: str, out: Path) -> int:
    """Re-rank an already seeded org, e.g. after restarting the API with graph ranking off."""
    ctx = {"org": org, "admin": session(ADMIN_APP)["token"], "member": session(MEMBER_APP)["token"]}
    for mode in ("fast", "precise"):
        evaluate(ctx, f"{label}-{mode}", mode)
    out.write_text(json.dumps(RESULTS["evaluation"], ensure_ascii=False, indent=2))
    return 0


def main() -> int:
    if len(sys.argv) == 5 and sys.argv[1] == "--evaluate":
        return evaluate_only(sys.argv[2], sys.argv[3], Path(sys.argv[4]))
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/team-encyclopedia-api-e2e.json")
    ctx = setup()
    RESULTS["org"] = {"name": ctx["name"], "id": ctx["org"]}
    upload_corpus(ctx)
    rollout(ctx, "off")
    off_probe, _ = search(ctx["admin"], ctx["org"], "出差回来多久要交票")
    evaluate(ctx, "off", "fast")
    rollout(ctx, "on")
    e07_kill_mid_backfill(ctx)
    RESULTS["index"] = progress(ctx)
    e08_rollouts(ctx, off_probe)
    evaluate(ctx, "fast", "fast")
    evaluate(ctx, "precise", "precise")
    for case in (lambda: e01_paraphrase(ctx, off_probe), lambda: e02_cross_department(ctx),
                 lambda: e05_draft(ctx), lambda: e09_multi_hop(ctx), lambda: e10_entity_side_channel(ctx),
                 lambda: e15_invalid_input(ctx), lambda: e03_regenerated(ctx), lambda: e04_folder_excluded(ctx),
                 e06_e11_observed):
        try:
            case()
        except Exception as error:  # one broken case must not hide the others
            log(f"case error {error}")
            RESULTS["cases"].setdefault(f"error-{len(RESULTS['cases'])}", {"passed": False, "error": str(error)[:300]})
    out.write_text(json.dumps(RESULTS, ensure_ascii=False, indent=2))
    failed = [case for case, row in RESULTS["cases"].items() if not row["passed"]]
    log(f"results -> {out}; failed={failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
