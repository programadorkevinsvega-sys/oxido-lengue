#!/bin/sh
set -eu

backend_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$backend_root"
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

for required_value in "${POSTGRES_CONTAINER:-}" "${POSTGRES_USER:-}" "${POSTGRES_PASSWORD:-}" "${MIGRATOR_DATABASE_URL:-}" "${MIGRATOR_DATABASE_PASSWORD:-}" "${RUNTIME_DATABASE_USER:-}" "${IDENTITY_DATABASE_USER:-}" "${IDENTITY_DATABASE_PASSWORD:-}" "${TENANCY_ACCESS_DATABASE_USER:-}" "${TENANCY_ACCESS_DATABASE_PASSWORD:-}" "${REALTIME_DATABASE_USER:-}" "${REALTIME_DATABASE_PASSWORD:-}" "${PLATFORM_CONTROL_DATABASE_USER:-}" "${PLATFORM_CONTROL_DATABASE_PASSWORD:-}" "${PLATFORM_CONTROL_DATABASE_URL:-}"; do
  if [ -z "$required_value" ]; then
    printf '%s\n' 'A required PostgreSQL setting in backend/.env is missing.' >&2
    exit 1
  fi
done

test_database="xovyra_phase2_$(openssl rand -hex 12)"
database_created=0
cleanup() {
  exit_status=$?
  trap - EXIT INT TERM
  if [ "$database_created" -eq 1 ]; then
    if ! docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "DROP DATABASE \"$test_database\" WITH (FORCE)" >/dev/null; then
      printf '%s\n' 'Could not remove the temporary XOVYRA verification database.' >&2
      exit_status=1
    fi
  fi
  exit "$exit_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "CREATE DATABASE \"$test_database\""
database_created=1
for owner_role in xovyra_identity_owner xovyra_tenancy_access_owner xovyra_commercial_billing_owner xovyra_usage_ledger_owner xovyra_provider_catalog_owner xovyra_integrations_owner xovyra_projects_owner xovyra_financial_research_owner xovyra_generation_owner xovyra_asset_storage_owner xovyra_jobs_owner xovyra_media_pipeline_owner xovyra_approvals_owner xovyra_publishing_owner xovyra_analytics_owner xovyra_notifications_owner xovyra_audit_owner xovyra_realtime_owner xovyra_platform_control_owner; do
  docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CREATE ON DATABASE \"$test_database\" TO \"$owner_role\"" >/dev/null
done
docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CONNECT ON DATABASE \"$test_database\" TO \"$RUNTIME_DATABASE_USER\"" >/dev/null
docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CONNECT ON DATABASE \"$test_database\" TO \"$IDENTITY_DATABASE_USER\"" >/dev/null
docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CONNECT ON DATABASE \"$test_database\" TO \"$TENANCY_ACCESS_DATABASE_USER\"" >/dev/null
docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CONNECT ON DATABASE \"$test_database\" TO \"$REALTIME_DATABASE_USER\"" >/dev/null
docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname postgres --set=ON_ERROR_STOP=1 --command "GRANT CONNECT ON DATABASE \"$test_database\" TO \"$PLATFORM_CONTROL_DATABASE_USER\"" >/dev/null

test_database_url="$(python3 - "$MIGRATOR_DATABASE_URL" "$test_database" <<'PY'
import sys
from urllib.parse import urlsplit, urlunsplit

connection = urlsplit(sys.argv[1])
if connection.scheme not in {"postgres", "postgresql"}:
    raise SystemExit("MIGRATOR_DATABASE_URL must use PostgreSQL URL syntax")
print(urlunsplit((connection.scheme, connection.netloc, f"/{sys.argv[2]}", connection.query, connection.fragment)))
PY
)"
MIGRATOR_DATABASE_URL="$test_database_url" cargo run --quiet -p xovyra-migrator
MIGRATOR_DATABASE_URL="$test_database_url" cargo run --quiet -p xovyra-migrator
test_platform_control_url="$(python3 - "$PLATFORM_CONTROL_DATABASE_URL" "$test_database" <<'PY'
import sys
from urllib.parse import urlsplit, urlunsplit

connection = urlsplit(sys.argv[1])
if connection.scheme not in {"postgres", "postgresql"}:
    raise SystemExit("PLATFORM_CONTROL_DATABASE_URL must use PostgreSQL URL syntax")
print(urlunsplit((connection.scheme, connection.netloc, f"/{sys.argv[2]}", connection.query, connection.fragment)))
PY
)"
test_realtime_url="$(python3 - "$REALTIME_DATABASE_URL" "$test_database" <<'PY'
import sys
from urllib.parse import urlsplit, urlunsplit

connection = urlsplit(sys.argv[1])
if connection.scheme not in {"postgres", "postgresql"}:
    raise SystemExit("REALTIME_DATABASE_URL must use PostgreSQL URL syntax")
