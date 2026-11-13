#!/bin/sh
set -eu

backend_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$backend_root"
sh ./scripts/verify-proto-contract.sh
if [ ! -f .env ]; then
  printf '%s\n' 'backend/.env is required.' >&2
  exit 1
fi

set -a
. ./.env
set +a

POSTGRES_CONTAINER="$(docker compose ps -q postgres)"
if [ -z "$POSTGRES_CONTAINER" ]; then
  printf '%s\n' 'The PostgreSQL Compose service is not running; start it with docker compose up -d postgres.' >&2
  exit 1
fi
export POSTGRES_CONTAINER

for required_value in "$POSTGRES_CONTAINER" "$POSTGRES_USER" "$POSTGRES_PASSWORD" "$POSTGRES_DB" "$MIGRATOR_DATABASE_USER" "$MIGRATOR_DATABASE_PASSWORD" "$RUNTIME_DATABASE_USER" "$RUNTIME_DATABASE_PASSWORD" "$IDENTITY_DATABASE_USER" "$IDENTITY_DATABASE_PASSWORD" "$IDENTITY_DATABASE_URL" "$TENANCY_ACCESS_DATABASE_USER" "$TENANCY_ACCESS_DATABASE_PASSWORD" "$TENANCY_ACCESS_DATABASE_URL" "$REDIS_CONTAINER" "$REDIS_PASSWORD" "$REDIS_URL" "$NATS_URL" "$NATS_USERNAME" "$NATS_PASSWORD" "$MODULE_RUNTIME_BIND" "$MODULE_RUNTIME_ENDPOINT" "$BFF_BIND"; do
  if [ -z "$required_value" ]; then
    printf '%s\n' 'A required setting in backend/.env is missing.' >&2
    exit 1
  fi
done

runtime_log="$(mktemp)"
bff_log="$(mktemp)"
response_file="$(mktemp)"
header_file="$(mktemp)"
runtime_process=
bff_process=
redis_key_created=0
redis_key="xovyra:verification:$(openssl rand -hex 16)"
redis_value="$(openssl rand -hex 16)"

cleanup() {
  trap - EXIT INT TERM
  if [ -n "$bff_process" ]; then
    kill "$bff_process" 2>/dev/null || true
    wait "$bff_process" 2>/dev/null || true
  fi
  if [ -n "$runtime_process" ]; then
    kill "$runtime_process" 2>/dev/null || true
    wait "$runtime_process" 2>/dev/null || true
  fi
  if [ "$redis_key_created" -eq 1 ]; then
    docker exec --env "REDISCLI_AUTH=$REDIS_PASSWORD" "$REDIS_CONTAINER" redis-cli --no-auth-warning DEL "$redis_key" >/dev/null 2>&1 || true
  fi
  rm -f "$runtime_log" "$bff_log" "$response_file" "$header_file"
}
trap cleanup EXIT INT TERM

sh ./scripts/bootstrap-existing-postgres.sh
sh ./scripts/verify-empty-database.sh
cargo run --quiet -p xovyra-migrator
cargo run --quiet -p xovyra-migrator

schema_count="$(docker exec --env "PGPASSWORD=$MIGRATOR_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$MIGRATOR_DATABASE_USER" --dbname "$POSTGRES_DB" --tuples-only --no-align --command "SELECT count(*) FROM pg_namespace WHERE nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','realtime','platform_control')")"
outbox_count="$(docker exec --env "PGPASSWORD=$MIGRATOR_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$MIGRATOR_DATABASE_USER" --dbname "$POSTGRES_DB" --tuples-only --no-align --command "SELECT count(*) FROM pg_class relation JOIN pg_namespace schema ON schema.oid = relation.relnamespace WHERE relation.relname = 'outbox_events' AND schema.nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','realtime','platform_control')")"
migration_table_count="$(docker exec --env "PGPASSWORD=$MIGRATOR_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$MIGRATOR_DATABASE_USER" --dbname "$POSTGRES_DB" --tuples-only --no-align --command "SELECT count(*) FROM pg_class relation JOIN pg_namespace schema ON schema.oid = relation.relnamespace WHERE relation.relname = '_sqlx_migrations' AND schema.nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','realtime','platform_control')")"
if [ "$schema_count" -ne 19 ] || [ "$outbox_count" -ne 18 ] || [ "$migration_table_count" -ne 19 ]; then
  printf '%s\n' 'Expected all 19 module schemas and migration ledgers plus 18 module Outbox tables.' >&2
  exit 1
fi

if ! docker exec --env "PGPASSWORD=$IDENTITY_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$IDENTITY_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SELECT user_id FROM identity.users LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The identity runtime role cannot access its own module schema.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$IDENTITY_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$IDENTITY_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SELECT event_id FROM tenancy_access.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'The identity runtime role can access a different module schema.' >&2
  exit 1
fi

if ! docker exec --env "PGPASSWORD=$TENANCY_ACCESS_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$TENANCY_ACCESS_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SELECT event_id FROM tenancy_access.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'The tenancy runtime role cannot access its own module schema.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$TENANCY_ACCESS_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$TENANCY_ACCESS_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SELECT user_id FROM identity.users LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The tenancy runtime role can access the identity module schema.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$MIGRATOR_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$MIGRATOR_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SET ROLE xovyra_identity_owner; SELECT event_id FROM tenancy_access.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'A module owner can read a different module schema.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$RUNTIME_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$RUNTIME_DATABASE_USER" --dbname "$POSTGRES_DB" --command 'SELECT event_id FROM identity.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'The runtime role can read a module-owned schema.' >&2
  exit 1
