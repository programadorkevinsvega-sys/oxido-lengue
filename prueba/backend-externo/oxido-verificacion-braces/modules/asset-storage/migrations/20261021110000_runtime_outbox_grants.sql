GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA asset_storage
    TO xovyra_asset_storage_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA asset_storage
    TO xovyra_asset_storage_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_asset_storage_owner IN SCHEMA asset_storage
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO xovyra_asset_storage_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE xovyra_asset_storage_owner IN SCHEMA asset_storage
    GRANT USAGE, SELECT ON SEQUENCES TO xovyra_asset_storage_runtime;
