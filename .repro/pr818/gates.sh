#!/bin/sh
# gates.sh: runs inside dk.sh at /w. The CI jobs of .github/workflows/ci.yml, one GATE line each, then the count lines.
cargo fmt --all --check >/tmp/fmt.out 2>&1; echo "GATE fmt exit=$?"; grep -v wrap_comments /tmp/fmt.out | head -40
timeout 3000 cargo clippy --workspace --all-targets -- -D warnings >/tmp/clippy.out 2>&1; echo "GATE clippy exit=$?"; grep -E '^(error|warning)' /tmp/clippy.out | head -20; tail -1 /tmp/clippy.out
RUSTDOCFLAGS="-D warnings" timeout 1800 cargo doc -p pallas-network2 --no-deps >/tmp/doc.out 2>&1; echo "GATE doc-network2 exit=$?"; grep -E '^(error|warning)' /tmp/doc.out | head; tail -1 /tmp/doc.out
timeout 3000 cargo doc --workspace --no-deps >/tmp/docw.out 2>&1; echo "GATE doc-workspace exit=$?"; grep -E '^(error|warning)' /tmp/docw.out | head; tail -1 /tmp/docw.out
timeout 3000 cargo +1.88 check --workspace --all-targets >/tmp/msrv.out 2>&1; echo "GATE msrv exit=$?"; grep -E '^error' /tmp/msrv.out | head; tail -1 /tmp/msrv.out
timeout 3000 cargo test -p pallas-network2 >/tmp/test.out 2>&1; echo "GATE test exit=$?"; grep -E '^test result|FAILED|panicked' /tmp/test.out
timeout 3000 cargo test -p pallas-network -p pallas-network2 --features blueprint >/tmp/bp.out 2>&1; echo "GATE blueprint exit=$?"; grep -E '^test result|FAILED|panicked' /tmp/bp.out
echo "GATES DONE"
