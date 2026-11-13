GRANT USAGE ON SCHEMA commercial_billing TO xovyra_commercial_billing_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA commercial_billing
    TO xovyra_commercial_billing_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA commercial_billing
    TO xovyra_commercial_billing_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_commercial_billing_owner IN SCHEMA commercial_billing
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_commercial_billing_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_commercial_billing_owner IN SCHEMA commercial_billing
    GRANT USAGE, SELECT ON SEQUENCES TO xovyra_commercial_billing_runtime;
