#!/usr/bin/env bash
set -euo pipefail

backend_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
repository_root="$(cd -- "$backend_root/.." && pwd)"
cd "$backend_root"

log_file="$(mktemp "${TMPDIR:-/tmp}/xovyra-phase-4.5.XXXXXX")"
trap 'rm -f "$log_file"' EXIT

cargo fmt --all --check
cargo clippy -p xovyra-realtime -p xovyra-platform-control -p xovyra-module-runtime -p xovyra-identity -p xovyra-bff --all-targets -- -D warnings
cargo build -p xovyra-bff -p xovyra-module-runtime
cargo test -p xovyra-bff
cargo test -p xovyra-realtime --test journal_real -- --test-threads=1
sh scripts/verify-empty-database.sh

if ! cargo test -p xovyra-identity --test security_real real_account_security_and_operator_flows_are_isolated_and_publish_events -- --exact --test-threads=1 >"$log_file" 2>&1; then
  rg 'error:|FAILED|test result:' "$log_file" | tail -30 || true
  exit 1
fi

python3 - "$log_file" <<'PY'
import json
import sys

events = []
with open(sys.argv[1], encoding="utf-8") as log:
    for line in log:
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        fields = record.get("fields", {})
        event = fields.get("event")
        if event in {"bff.readiness.grpc.failed", "bff.readiness.grpc.timeout", "bff.readiness.runtime.not_ready", "bff.readiness.trace_context.mismatch", "http.request.failed"}:
            events.append((event, fields))

closures = [
    fields
    for event, fields in events
    if event == "http.request.failed" and fields.get("http_route") == "/api/v1/system/health"
]
causes = [fields for event, fields in events if event.startswith("bff.readiness.")]
if not closures or not causes:
    raise SystemExit("The real BFF acceptance did not emit both readiness cause and HTTP closure events.")
for closure in closures:
    correlated = [
        cause
        for cause in causes
        if cause.get("correlation_id") == closure.get("correlation_id")
        and cause.get("trace_id") == closure.get("trace_id")
    ]
    if not correlated:
        raise SystemExit("A readiness HTTP closure has no operation error with matching correlation_id and trace_id.")
    if closure.get("error_code") == "unclassified_http_failure":
        raise SystemExit("A readiness HTTP closure lost its operation-specific error classification.")

print(f"BFF diagnostic correlation passed: {len(closures)} HTTP error closures pair with operation failures.")
PY

cd "$repository_root/frontend/admin_plataform"
bun run test
bun run lint
bun run build

if [[ "${PHASE45_VERIFY_R2:-0}" == "1" ]]; then
  cd "$backend_root"
  cargo test -p xovyra-module-runtime --features real-r2-tests r2_storage::real_integration::direct_presigned_upload_head_download_and_delete_use_real_cloudflare_r2 -- --exact --test-threads=1
fi

printf '%s\n' 'Phase 4.5 automated backend, BFF, database, realtime, and admin frontend verification passed.'
