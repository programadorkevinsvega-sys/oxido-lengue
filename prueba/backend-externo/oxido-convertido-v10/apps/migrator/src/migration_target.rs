use sqlx::migrate::Migrator;
use xovyra_shared_kernel::BoundedContext;

pub struct MigrationTarget {
    pub context: BoundedContext,
    pub owner_role: &'static str,
    pub migrator: &'static Migrator,
}

pub fn migration_targets() -> [MigrationTarget; 19] {
    [
        MigrationTarget {
            context: xovyra_identity::module_public_api::bounded_context(),
            owner_role: xovyra_identity::module_public_api::OWNER_ROLE,
            migrator: &xovyra_identity::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_tenancy_access::module_public_api::bounded_context(),
            owner_role: xovyra_tenancy_access::module_public_api::OWNER_ROLE,
            migrator: &xovyra_tenancy_access::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_commercial_billing::module_public_api::bounded_context(),
            owner_role: xovyra_commercial_billing::module_public_api::OWNER_ROLE,
            migrator: &xovyra_commercial_billing::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_usage_ledger::module_public_api::bounded_context(),
            owner_role: xovyra_usage_ledger::module_public_api::OWNER_ROLE,
            migrator: &xovyra_usage_ledger::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_provider_catalog::module_public_api::bounded_context(),
            owner_role: xovyra_provider_catalog::module_public_api::OWNER_ROLE,
            migrator: &xovyra_provider_catalog::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_integrations::module_public_api::bounded_context(),
            owner_role: xovyra_integrations::module_public_api::OWNER_ROLE,
            migrator: &xovyra_integrations::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_projects::module_public_api::bounded_context(),
            owner_role: xovyra_projects::module_public_api::OWNER_ROLE,
            migrator: &xovyra_projects::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_financial_research::module_public_api::bounded_context(),
            owner_role: xovyra_financial_research::module_public_api::OWNER_ROLE,
            migrator: &xovyra_financial_research::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_generation::module_public_api::bounded_context(),
            owner_role: xovyra_generation::module_public_api::OWNER_ROLE,
            migrator: &xovyra_generation::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_asset_storage::module_public_api::bounded_context(),
            owner_role: xovyra_asset_storage::module_public_api::OWNER_ROLE,
            migrator: &xovyra_asset_storage::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_jobs::module_public_api::bounded_context(),
            owner_role: xovyra_jobs::module_public_api::OWNER_ROLE,
            migrator: &xovyra_jobs::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_media_pipeline::module_public_api::bounded_context(),
            owner_role: xovyra_media_pipeline::module_public_api::OWNER_ROLE,
            migrator: &xovyra_media_pipeline::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_approvals::module_public_api::bounded_context(),
            owner_role: xovyra_approvals::module_public_api::OWNER_ROLE,
            migrator: &xovyra_approvals::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_publishing::module_public_api::bounded_context(),
            owner_role: xovyra_publishing::module_public_api::OWNER_ROLE,
            migrator: &xovyra_publishing::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_analytics::module_public_api::bounded_context(),
            owner_role: xovyra_analytics::module_public_api::OWNER_ROLE,
            migrator: &xovyra_analytics::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_notifications::module_public_api::bounded_context(),
            owner_role: xovyra_notifications::module_public_api::OWNER_ROLE,
            migrator: &xovyra_notifications::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_realtime::module_public_api::bounded_context(),
            owner_role: xovyra_realtime::module_public_api::OWNER_ROLE,
            migrator: &xovyra_realtime::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_audit::module_public_api::bounded_context(),
            owner_role: xovyra_audit::module_public_api::OWNER_ROLE,
            migrator: &xovyra_audit::module_public_api::MIGRATOR,
        },
        MigrationTarget {
            context: xovyra_platform_control::module_public_api::bounded_context(),
            owner_role: xovyra_platform_control::module_public_api::OWNER_ROLE,
            migrator: &xovyra_platform_control::module_public_api::MIGRATOR,
        },
    ]
}