fi

redis_set_result="$(docker exec --env "REDISCLI_AUTH=$REDIS_PASSWORD" "$REDIS_CONTAINER" redis-cli --no-auth-warning SET "$redis_key" "$redis_value" NX EX 60)"
if [ "$redis_set_result" != "OK" ]; then
  printf '%s\n' 'Redis could not create the unique temporary verification key.' >&2
  exit 1
fi
redis_key_created=1
cached_value="$(docker exec --env "REDISCLI_AUTH=$REDIS_PASSWORD" "$REDIS_CONTAINER" redis-cli --no-auth-warning GET "$redis_key")"
if [ "$cached_value" != "$redis_value" ]; then
  printf '%s\n' 'Redis did not return the value written under the temporary verification key.' >&2
  exit 1
fi
docker exec --env "REDISCLI_AUTH=$REDIS_PASSWORD" "$REDIS_CONTAINER" redis-cli --no-auth-warning DEL "$redis_key" >/dev/null
redis_key_created=0

cargo test --quiet -p xovyra-module-runtime --test jetstream_real
cargo test --quiet -p xovyra-platform --test outbox_real
cargo build --quiet -p xovyra-module-runtime -p xovyra-bff
./target/debug/xovyra-module-runtime >"$runtime_log" 2>&1 &
runtime_process=$!
attempt=0
while ! grep -q 'Starting module runtime' "$runtime_log"; do
  if ! kill -0 "$runtime_process" 2>/dev/null; then
    cat "$runtime_log" >&2
    exit 1
  fi
  attempt=$((attempt + 1))
  if [ "$attempt" -gt 30 ]; then
    cat "$runtime_log" >&2
    exit 1
  fi
  sleep 1
done

./target/debug/xovyra-bff >"$bff_log" 2>&1 &
bff_process=$!
correlation_id="$(python3 -c 'import uuid; print(uuid.uuid4())')"
trace_id="$(openssl rand -hex 16)"
trace_span_id="$(openssl rand -hex 8)"
traceparent="00-$trace_id-$trace_span_id-01"
attempt=0
http_status=
while [ "$http_status" != 200 ]; do
  if ! kill -0 "$bff_process" 2>/dev/null; then
    cat "$bff_log" >&2
    exit 1
  fi
  http_status="$(curl --silent --output "$response_file" --dump-header "$header_file" --write-out '%{http_code}' --header "x-correlation-id: $correlation_id" --header "traceparent: $traceparent" "http://$BFF_BIND/api/v1/system/health" || true)"
  attempt=$((attempt + 1))
  if [ "$attempt" -gt 30 ]; then
    cat "$runtime_log" "$bff_log" >&2
    exit 1
  fi
  sleep 1
done

python3 - "$response_file" "$header_file" "$correlation_id" "$trace_id" <<'PY'
import json
import sys
from pathlib import Path

response = json.loads(Path(sys.argv[1]).read_text())
headers = Path(sys.argv[2]).read_text().lower()
correlation_id = sys.argv[3]
trace_id = sys.argv[4]
if response.get("status") != "ready":
    raise SystemExit("REST readiness did not report the configured real dependencies ready")
if response.get("correlation_id") != correlation_id:
    raise SystemExit("REST response did not preserve the correlation ID")
if f"x-correlation-id: {correlation_id}" not in headers:
    raise SystemExit("REST response header did not preserve the correlation ID")
traceparent_lines = [line.split(":", 1)[1].strip() for line in headers.splitlines() if line.startswith("traceparent:")]
if not traceparent_lines:
    raise SystemExit("REST response did not return a W3C trace context")
trace_parts = traceparent_lines[-1].split("-")
if len(trace_parts) != 4 or trace_parts[1] != trace_id:
    raise SystemExit("REST response did not preserve the incoming W3C trace ID")
PY

curl --fail --silent --show-error "http://$BFF_BIND/openapi.json" | python3 -c 'import json, sys; document=json.load(sys.stdin); paths=document.get("paths", {}); required={"/health/live", "/api/v1/system/health"}; missing=required.difference(paths); raise SystemExit(f"OpenAPI is missing mounted routes: {sorted(missing)}") if missing else SystemExit(0)'
kill -TERM "$runtime_process"
wait "$runtime_process"
runtime_process=
if ! grep -q 'SIGTERM' "$runtime_log" || ! grep -q 'Graceful shutdown requested' "$runtime_log"; then
  cat "$runtime_log" >&2
  printf '%s\n' 'Module runtime did not record its graceful SIGTERM shutdown.' >&2
  exit 1
fi
degraded_status="$(curl --silent --output "$response_file" --write-out '%{http_code}' "http://$BFF_BIND/api/v1/system/health")"
if [ "$degraded_status" != 503 ]; then
  printf '%s\n' 'BFF readiness did not degrade when the module runtime became unavailable.' >&2
  exit 1
fi
live_status="$(curl --silent --output "$response_file" --write-out '%{http_code}' "http://$BFF_BIND/health/live")"
if [ "$live_status" != 200 ]; then
  printf '%s\n' 'BFF liveness incorrectly depended on the unavailable module runtime.' >&2
  exit 1
fi
kill -INT "$bff_process"
wait "$bff_process"
bff_process=
if ! grep -q 'SIGINT' "$bff_log" || ! grep -q 'Graceful shutdown requested' "$bff_log"; then
  cat "$bff_log" >&2
  printf '%s\n' 'BFF did not record its graceful SIGINT shutdown.' >&2
  exit 1
fi
printf '%s\n' 'Backend Phase 2 infrastructure checks passed using only the configured existing services.'
