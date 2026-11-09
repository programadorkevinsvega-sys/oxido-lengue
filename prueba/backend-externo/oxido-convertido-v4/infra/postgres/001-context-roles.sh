#!/bin/sh
set -eu

psql \
  --username "$POSTGRES_USER" \
  --dbname "$POSTGRES_DB" \
  --set=ON_ERROR_STOP=1 \
  --set=runtime_user="$RUNTIME_DATABASE_USER" \
  --set=runtime_password="$RUNTIME_DATABASE_PASSWORD" \
  --set=commercial_billing_user="$COMMERCIAL_BILLING_DATABASE_USER" \
  --set=commercial_billing_password="$COMMERCIAL_BILLING_DATABASE_PASSWORD" \
  --set=platform_control_user="$PLATFORM_CONTROL_DATABASE_USER" \
  --set=platform_control_password="$PLATFORM_CONTROL_DATABASE_PASSWORD" \
  --set=notifications_user="$NOTIFICATIONS_DATABASE_USER" \
  --set=notifications_password="$NOTIFICATIONS_DATABASE_PASSWORD" \
  --set=realtime_user="$REALTIME_DATABASE_USER" \
  --set=realtime_password="$REALTIME_DATABASE_PASSWORD" <<'SQL'
CREATE ROLE xovyra_identity_owner NOLOGIN;
CREATE ROLE xovyra_tenancy_access_owner NOLOGIN;
CREATE ROLE xovyra_commercial_billing_owner NOLOGIN;
CREATE ROLE xovyra_usage_ledger_owner NOLOGIN;
CREATE ROLE xovyra_provider_catalog_owner NOLOGIN;
CREATE ROLE xovyra_integrations_owner NOLOGIN;
CREATE ROLE xovyra_projects_owner NOLOGIN;
CREATE ROLE xovyra_financial_research_owner NOLOGIN;
CREATE ROLE xovyra_generation_owner NOLOGIN;
CREATE ROLE xovyra_asset_storage_owner NOLOGIN;
CREATE ROLE xovyra_jobs_owner NOLOGIN;
CREATE ROLE xovyra_media_pipeline_owner NOLOGIN;
CREATE ROLE xovyra_approvals_owner NOLOGIN;
CREATE ROLE xovyra_publishing_owner NOLOGIN;
CREATE ROLE xovyra_analytics_owner NOLOGIN;
CREATE ROLE xovyra_notifications_owner NOLOGIN;
CREATE ROLE xovyra_audit_owner NOLOGIN;
CREATE ROLE xovyra_realtime_owner NOLOGIN;
CREATE ROLE xovyra_platform_control_owner NOLOGIN;
CREATE ROLE :"notifications_user" LOGIN PASSWORD :'notifications_password';
CREATE ROLE :"realtime_user" LOGIN PASSWORD :'realtime_password';
CREATE ROLE :"platform_control_user" LOGIN PASSWORD :'platform_control_password';

CREATE ROLE :"runtime_user" LOGIN PASSWORD :'runtime_password';
CREATE ROLE :"commercial_billing_user" LOGIN PASSWORD :'commercial_billing_password';
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'runtime_user') \gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'commercial_billing_user') \gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'platform_control_user') \gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'notifications_user') \gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'realtime_user') \gexec
SELECT format('GRANT CREATE ON DATABASE %I TO %I', current_database(), owner_role) \gexec
SELECT format('GRANT %I TO %I', owner_role, current_user)
FROM unnest(ARRAY[
  'xovyra_identity_owner',
  'xovyra_tenancy_access_owner',
  'xovyra_commercial_billing_owner',
  'xovyra_usage_ledger_owner',
  'xovyra_provider_catalog_owner',
  'xovyra_integrations_owner',
  'xovyra_projects_owner',
  'xovyra_financial_research_owner',
  'xovyra_generation_owner',
  'xovyra_asset_storage_owner',
  'xovyra_jobs_owner',
  'xovyra_media_pipeline_owner',
  'xovyra_approvals_owner',
  'xovyra_publishing_owner',
  'xovyra_analytics_owner',
  'xovyra_notifications_owner',
  'xovyra_audit_owner',
  'xovyra_realtime_owner',
  'xovyra_platform_control_owner'
]) AS owners(owner_role) \gexec
SQL
