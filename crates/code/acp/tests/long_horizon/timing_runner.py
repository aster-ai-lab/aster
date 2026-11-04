"""Passive phase timing for the existing qualification commands."""

from contextlib import contextmanager
import json
import os
from pathlib import Path
import subprocess
import time


class Recorder:
    def __init__(self, path):
        self.path = Path(path)
        self.origin_ns = time.monotonic_ns()
        self.wall_ns = time.time_ns()
        self.sequence = 0
        self.parents = []
        self.closed = False
        self.event("open", "timing_open")

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def close(self):
        if not self.closed:
            self.event("close", "timing_close")
            self.closed = True

    def event(self, event, phase, *, span=None, at_ns=None, duration_ns=None, subject=None):
        now = time.monotonic_ns() if at_ns is None else at_ns
        self.sequence += 1
        row = {
            "schema": "aster.test.phase-timing.v1",
            "event": event,
            "pid": os.getpid(),
            "sequence": self.sequence,
            "span": self.sequence if span is None else span,
            "parent_span": self.parents[-1] if self.parents else None,
            "phase": phase,
            "elapsed_us": (now - self.origin_ns) // 1000,
            "clock_origin_unix_us": self.wall_ns // 1000,
            "unix_time_us": (self.wall_ns + now - self.origin_ns) // 1000,
            "duration_us": None if duration_ns is None else duration_ns // 1000,
            "subject": subject,
            "dropped_records": 0,
        }
        with self.path.open("ab", buffering=0) as stream:
            stream.write((json.dumps(row, separators=(",", ":")) + "\n").encode("utf-8"))
        return row["span"]

    @contextmanager
    def phase(self, name, subject=None):
        start = time.monotonic_ns()
        span = self.event("begin", name, at_ns=start, subject=subject)
        self.parents.append(span)
        try:
            yield span
        finally:
            end = time.monotonic_ns()
            self.parents.pop()
            self.event("end", name, span=span, at_ns=end, duration_ns=end - start, subject=subject)

    def run_logged(self, command, *, cwd, env, log, phase):
        """Retain stdout/stderr bytes; record the first observed output without parsing it."""
        with self.phase(phase):
            with Path(log).open("wb", buffering=0) as output:
                with self.phase("process_spawn"):
                    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
                self.event("point", "process_started", subject={"process": process.pid})
                first = True
                try:
                    while True:
                        chunk = os.read(process.stdout.fileno(), 65536)
                        if not chunk:
                            break
                        if first:
                            self.event("point", "process_first_output", subject={"process": process.pid})
                            first = False
                        output.write(chunk)
                    code = process.wait()
                except BaseException:
                    process.kill()
                    process.wait()
                    raise
                finally:
                    process.stdout.close()
                    self.event("point", "process_exit", subject={"process": process.pid, "exit_code": process.returncode})
                return subprocess.CompletedProcess(command, code)

    def run_capture(self, command, *, cwd, env=None, phase, timeout=None):
        """Preserve the caller's existing capture and timeout semantics."""
        with self.phase(phase):
            with self.phase("process_spawn"):
                process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.event("point", "process_started", subject={"process": process.pid})
            try:
                stdout, stderr = process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
                self.event("point", "process_timeout", subject={"process": process.pid})
                raise
            except BaseException:
                process.kill()
                process.wait()
                raise
            finally:
                self.event("point", "process_exit", subject={"process": process.pid, "exit_code": process.returncode})
            return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def summarize(paths):
    """Report inclusive spans and missing evidence; overlapping durations are not added."""
    begins = {}
    spans = []
    points = []
    errors = []
    streams = {}
    for path in paths:
        if Path(path).stat().st_size > 32 * 1024 * 1024:
            errors.append({"kind": "input_too_large", "file": str(path)})
            continue
        for number, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), 1):
            try:
                row = json.loads(line)
                if row["schema"] != "aster.test.phase-timing.v1":
                    raise ValueError("unsupported timing schema")
                for field in ["pid", "sequence", "span", "elapsed_us", "unix_time_us", "clock_origin_unix_us", "dropped_records"]:
                    if type(row[field]) is not int or row[field] < 0:
                        raise ValueError("invalid timing counter")
                if not isinstance(row["phase"], str) or len(row["phase"]) > 128:
                    raise ValueError("invalid phase name")
                if row["event"] not in ["begin", "end", "open", "close", "point"]:
                    raise ValueError("invalid event")
                if row["event"] == "end" and (type(row["duration_us"]) is not int or row["duration_us"] < 0):
                    raise ValueError("invalid duration")
                key = (row["pid"], row["clock_origin_unix_us"], row["span"])
                stream = streams.setdefault(key[:2], {"open": 0, "close": 0, "dropped": 0})
                stream["dropped"] = max(stream["dropped"], row["dropped_records"])
                if row["event"] in ["open", "close"]:
                    stream[row["event"]] += 1
                if row["event"] == "begin":
                    if key in begins:
                        errors.append({"kind": "duplicate_begin", "pid": key[0], "span": key[2]})
                    begins[key] = row
                elif row["event"] == "end":
                    begin = begins.pop(key, None)
                    if begin is None:
                        errors.append({"kind": "end_without_begin", "pid": key[0], "span": key[2]})
                    elif begin["phase"] != row["phase"] or row["elapsed_us"] < begin["elapsed_us"]:
                        errors.append({"kind": "invalid_interval", "pid": key[0], "span": key[2]})
                    else:
                        spans.append({"pid": row["pid"], "span": row["span"], "phase": row["phase"], "start_us": begin["unix_time_us"], "end_us": row["unix_time_us"], "duration_us": row["duration_us"], "subject": row["subject"], "clock_origin_unix_us": row["clock_origin_unix_us"]})
                else:
                    points.append(row)
                    if row["phase"].endswith(("_missing_start", "_unfinished")):
                        errors.append({"kind": "incomplete_lifecycle", "phase": row["phase"], "pid": row["pid"]})
            except (KeyError, TypeError, ValueError):
                errors.append({"kind": "invalid_record", "file": str(path), "line": number})
    errors.extend({"kind": "missing_end", "pid": key[0], "span": key[2], "phase": row["phase"]} for key, row in begins.items())
    for (pid, origin), stream in streams.items():
        if stream["open"] != 1 or stream["close"] != 1:
            errors.append({"kind": "stream_not_closed", "pid": pid, "clock_origin_unix_us": origin})
        if stream["dropped"]:
            errors.append({"kind": "dropped_records", "pid": pid, "count": stream["dropped"]})
    phases = {}
    for phase in sorted({span["phase"] for span in spans}):
        group = [span for span in spans if span["phase"] == phase]
        inclusive = sum(span['duration_us'] for span in group)
        union = interval_union((span['start_us'], span['end_us']) for span in group)
        concurrent = peak = 0
        for _, delta in sorted(event for span in group for event in [(span['start_us'],1),(span['end_us'],-1)] if span['end_us'] > span['start_us']):
            concurrent += delta
            peak = max(peak, concurrent)
        phases[phase] = {"count": len(group), "inclusive_us": inclusive, "union_us": union, "overlap_us": max(0,inclusive-union), "max_parallel":peak}
    requests = []
    for request in (span for span in spans if span['phase'] == 'model_request'):
        def matches(row):
            return row['pid'] == request['pid'] and row['clock_origin_unix_us'] == request['clock_origin_unix_us'] and row['subject'] == request['subject']
        header = next((span for span in spans if span['phase'] == 'model_headers' and matches(span)), None)
        first = next((point for point in points if point['phase'] == 'model_first_byte' and matches(point)), None)
        requests.append({'pid':request['pid'],'request':request['subject'],'duration_us':request['duration_us'],'headers_us':header['duration_us'] if header else None,'first_body_byte_us':first['unix_time_us']-request['start_us'] if first else None,'missing_note':None if header and first else 'Buffered HTTP has no separate header/first-byte observation; transport errors or empty streams may also omit them. Never use zero.'})
    return {"schema": "aster.test.phase-summary.v1", "complete": bool(streams) and not errors, "errors": errors, "spans": spans, "points": points, "phases": phases, "model_requests":requests, "covered_union_us": interval_union((span["start_us"], span["end_us"]) for span in spans), "inclusive_sum_us": sum(span["duration_us"] for span in spans), "clock_note": "Durations use a monotonic clock; UTC anchors correlate processes. Nested and parallel spans overlap. Scope exit does not imply task success."}


