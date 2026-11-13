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

for database_user in "$POSTGRES_USER" "$MIGRATOR_DATABASE_USER" "$RUNTIME_DATABASE_USER" "$IDENTITY_DATABASE_USER" "$TENANCY_ACCESS_DATABASE_USER" "$COMMERCIAL_BILLING_DATABASE_USER" "$ASSET_STORAGE_DATABASE_USER" "$JOBS_DATABASE_USER" "$NOTIFICATIONS_DATABASE_USER" "$REALTIME_DATABASE_USER"; do
  if [ "$PLATFORM_CONTROL_DATABASE_USER" = "$database_user" ]; then
    printf '%s\n' 'Platform-control PostgreSQL runtime user must be distinct from every other database role.' >&2
    exit 1
  fi
done

for required_value in "${POSTGRES_CONTAINER:-}" "${POSTGRES_USER:-}" "${POSTGRES_PASSWORD:-}" "${POSTGRES_DB:-}" "${MIGRATOR_DATABASE_USER:-}" "${MIGRATOR_DATABASE_PASSWORD:-}" "${RUNTIME_DATABASE_USER:-}" "${RUNTIME_DATABASE_PASSWORD:-}" "${IDENTITY_DATABASE_USER:-}" "${IDENTITY_DATABASE_PASSWORD:-}" "${TENANCY_ACCESS_DATABASE_USER:-}" "${TENANCY_ACCESS_DATABASE_PASSWORD:-}" "${COMMERCIAL_BILLING_DATABASE_USER:-}" "${COMMERCIAL_BILLING_DATABASE_PASSWORD:-}" "${ASSET_STORAGE_DATABASE_USER:-}" "${ASSET_STORAGE_DATABASE_PASSWORD:-}" "${JOBS_DATABASE_USER:-}" "${JOBS_DATABASE_PASSWORD:-}" "${NOTIFICATIONS_DATABASE_USER:-}" "${NOTIFICATIONS_DATABASE_PASSWORD:-}" "${REALTIME_DATABASE_USER:-}" "${REALTIME_DATABASE_PASSWORD:-}" "${PLATFORM_CONTROL_DATABASE_USER:-}" "${PLATFORM_CONTROL_DATABASE_PASSWORD:-}"; do
  if [ -z "$required_value" ]; then
    printf '%s\n' 'A required PostgreSQL setting in backend/.env is missing.' >&2
    exit 1
  fi
done

  if [ "$POSTGRES_USER" = "$MIGRATOR_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$RUNTIME_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$IDENTITY_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$TENANCY_ACCESS_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$COMMERCIAL_BILLING_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$POSTGRES_USER" = "$JOBS_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$RUNTIME_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$IDENTITY_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$TENANCY_ACCESS_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$COMMERCIAL_BILLING_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$MIGRATOR_DATABASE_USER" = "$JOBS_DATABASE_USER" ] || [ "$RUNTIME_DATABASE_USER" = "$IDENTITY_DATABASE_USER" ] || [ "$RUNTIME_DATABASE_USER" = "$TENANCY_ACCESS_DATABASE_USER" ] || [ "$RUNTIME_DATABASE_USER" = "$COMMERCIAL_BILLING_DATABASE_USER" ] || [ "$RUNTIME_DATABASE_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$RUNTIME_DATABASE_USER" = "$JOBS_DATABASE_USER" ] || [ "$IDENTITY_DATABASE_USER" = "$TENANCY_ACCESS_DATABASE_USER" ] || [ "$IDENTITY_DATABASE_USER" = "$COMMERCIAL_BILLING_DATABASE_USER" ] || [ "$IDENTITY_DATABASE_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$IDENTITY_DATABASE_USER" = "$JOBS_DATABASE_USER" ] || [ "$TENANCY_ACCESS_DATABASE_USER" = "$COMMERCIAL_BILLING_DATABASE_USER" ] || [ "$TENANCY_ACCESS_DATABASE_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$TENANCY_ACCESS_DATABASE_USER" = "$JOBS_DATABASE_USER" ] || [ "$COMMERCIAL_BILLING_DATABASE_USER" = "$ASSET_STORAGE_DATABASE_USER" ] || [ "$COMMERCIAL_BILLING_DATABASE_USER" = "$JOBS_DATABASE_USER" ] || [ "$ASSET_STORAGE_DATABASE_USER" = "$JOBS_DATABASE_USER" ]; then
  printf '%s\n' 'PostgreSQL administrator, migrator and module runtime users must be distinct.' >&2
  exit 1
fi