print(urlunsplit((connection.scheme, connection.netloc, f"/{sys.argv[2]}", connection.query, connection.fragment)))
PY
)"
PLATFORM_CONTROL_DATABASE_URL="$test_platform_control_url" REALTIME_DATABASE_URL="$test_realtime_url" MIGRATOR_DATABASE_URL="$test_database_url" cargo test --quiet -p xovyra-platform-control --test grants_real -- --test-threads=1
REALTIME_DATABASE_URL="$test_realtime_url" cargo test --quiet -p xovyra-realtime --test journal_real platform_support_events_use_an_isolated_global_platform_scope -- --test-threads=1

schema_count="$(docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname "$test_database" --tuples-only --no-align --command "SELECT count(*) FROM pg_namespace WHERE nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','realtime','platform_control')")"
outbox_count="$(docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname "$test_database" --tuples-only --no-align --command "SELECT count(*) FROM pg_class relation JOIN pg_namespace schema ON schema.oid = relation.relnamespace WHERE relation.relname = 'outbox_events' AND schema.nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','platform_control')")"
migration_count="$(docker exec --env "PGPASSWORD=$POSTGRES_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$POSTGRES_USER" --dbname "$test_database" --tuples-only --no-align --command "SELECT count(*) FROM pg_class relation JOIN pg_namespace schema ON schema.oid = relation.relnamespace WHERE relation.relname = '_sqlx_migrations' AND schema.nspname IN ('identity','tenancy_access','commercial_billing','usage_ledger','provider_catalog','integrations','projects','financial_research','generation','asset_storage','jobs','media_pipeline','approvals','publishing','analytics','notifications','audit','realtime','platform_control')")"
if [ "$schema_count" -ne 19 ] || [ "$outbox_count" -ne 18 ] || [ "$migration_count" -ne 19 ]; then
  printf '%s\n' 'An empty-database migration did not install all 19 module schemas and ledgers, including all 18 module Outboxes.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$MIGRATOR_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$MIGRATOR_DATABASE_USER" --dbname "$test_database" --command 'SET ROLE xovyra_identity_owner; SELECT event_id FROM tenancy_access.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'An owner role can read a different module schema in the empty-database install.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$RUNTIME_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$RUNTIME_DATABASE_USER" --dbname "$test_database" --command 'SELECT event_id FROM identity.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'The runtime role can read a module schema in the empty-database install.' >&2
  exit 1
fi

if ! docker exec --env "PGPASSWORD=$IDENTITY_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$IDENTITY_DATABASE_USER" --dbname "$test_database" --command 'SELECT user_id FROM identity.users LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The identity runtime role cannot access its own schema after a clean migration.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$IDENTITY_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$IDENTITY_DATABASE_USER" --dbname "$test_database" --command 'SELECT event_id FROM tenancy_access.outbox_events' >/dev/null 2>&1; then
  printf '%s\n' 'The identity runtime role can access a different module schema after a clean migration.' >&2
  exit 1
fi

if ! docker exec --env "PGPASSWORD=$TENANCY_ACCESS_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$TENANCY_ACCESS_DATABASE_USER" --dbname "$test_database" --command 'SELECT workspace_id FROM tenancy_access.workspaces LIMIT 0; SELECT event_id FROM tenancy_access.outbox_events LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The tenancy runtime role cannot access its own schema after a clean migration.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$TENANCY_ACCESS_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$TENANCY_ACCESS_DATABASE_USER" --dbname "$test_database" --command 'SELECT user_id FROM identity.users LIMIT 0; SELECT event_id FROM commercial_billing.outbox_events LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The tenancy runtime role can access another module schema after a clean migration.' >&2
  exit 1
fi

if ! docker exec --env "PGPASSWORD=$PLATFORM_CONTROL_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$PLATFORM_CONTROL_DATABASE_USER" --dbname "$test_database" --command 'SELECT grant_id FROM platform_control.support_grants LIMIT 0; SELECT event_id FROM platform_control.outbox_events LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The platform-control runtime role cannot access its own schema after a clean migration.' >&2
  exit 1
fi

if docker exec --env "PGPASSWORD=$PLATFORM_CONTROL_DATABASE_PASSWORD" "$POSTGRES_CONTAINER" psql --username "$PLATFORM_CONTROL_DATABASE_USER" --dbname "$test_database" --command 'SELECT user_id FROM identity.users LIMIT 0; SELECT workspace_id FROM tenancy_access.workspaces LIMIT 0' >/dev/null 2>&1; then
  printf '%s\n' 'The platform-control runtime role can access another module schema after a clean migration.' >&2
  exit 1
fi

printf '%s\n' 'Empty PostgreSQL database installation, repeat migration and schema isolation passed.'
