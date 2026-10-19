"""Report shared-crate drift across the three a3s crate lineages.

Lineages compared by default:
  aster   <repo>/crates
  api     <repo>/apps/api/crates
  cloud   <repo>/apps/cloud/crates

A crate is identified by its Cargo.toml package name. For every name
found in more than one lineage the report classifies it:

  SAME           same version and same content digest in every lineage
  CONTENT-DRIFT  same version, different content digest
  VERSION-DRIFT  different version across lineages

Names found in a single lineage are listed as UNIQUE(<lineage>).

Modes:
  python3 tests/crate_drift_report.py            print the report
  python3 tests/crate_drift_report.py --json     print machine-readable JSON
  ... --baseline tests/crate_drift_baseline.json fail when the drifted
      set grew relative to the checked-in baseline (shrink-only ratchet)
  ... --update-baseline <path>                   rewrite the baseline file

The digest covers every file under the crate except `target/` and VCS
directories, so any content difference is drift even when versions match.
"""

import argparse
import hashlib
import json
import sys
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

DEFAULT_LINEAGES = {
    "aster": ROOT / "crates",
    "api": ROOT / "apps" / "api" / "crates",
    "cloud": ROOT / "apps" / "cloud" / "crates",
}

SKIP_DIRS = {"target", ".git"}

WORSENED = "the drift ratchet only shrinks: update the baseline only when drift was removed"


def load_crate(dir_path: Path):
    """Return (name, version, digest) for a crate directory, or None."""
    manifest = dir_path / "Cargo.toml"
    if not manifest.is_file():
        return None
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as exc:
        raise SystemExit(f"fail-closed: {manifest}: {exc}")
    package = data.get("package")
    if not isinstance(package, dict):
        # Workspace-level manifests without [package] are not crates.
        return None
    name = package.get("name")
    version = package.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        return None
    hasher = hashlib.sha256()
    files = sorted(
        p for p in dir_path.rglob("*")
        if p.is_file()
        and not any(part in SKIP_DIRS for part in p.relative_to(dir_path).parts[:-1])
    )
    for path in files:
        rel = path.relative_to(dir_path).as_posix()
        hasher.update(rel.encode("utf-8"))
        hasher.update(b"\0")
        hasher.update(hashlib.sha256(path.read_bytes()).digest())
    return name, version, hasher.hexdigest()


def collect_lineage(root: Path):
    """Return (crates, duplicates) for every crate under root.

    crates maps name -> (version, digest); duplicates maps name -> the
    relative crate paths that share it. A duplicate is a finding, not an
    abort: it is reported through the ratchet like any other drift.
    """
    crates = {}
    duplicates = {}
    for manifest in sorted(root.rglob("Cargo.toml")):
        if any(part in SKIP_DIRS for part in manifest.relative_to(root).parts[:-1]):
            continue
        loaded = load_crate(manifest.parent)
        if loaded is None:
            continue
        name, version, digest = loaded
        if name in crates:
            duplicates.setdefault(name, []).append(
                manifest.parent.relative_to(root).as_posix()
            )
            continue
        crates[name] = (version, digest)
    return crates, duplicates


def classify(lineages, duplicates=None):
    """Return (drift: dict[name -> kind], unique: dict[name -> lineage])."""
    places = {}
    for lineage, crates in lineages.items():
        for name, meta in crates.items():
            places.setdefault(name, {})[lineage] = meta
    drift, unique = {}, {}
    for name, present in sorted(places.items()):
        if len(present) == 1:
            unique[name] = next(iter(present))
            continue
        versions = {version for version, _ in present.values()}
        digests = {digest for _, digest in present.values()}
        if len(versions) > 1:
            drift[name] = "VERSION-DRIFT"
        elif len(digests) > 1:
            drift[name] = "CONTENT-DRIFT"
        # same version and digest everywhere: not drift, not listed
    for lineage, dups in (duplicates or {}).items():
        for name in dups:
            drift[name] = f"DUPLICATE-{lineage}"
    return drift, unique


def build_report(roots):
    missing = sorted(label for label, root in roots.items() if not root.is_dir())
    if missing:
        # Fail closed: without every lineage the report silently weakens.
        raise SystemExit(
            "fail-closed: lineage roots missing: "
            + ", ".join(f"{label}={roots[label]}" for label in missing)
        )
    lineages = {}
    duplicates = {}
    for label, root in roots.items():
        crates, dups = collect_lineage(root)
        lineages[label] = crates
        if dups:
            duplicates[label] = dups
    drift, unique = classify(lineages, duplicates)
    shared = sum(
        1 for name in {n for c in lineages.values() for n in c}
        if name not in unique
    )
    return {
        "lineages": {label: str(root) for label, root in roots.items()},
        "counts": {
            "crates": {label: len(crates) for label, crates in lineages.items()},
            "shared": shared,
            "drifted": len(drift),
            "unique": len(unique),
        },
        "drift": drift,
        "unique": unique,
    }