def interval_union(intervals):
    intervals = sorted(intervals)
    union = 0
    if intervals:
        start, end = intervals[0]
        for next_start, next_end in intervals[1:]:
            if next_start <= end:
                end = max(end, next_end)
            else:
                union += end - start
                start, end = next_start, next_end
        union += end - start
    return union


def reconcile(summary, process_pid):
    """Partition one tester's lifetime; keep pre-instrumentation and uncovered time visible."""
    starts = [row for row in summary["points"] if row["phase"] == "process_started" and row["subject"]["process"] == process_pid]
    ends = [row for row in summary["points"] if row["phase"] == "process_exit" and row["subject"]["process"] == process_pid]
    if len(starts) != 1 or len(ends) != 1:
        return {"complete": False, "reason": "missing process lifetime"}
    start, end = starts[0]["unix_time_us"], ends[0]["unix_time_us"]
    native = [span for span in summary["spans"] if span["pid"] == process_pid]
    priority = [
        ("task", {"prompt", "tui_engine_run"}),
        ("cleanup", {"driver_close", "relay_close", "tui_proxy_cleanup"}),
        ("preparation", {"campaign_prepare", "campaign_spawn", "initialize", "session_new", "session_load", "tui_suite_check"}),
        ("verification", {"identity_recheck", "artifact_read", "public_evidence_write", "oracle", "fact_read", "result_write"}),
        ("fixture", {"fixture_setup", "feed_publish", "goal_state"}),
    ]
    clipped = [(max(start, span["start_us"]), min(end, span["end_us"]), span["phase"]) for span in native if span["end_us"] > start and span["start_us"] < end]
    boundaries = sorted({start, end, *(point for a, b, _ in clipped for point in [a, b])})
    totals = {name: 0 for name, _ in priority}
    totals["other_instrumented"] = 0
    totals["unattributed"] = 0
    for a, b in zip(boundaries, boundaries[1:]):
        active = {phase for x, y, phase in clipped if x <= a and y >= b}
        category = next((name for name, phases in priority if phases & active), "other_instrumented" if active else "unattributed")
        totals[category] += b - a
    wall = end - start
    return {"complete": summary["complete"], "process_pid": process_pid, "wall_us": wall, "partition_us": totals, "sum_us": sum(totals.values()), "unattributed_percent": totals["unattributed"] / wall * 100 if wall else None, "note": "Exclusive wall-time partition; model/tools/subagents/compaction overlap within task time. Unattributed includes native loading, uninstrumented fixture work, writer flush and scheduling. It is never treated as zero."}


def compare(current, baseline):
    rows = {}
    for phase in sorted(current.keys() | baseline.keys()):
        before, after = baseline.get(phase), current.get(phase)
        delta = after - before if before is not None and after is not None else None
        rows[phase] = {"before": before, "after": after, "delta": delta, "percent": delta / before * 100 if delta is not None and before else None}
    return rows