docker exec --interactive \
  --env "PGPASSWORD=$POSTGRES_PASSWORD" \
  --env "MIGRATOR_DATABASE_USER=$MIGRATOR_DATABASE_USER" \
  --env "MIGRATOR_DATABASE_PASSWORD=$MIGRATOR_DATABASE_PASSWORD" \
  --env "RUNTIME_DATABASE_USER=$RUNTIME_DATABASE_USER" \
  --env "RUNTIME_DATABASE_PASSWORD=$RUNTIME_DATABASE_PASSWORD" \
  --env "IDENTITY_DATABASE_USER=$IDENTITY_DATABASE_USER" \
  --env "IDENTITY_DATABASE_PASSWORD=$IDENTITY_DATABASE_PASSWORD" \
  --env "TENANCY_ACCESS_DATABASE_USER=$TENANCY_ACCESS_DATABASE_USER" \
  --env "TENANCY_ACCESS_DATABASE_PASSWORD=$TENANCY_ACCESS_DATABASE_PASSWORD" \
  --env "COMMERCIAL_BILLING_DATABASE_USER=$COMMERCIAL_BILLING_DATABASE_USER" \
  --env "COMMERCIAL_BILLING_DATABASE_PASSWORD=$COMMERCIAL_BILLING_DATABASE_PASSWORD" \
  --env "ASSET_STORAGE_DATABASE_USER=$ASSET_STORAGE_DATABASE_USER" \
  --env "ASSET_STORAGE_DATABASE_PASSWORD=$ASSET_STORAGE_DATABASE_PASSWORD" \
  --env "JOBS_DATABASE_USER=$JOBS_DATABASE_USER" \
  --env "JOBS_DATABASE_PASSWORD=$JOBS_DATABASE_PASSWORD" \
  --env "NOTIFICATIONS_DATABASE_USER=$NOTIFICATIONS_DATABASE_USER" \
  --env "NOTIFICATIONS_DATABASE_PASSWORD=$NOTIFICATIONS_DATABASE_PASSWORD" \
  --env "REALTIME_DATABASE_USER=$REALTIME_DATABASE_USER" \
  --env "REALTIME_DATABASE_PASSWORD=$REALTIME_DATABASE_PASSWORD" \
  --env "PLATFORM_CONTROL_DATABASE_USER=$PLATFORM_CONTROL_DATABASE_USER" \
  --env "PLATFORM_CONTROL_DATABASE_PASSWORD=$PLATFORM_CONTROL_DATABASE_PASSWORD" \
  "$POSTGRES_CONTAINER" \
  psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" --set=ON_ERROR_STOP=1 <<'SQL'
\getenv migrator_user MIGRATOR_DATABASE_USER
\getenv migrator_password MIGRATOR_DATABASE_PASSWORD
\getenv runtime_user RUNTIME_DATABASE_USER
\getenv runtime_password RUNTIME_DATABASE_PASSWORD
\getenv identity_user IDENTITY_DATABASE_USER
\getenv identity_password IDENTITY_DATABASE_PASSWORD
\getenv tenancy_user TENANCY_ACCESS_DATABASE_USER
\getenv tenancy_password TENANCY_ACCESS_DATABASE_PASSWORD
\getenv commercial_billing_user COMMERCIAL_BILLING_DATABASE_USER
\getenv commercial_billing_password COMMERCIAL_BILLING_DATABASE_PASSWORD
\getenv asset_storage_user ASSET_STORAGE_DATABASE_USER
\getenv asset_storage_password ASSET_STORAGE_DATABASE_PASSWORD
\getenv jobs_user JOBS_DATABASE_USER
\getenv jobs_password JOBS_DATABASE_PASSWORD
\getenv notifications_user NOTIFICATIONS_DATABASE_USER
\getenv notifications_password NOTIFICATIONS_DATABASE_PASSWORD
\getenv realtime_user REALTIME_DATABASE_USER
\getenv realtime_password REALTIME_DATABASE_PASSWORD
\getenv platform_control_user PLATFORM_CONTROL_DATABASE_USER
\getenv platform_control_password PLATFORM_CONTROL_DATABASE_PASSWORD

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_roles
        WHERE rolname IN (
            'xovyra_identity_owner', 'xovyra_tenancy_access_owner',
            'xovyra_commercial_billing_owner', 'xovyra_usage_ledger_owner',
            'xovyra_provider_catalog_owner', 'xovyra_integrations_owner',
            'xovyra_projects_owner', 'xovyra_financial_research_owner',
            'xovyra_generation_owner', 'xovyra_asset_storage_owner',
            'xovyra_jobs_owner', 'xovyra_media_pipeline_owner',
            'xovyra_approvals_owner', 'xovyra_publishing_owner',
            'xovyra_analytics_owner', 'xovyra_notifications_owner',
            'xovyra_audit_owner', 'xovyra_realtime_owner',
            'xovyra_platform_control_owner'
        ) AND rolcanlogin
    ) THEN
        RAISE EXCEPTION 'A bounded-context owner role unexpectedly permits login';
    END IF;
