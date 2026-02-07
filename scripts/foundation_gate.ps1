$ErrorActionPreference = 'Stop'

Write-Host '==> foundation-gate: cargo test -p xnote-core'
cargo test -p xnote-core

Write-Host '==> foundation-gate: cargo check -p xnote-ui'
cargo check -p xnote-ui

Write-Host '==> foundation-gate: cargo test -p xnote-ui --no-run'
cargo test -p xnote-ui --no-run

Write-Host '==> foundation-gate: cargo check -p xtask'
cargo check -p xtask

Write-Host '==> foundation-gate: perf baseline default'
python scripts/check_perf_baseline.py --vault Knowledge.vault --query note --iterations 10 --retries 3 --baseline-profile default --report-out perf/latest-report.json --previous-report perf/latest-report.json --delta-report-out perf/latest-delta-report.json

Write-Host '==> foundation-gate: perf baseline windows_ci'
python scripts/check_perf_baseline.py --vault Knowledge.vault --query note --iterations 10 --retries 3 --baseline-profile windows_ci --report-out perf/latest-report-windows-ci.json --previous-report perf/latest-report-windows-ci.json --delta-report-out perf/latest-delta-report-windows-ci.json

Write-Host 'foundation-gate: OK'
