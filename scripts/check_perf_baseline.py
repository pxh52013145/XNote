#!/usr/bin/env python3
import argparse
import json
import re
import subprocess
import sys
import statistics
from pathlib import Path
from datetime import datetime, timezone


def parse_metrics(output: str) -> dict:
    metrics = {}
    for line in output.splitlines():
        line = line.strip()
        match = re.match(r"^([a-zA-Z0-9_]+):\s*(.+)$", line)
        if match:
            key = match.group(1)
            value = match.group(2)
            if value.isdigit():
                metrics[key] = int(value)
            else:
                metrics[key] = value
            continue

        match = re.match(r"^([a-zA-Z0-9_]+)\s*:\s*(\d+)$", line)
        if match:
            metrics[match.group(1)] = int(match.group(2))

    return metrics


def run_xtask_perf(vault_path: str, query: str, iterations: int) -> dict:
    cmd = [
        "cargo",
        "run",
        "-p",
        "xtask",
        "--",
        "perf",
        "--path",
        vault_path,
        "--query",
        query,
        "--iterations",
        str(iterations),
    ]
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
    if proc.returncode != 0:
        print(combined)
        raise RuntimeError("xtask perf failed")

    return parse_metrics(combined)


def aggregate_metrics(runs: list[dict]) -> dict:
    if not runs:
        return {}

    aggregated: dict = {}
    first = runs[0]
    keys = set()
    for run in runs:
        keys.update(run.keys())

    for key in sorted(keys):
        values = [run.get(key) for run in runs]
        numeric = [v for v in values if isinstance(v, int)]
        if len(numeric) == len(runs):
            aggregated[key] = int(statistics.median(numeric))
        else:
            aggregated[key] = first.get(key)

    return aggregated


def load_report(path: Path) -> dict | None:
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def compute_metric_delta(current: dict, previous: dict | None) -> dict:
    if not previous:
        return {"status": "no_previous_report"}

    previous_metrics = previous.get("metrics")
    if not isinstance(previous_metrics, dict):
        return {"status": "invalid_previous_report"}

    deltas: dict[str, dict] = {}
    for key, value in current.items():
        if not isinstance(value, int):
            continue
        prev_value = previous_metrics.get(key)
        if not isinstance(prev_value, int):
            continue
        deltas[key] = {
            "previous": prev_value,
            "current": value,
            "delta": value - prev_value,
        }

    return {
        "status": "ok",
        "metrics": deltas,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Check XNote perf baseline")
    parser.add_argument("--vault", default="Knowledge.vault")
    parser.add_argument("--query", default="note")
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument(
        "--baseline",
        default="perf/baseline.json",
        help="Path to baseline JSON",
    )
    parser.add_argument(
        "--report-out",
        default="perf/latest-report.json",
        help="Path to write latest perf report",
    )
    parser.add_argument(
        "--baseline-profile",
        default="default",
        help="Baseline profile key when baseline JSON is profile-based",
    )
    parser.add_argument(
        "--retries",
        type=int,
        default=1,
        help="Number of perf runs to aggregate with median",
    )
    parser.add_argument(
        "--previous-report",
        default="",
        help="Optional previous latest-report.json for delta comparison",
    )
    parser.add_argument(
        "--delta-report-out",
        default="perf/latest-delta-report.json",
        help="Path to write delta report JSON",
    )
    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    if not baseline_path.exists():
        print(f"baseline file not found: {baseline_path}")
        return 2

    baseline_raw = json.loads(baseline_path.read_text(encoding="utf-8"))
    if isinstance(baseline_raw, dict) and "profiles" in baseline_raw:
        profiles = baseline_raw.get("profiles", {})
        baseline = profiles.get(args.baseline_profile)
        if baseline is None:
            print(f"baseline profile not found: {args.baseline_profile}")
            print(f"available profiles: {', '.join(sorted(profiles.keys()))}")
            return 2
    else:
        baseline = baseline_raw
    runs = [
        run_xtask_perf(args.vault, args.query, args.iterations)
        for _ in range(max(1, args.retries))
    ]
    metrics = aggregate_metrics(runs)

    failed = []
    for key, max_allowed in baseline.items():
        value = metrics.get(key)
        if not isinstance(value, int):
            failed.append(f"missing metric: {key}")
            continue
        if value > max_allowed:
            failed.append(f"{key}: {value} > {max_allowed}")

    print("perf-metrics:")
    for key in sorted(metrics.keys()):
        print(f"  {key}: {metrics[key]}")
    print(f"perf-runs: {len(runs)}")

    report = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "vault": args.vault,
        "query": args.query,
        "iterations": args.iterations,
        "baseline_profile": args.baseline_profile,
        "retries": max(1, args.retries),
        "baseline": baseline,
        "metrics": metrics,
        "raw_runs": runs,
        "failed": failed,
        "ok": len(failed) == 0,
    }

    report_path = Path(args.report_out)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"perf-report: {report_path}")

    previous_report = None
    if args.previous_report:
        previous_report = load_report(Path(args.previous_report))
    metric_delta = compute_metric_delta(metrics, previous_report)
    delta_report = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_profile": args.baseline_profile,
        "previous_report": args.previous_report or None,
        "metric_delta": metric_delta,
    }
    delta_report_path = Path(args.delta_report_out)
    delta_report_path.parent.mkdir(parents=True, exist_ok=True)
    delta_report_path.write_text(
        json.dumps(delta_report, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"perf-delta-report: {delta_report_path}")

    if failed:
        print("perf-baseline-check: FAILED")
        for item in failed:
            print(f"  - {item}")
        return 1

    print("perf-baseline-check: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