END
$$;

SELECT format('CREATE ROLE %I NOLOGIN', owner_role)
FROM unnest(ARRAY[
    'xovyra_identity_owner', 'xovyra_tenancy_access_owner',
    'xovyra_commercial_billing_owner', 'xovyra_usage_ledger_owner',
    'xovyra_provider_catalog_owner', 'xovyra_integrations_owner',
    'xovyra_projects_owner', 'xovyra_financial_research_owner',
    'xovyra_generation_owner', 'xovyra_asset_storage_owner',
    'xovyra_jobs_owner', 'xovyra_media_pipeline_owner',
    'xovyra_approvals_owner', 'xovyra_publishing_owner',
    'xovyra_analytics_owner', 'xovyra_notifications_owner',
    'xovyra_audit_owner', 'xovyra_realtime_owner',
    'xovyra_platform_control_owner'
]) AS owners(owner_role)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = owner_role)
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'migrator_user', :'migrator_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'migrator_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'migrator_user', :'migrator_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'runtime_user', :'runtime_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'runtime_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'runtime_user', :'runtime_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'identity_user', :'identity_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'identity_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'identity_user', :'identity_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'tenancy_user', :'tenancy_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'tenancy_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'tenancy_user', :'tenancy_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'commercial_billing_user', :'commercial_billing_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'commercial_billing_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'commercial_billing_user', :'commercial_billing_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'asset_storage_user', :'asset_storage_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'asset_storage_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'asset_storage_user', :'asset_storage_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'jobs_user', :'jobs_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'jobs_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'jobs_user', :'jobs_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'notifications_user', :'notifications_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'notifications_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'notifications_user', :'notifications_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'realtime_user', :'realtime_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'realtime_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'realtime_user', :'realtime_password')
\gexec

SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'platform_control_user', :'platform_control_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'platform_control_user')
\gexec
SELECT format('ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD %L', :'platform_control_user', :'platform_control_password')
\gexec

SELECT format('GRANT CREATE ON DATABASE %I TO %I', current_database(), owner_role)
FROM unnest(ARRAY[
    'xovyra_identity_owner', 'xovyra_tenancy_access_owner',
    'xovyra_commercial_billing_owner', 'xovyra_usage_ledger_owner',
    'xovyra_provider_catalog_owner', 'xovyra_integrations_owner',
    'xovyra_projects_owner', 'xovyra_financial_research_owner',
    'xovyra_generation_owner', 'xovyra_asset_storage_owner',
    'xovyra_jobs_owner', 'xovyra_media_pipeline_owner',
    'xovyra_approvals_owner', 'xovyra_publishing_owner',
    'xovyra_analytics_owner', 'xovyra_notifications_owner',
    'xovyra_audit_owner', 'xovyra_realtime_owner',
    'xovyra_platform_control_owner'
]) AS owners(owner_role)
\gexec

SELECT format('GRANT %I TO %I', owner_role, :'migrator_user')
FROM unnest(ARRAY[
    'xovyra_identity_owner', 'xovyra_tenancy_access_owner',
    'xovyra_commercial_billing_owner', 'xovyra_usage_ledger_owner',
    'xovyra_provider_catalog_owner', 'xovyra_integrations_owner',
    'xovyra_projects_owner', 'xovyra_financial_research_owner',
    'xovyra_generation_owner', 'xovyra_asset_storage_owner',
    'xovyra_jobs_owner', 'xovyra_media_pipeline_owner',
    'xovyra_approvals_owner', 'xovyra_publishing_owner',
    'xovyra_analytics_owner', 'xovyra_notifications_owner',
    'xovyra_audit_owner', 'xovyra_realtime_owner',
    'xovyra_platform_control_owner'
]) AS owners(owner_role)
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'runtime_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'identity_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'tenancy_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'commercial_billing_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'asset_storage_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'jobs_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'notifications_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'realtime_user')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'platform_control_user')
\gexec
SQL

printf '%s\n' 'PostgreSQL XOVYRA roles and least-privilege grants are configured.'