def check_baseline(report, baseline_path: Path):
    """Fail when the drifted set grew. Shrinking requires --update-baseline."""
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    old = set(baseline.get("drift", {}))
    new = set(report["drift"])
    grew = sorted(new - old)
    if grew:
        print(f"drift ratchet grew ({len(grew)} new):", file=sys.stderr)
        for name in grew:
            print(f"  {name}: {report['drift'][name]}", file=sys.stderr)
        print(WORSENED, file=sys.stderr)
        return 1
    removed = sorted(old - new)
    if removed:
        print(
            "drift shrank; refresh the baseline with --update-baseline: "
            + ", ".join(removed)
        )
    return 0


class CrateDriftReportTest(unittest.TestCase):
    def make_lineages(self, tmp):
        roots = {"a": tmp / "a", "b": tmp / "b"}
        for root in roots.values():
            root.mkdir()
        return roots

    def write_crate(self, root, name, version, marker="m1"):
        crate = root / name.replace("/", "_")
        crate.mkdir(parents=True, exist_ok=True)
        (crate / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "{version}"\n', encoding="utf-8"
        )
        (crate / "src").mkdir(exist_ok=True)
        (crate / "src" / "lib.rs").write_text(f"// {marker}\n", encoding="utf-8")
        return crate

    def test_same_crate_is_not_drift(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            roots = self.make_lineages(tmp)
            for root in roots.values():
                self.write_crate(root, "a3s-x", "1.0.0")
            collected = {
                label: collect_lineage(root)[0] for label, root in roots.items()
            }
            # Guard against a vacuous pass: the crates must be found.
            self.assertEqual(
                {label: sorted(crates) for label, crates in collected.items()},
                {"a": ["a3s-x"], "b": ["a3s-x"]},
            )
            drift, unique = classify(collected)
            self.assertEqual(drift, {})
            self.assertEqual(unique, {})

    def test_content_and_version_drift_and_unique(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            roots = self.make_lineages(tmp)
            self.write_crate(roots["a"], "a3s-same", "1.0.0")
            self.write_crate(roots["b"], "a3s-same", "1.0.0")
            self.write_crate(roots["a"], "a3s-content", "2.0.0", marker="a")
            self.write_crate(roots["b"], "a3s-content", "2.0.0", marker="b")
            self.write_crate(roots["a"], "a3s-version", "1.0.0")
            self.write_crate(roots["b"], "a3s-version", "1.1.0")
            self.write_crate(roots["a"], "a3s-only-a", "0.1.0")
            drift, unique = classify(
                {label: collect_lineage(root)[0] for label, root in roots.items()}
            )
            self.assertEqual(
                drift,
                {"a3s-content": "CONTENT-DRIFT", "a3s-version": "VERSION-DRIFT"},
            )
            self.assertEqual(unique, {"a3s-only-a": "a"})

    def test_within_lineage_duplicate_is_a_finding(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            roots = self.make_lineages(tmp)
            self.write_crate(roots["a"], "a3s-x", "1.0.0")
            self.write_crate(roots["a"] / "vendored", "a3s-x", "1.0.0")
            self.write_crate(roots["b"], "a3s-x", "1.0.0")
            crates, dups = collect_lineage(roots["a"])
            self.assertEqual(list(crates), ["a3s-x"])
            self.assertEqual(len(dups["a3s-x"]), 1)
            drift, _ = classify({"a": crates, "b": collect_lineage(roots["b"])[0]}, {"a": dups})
            self.assertEqual(drift, {"a3s-x": "DUPLICATE-a"})

    def test_baseline_ratchet_only_shrinks(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            baseline = tmp / "baseline.json"
            baseline.write_text(
                json.dumps({"drift": {"a3s-old": "CONTENT-DRIFT"}}), encoding="utf-8"
            )
            report = {"drift": {"a3s-old": "CONTENT-DRIFT"}}
            self.assertEqual(check_baseline(report, baseline), 0)
            report = {
                "drift": {
                    "a3s-old": "CONTENT-DRIFT",
                    "a3s-new": "VERSION-DRIFT",
                }
            }
            self.assertEqual(check_baseline(report, baseline), 1)
            shrunk = {"drift": {}}
            self.assertEqual(check_baseline(shrunk, baseline), 0)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print JSON only")
    parser.add_argument("--baseline", type=Path, help="shrink-only drift ratchet file")
    parser.add_argument(
        "--update-baseline",
        type=Path,
        help="write the current drift set to this file and exit",
    )
    args = parser.parse_args(argv)

    if args.update_baseline:
        report = build_report(DEFAULT_LINEAGES)
        args.update_baseline.write_text(
            json.dumps(
                {
                    "counts": report["counts"],
                    "drift": report["drift"],
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        print(f"baseline written: {args.update_baseline}")
        return 0

    report = build_report(DEFAULT_LINEAGES)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        counts = report["counts"]
        print("a3s crate lineages:", ", ".join(
            f"{label}={counts['crates'][label]}" for label in report["lineages"]
        ))
        print(
            f"shared={counts['shared']} drifted={counts['drifted']} "
            f"unique={counts['unique']}"
        )
        for name, kind in sorted(report["drift"].items()):
            print(f"  DRIFT {name}: {kind}")
    if args.baseline:
        return check_baseline(report, args.baseline)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
