sintaxis llaves
use std::error::Error;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::consumer::pull::Config;
use futures_util::StreamExt;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_platform::outbox::{JetStreamEventPublisher, OutboxRelay};
use xovyra_shared_kernel::BoundedContext;
use xovyra_tenancy_access::teams::{
    invitations, members, models::TeamError, ownership, rate_limit, roles, tasks,
    token::InvitationTokenSigner,
};
use xovyra_tenancy_access::workspaces::{self, WorkspaceError};

const TENANCY_ACCESS: BoundedContext = BoundedContext {
    name: "tenancy_access",
    schema: "tenancy_access",
};

#[tokio::test]
async fn tenant_workspace_isolation_idempotency_and_outbox_work_with_real_services()
-> Result<(), Box<dyn Error + Send + Sync>> {
    load_local_environment()?;
    let database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
    let database = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;
    let cleanup_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let cleanup_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&cleanup_database_url)
        .await?;
    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();
    let session_a = Uuid::new_v4();
    let session_b = Uuid::new_v4();
    let idempotency_key_a = Uuid::new_v4().to_string();
    let idempotency_key_b = Uuid::new_v4().to_string();
    let verification_database = database.clone();
    let verification = tokio::spawn(async move {
        verify_workspace_flows(
            &verification_database,
            user_a,
            user_b,
            session_a,
            session_b,
            &idempotency_key_a,
            &idempotency_key_b,
        )
        .await
    })
    .await;
    let cleanup = cleanup_workspace_rows(&cleanup_database, &[user_a, user_b]).await;
    cleanup_database.close().await;
    database.close().await;
    match verification {
        Ok(result) => result?,
        Err(join_error) => {
            return Err(Box::new(join_error) as Box<dyn Error + Send + Sync>);
        }
    }
    cleanup?;
    Ok(())
}

async fn verify_workspace_flows(
    database: &sqlx::PgPool,
    user_a: Uuid,
    user_b: Uuid,
    session_a: Uuid,
    session_b: Uuid,
    idempotency_key_a: &str,
    idempotency_key_b: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let email_a = format!("owner-{user_a}@integration.xovyra.invalid");
    let email_b = format!("owner-{user_b}@integration.xovyra.invalid");
    let workspace_name = format!("Integration owner workspace {user_a}");
    let concurrent_a = workspaces::create_workspace(
        database,
        workspaces::WorkspaceCreation {
            user_id: user_a,
            session_id: session_a,
            name: &workspace_name,
            workspace_type: "personal",
            idempotency_key: idempotency_key_a,
            correlation_id: "workspace-integration-a",
            owner_email: &email_a,
        },
    );
    let concurrent_b = workspaces::create_workspace(
        database,
        workspaces::WorkspaceCreation {
            user_id: user_a,
            session_id: session_a,
            name: &workspace_name,
            workspace_type: "personal",
            idempotency_key: idempotency_key_a,
            correlation_id: "workspace-integration-a",
            owner_email: &email_a,
        },
    );
    let (result_a, result_b) = tokio::join!(concurrent_a, concurrent_b);
    let workspace_a = result_a?;
    let replayed_a = result_b?;
    assert_eq!(workspace_a.workspace_id, replayed_a.workspace_id);
    let platform_directory =
        workspaces::list_platform_workspaces(database, &workspace_name, "", 200).await?;
    assert!(
        platform_directory
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_a.workspace_id)
    );
    let platform_members =
        workspaces::list_platform_workspace_members(database, workspace_a.workspace_id, "", 200)
            .await?;
    assert!(
        platform_members
            .members
            .iter()
            .any(|member| member.user_id == user_a && member.status == "active")
    );

    assert!(matches!(
        workspaces::create_workspace(
            database,
            workspaces::WorkspaceCreation {
                user_id: user_a,
                session_id: session_a,
                name: "Different request body",
                workspace_type: "personal",
                idempotency_key: idempotency_key_a,
                correlation_id: "workspace-integration-conflict",
                owner_email: &email_a,
            },
        )
        .await,
        Err(WorkspaceError::IdempotencyConflict)
    ));

    let workspace_b = workspaces::create_workspace(
        database,
        workspaces::WorkspaceCreation {
            user_id: user_b,
            session_id: session_b,
            name: "Integration second tenant",
            workspace_type: "team",
            idempotency_key: idempotency_key_b,
            correlation_id: "workspace-integration-b",
            owner_email: &email_b,
        },
    )
    .await?;
    assert_ne!(workspace_a.workspace_id, workspace_b.workspace_id);
    let settings_roles: Vec<String> = sqlx::query_scalar("SELECT role_key FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND system_role = TRUE AND permissions @> ARRAY['workspace.settings.manage']::text[] ORDER BY role_key")
        .bind(workspace_b.workspace_id)
        .fetch_all(database)
        .await?;
    assert_eq!(settings_roles, vec!["admin", "owner"]);
    let editor_can_manage_settings: bool = sqlx::query_scalar("SELECT permissions @> ARRAY['workspace.settings.manage']::text[] FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = 'editor'")
        .bind(workspace_b.workspace_id)
        .fetch_one(database)
        .await?;
    assert!(!editor_can_manage_settings);
    let editor_permissions: Vec<String> = sqlx::query_scalar(
        "SELECT permissions FROM tenancy_access.workspace_roles WHERE workspace_id = $1 AND role_key = 'editor'",
    )
    .bind(workspace_b.workspace_id)
    .fetch_one(database)
    .await?;
    assert!(editor_permissions.contains(&"tasks.update_assigned_status".to_owned()));
    assert!(!editor_permissions.contains(&"tasks.manage".to_owned()));
    verify_team_flows(
        database,
        workspace_b.workspace_id,
        workspace_b.row_version,
        user_a,
        user_b,
        &email_a,
    )
    .await?;

    let owner_workspaces = workspaces::list_workspaces_page(database, user_a, None, false).await?;
    assert_eq!(owner_workspaces.workspaces.len(), 2);
    assert!(
        owner_workspaces
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_a.workspace_id)
    );
    assert!(
        owner_workspaces
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_b.workspace_id)
    );
    let other_workspaces = workspaces::list_workspaces_page(database, user_b, None, false).await?;
    assert!(other_workspaces.workspaces.is_empty());

    let active_a = workspaces::get_active_workspace(database, user_a, session_a)
        .await?
        .ok_or("creation did not set active workspace")?;
    assert_eq!(active_a.workspace_id, workspace_a.workspace_id);
    assert_eq!(workspace_a.locale, "es-CO");
    assert_eq!(workspace_a.timezone, "America/Bogota");
    assert!(
        workspaces::get_active_workspace(database, user_b, session_a)
            .await?
            .is_none()
    );
    assert!(matches!(
        workspaces::set_active_workspace(database, user_b, session_b, workspace_a.workspace_id)
            .await,
        Err(WorkspaceError::NotFound)
    ));

    assert!(matches!(
        workspaces::get_workspace(database, user_b, workspace_a.workspace_id).await,
        Err(WorkspaceError::NotFound)
    ));
    let changed = workspaces::update_workspace(
        database,
        user_a,
        workspace_a.workspace_id,
        workspaces::WorkspaceUpdate {
            expected_row_version: workspace_a.row_version,
            name: Some("Workspace renamed"),
            locale: Some("en-US"),
            timezone: Some("Asia/Tokyo"),
            correlation_id: "workspace-integration-update",
        },
    )
    .await?;
    assert_eq!(changed.name, "Workspace renamed");
    assert_eq!(changed.row_version, workspace_a.row_version + 1);
    assert_eq!(changed.locale, "en-US");
    assert_eq!(changed.timezone, "Asia/Tokyo");
    assert!(matches!(
        workspaces::update_workspace(
            database,
            user_a,
            workspace_a.workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: changed.row_version,
                name: None,
                locale: Some("en-US-u-ca-gregory"),
                timezone: None,
                correlation_id: "workspace-integration-invalid-locale",
            },
        )
        .await,
        Err(WorkspaceError::InvalidInput)
    ));
    assert!(matches!(
        workspaces::update_workspace(
            database,
            user_b,
            workspace_a.workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: changed.row_version,
                name: None,
                locale: Some("fr-FR"),
                timezone: None,
                correlation_id: "workspace-integration-cross-tenant-settings",
            },
        )
        .await,
        Err(WorkspaceError::NotFound)
    ));
    assert!(matches!(
        workspaces::update_workspace(
            database,
            user_a,
            workspace_a.workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: workspace_a.row_version,
                name: Some("Stale name"),
                locale: None,
                timezone: None,
                correlation_id: "workspace-integration-stale",
            },
        )
        .await,
        Err(WorkspaceError::VersionConflict)
    ));
    assert!(matches!(
        workspaces::update_workspace(
            database,
            user_a,
            workspace_a.workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: changed.row_version,
                name: None,
                locale: None,
                timezone: Some("Not/A/Timezone"),
                correlation_id: "workspace-integration-timezone",
            },
        )
        .await,
        Err(WorkspaceError::InvalidInput)
    ));

    let deletion_key = Uuid::new_v4().to_string();
    let deletion = workspaces::request_workspace_deletion(
        database,
        user_a,
        workspace_a.workspace_id,
        &deletion_key,
        &changed.name,
        "scheduled",
        "workspace-integration-delete",
    )
    .await?;
    assert_eq!(deletion.request_status, "pending");
    assert_eq!(deletion.status, "deletion_pending");
    let replayed_deletion = workspaces::request_workspace_deletion(
        database,
        user_a,
        workspace_a.workspace_id,
        &deletion_key,
        &changed.name,
        "scheduled",
        "workspace-integration-delete-retry",
    )
    .await?;
    assert_eq!(replayed_deletion.request_id, deletion.request_id);
    assert!(matches!(
        workspaces::request_workspace_deletion(
            database,
            user_a,
            workspace_a.workspace_id,
            &deletion_key,
            "Different name",
            "scheduled",
            "workspace-integration-delete-conflict",
        )
        .await,
        Err(WorkspaceError::IdempotencyConflict)
    ));
    assert!(
        workspaces::get_active_workspace(database, user_a, session_a)
            .await?
            .is_none()
    );
    let active_after_delete =
        workspaces::list_workspaces_page(database, user_a, None, false).await?;
    assert_eq!(active_after_delete.workspaces.len(), 1);
    assert_eq!(
        active_after_delete.workspaces[0].workspace_id,
        workspace_b.workspace_id
    );
    let including_pending = workspaces::list_workspaces_page(database, user_a, None, true).await?;
    assert!(including_pending.workspaces.iter().any(|workspace| {
        workspace.workspace_id == workspace_a.workspace_id && workspace.status == "deletion_pending"
    }));
    assert!(matches!(
        workspaces::set_active_workspace(database, user_a, session_a, workspace_a.workspace_id)
            .await,
        Err(WorkspaceError::NotFound)
    ));
    let restored = workspaces::restore_workspace(
        database,
        user_a,
        workspace_a.workspace_id,
        "workspace-integration-restore",
    )
    .await?;
    assert_eq!(restored.status, "active");
    assert_eq!(restored.row_version, deletion.row_version + 1);

    let purge_key = Uuid::new_v4().to_string();
    let purge = workspaces::request_workspace_deletion(
        database,
        user_a,
        workspace_a.workspace_id,
        &purge_key,
        &restored.name,
        "scheduled",
        "workspace-integration-purge",
    )
    .await?;
    sqlx::query("UPDATE tenancy_access.workspace_deletion_requests SET requested_at = now() - INTERVAL '31 days', effective_at = now() - INTERVAL '1 second' WHERE request_id = $1")
        .bind(purge.request_id)
        .execute(database)
        .await?;
    sqlx::query("UPDATE tenancy_access.workspaces SET deletion_effective_at = now() - INTERVAL '1 second' WHERE workspace_id = $1")
        .bind(workspace_a.workspace_id)
        .execute(database)
        .await?;
    assert_eq!(
        workspaces::purge_due_workspaces(database, Some(purge.request_id)).await?,
        1
    );
    assert!(
        workspaces::get_workspace(database, user_a, workspace_a.workspace_id)
            .await
            .is_err()
    );

    sqlx::query("UPDATE tenancy_access.workspace_memberships SET status = 'revoked' WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_a.workspace_id)
        .bind(user_a)
        .execute(database)
        .await?;
    assert!(
        workspaces::get_active_workspace(database, user_a, session_a)
            .await?
            .is_none()
    );
    assert!(matches!(
        workspaces::set_active_workspace(database, user_a, session_a, workspace_a.workspace_id)
            .await,
        Err(WorkspaceError::NotFound)
    ));
    sqlx::query(
        "UPDATE tenancy_access.workspaces SET status = 'suspended' WHERE workspace_id = $1",
    )
    .bind(workspace_b.workspace_id)
    .execute(database)
    .await?;
    assert!(matches!(
        workspaces::set_active_workspace(database, user_b, session_b, workspace_b.workspace_id)
            .await,
        Err(WorkspaceError::NotFound)
    ));

    verify_workspace_outbox(database, workspace_a.workspace_id).await?;
    Ok(())
}

async fn verify_team_flows(
    database: &sqlx::PgPool,
    workspace_id: Uuid,
    workspace_version: i64,
    user_a: Uuid,
    user_b: Uuid,
    email_a: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut expected_workspace_version = workspace_version;
    let redis_url = std::env::var("REDIS_URL")?;
    let client = redis::Client::open(redis_url)?;
    let mut cache = client.get_connection_manager().await?;
    let signer = InvitationTokenSigner::new(std::env::var("TENANCY_INVITATION_TOKEN_SECRET")?)?;
    let invitation_key = Uuid::new_v4().to_string();
    let invitation = invitations::create(
        database,
        &mut cache,
        &signer,
        invitations::InvitationCreation {
            workspace_id,
            actor: user_b,
            email: email_a,
            invited_user_id: None,
            role_key: "editor",
            idempotency_key: &invitation_key,
            correlation_id: "team-invitation-created",
        },
    )
    .await?;
    assert_eq!(invitation.status, "pending");
    let delivery = invitations::delivery(database, &signer, invitation.invitation_id).await?;
    assert_eq!(delivery.email, email_a);
    assert!(delivery.expires_at_unix > chrono_free_now());
    invitations::mark_delivery(
        database,
        invitation.invitation_id,
        true,
        "team-invitation-delivered",
    )
    .await?;
    invitations::mark_delivery(
        database,
        invitation.invitation_id,
        true,
        "team-invitation-delivery-replayed",
    )
    .await?;
    let delivered_attempts: i32 = sqlx::query_scalar(
        "SELECT delivery_attempts FROM tenancy_access.workspace_invitations WHERE invitation_id=$1",
    )
    .bind(invitation.invitation_id)
    .fetch_one(database)
    .await?;
    assert_eq!(delivered_attempts, 1);
    let delivery_events: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM tenancy_access.outbox_events WHERE event_type='tenancy_access.invitation.delivery_updated' AND payload->>'invitation_id'=$1",
    )
    .bind(invitation.invitation_id.to_string())
    .fetch_all(database)
    .await?;
    assert_eq!(delivery_events.len(), 1);
    assert_eq!(delivery_events[0]["workspace_id"], workspace_id.to_string());
    assert_eq!(
        delivery_events[0]["correlation_id"],
        "team-invitation-delivered"
    );
    assert!(delivery_events[0]["aggregate_version"].as_i64().is_some());
    let failed_email = format!("failed-{}@integration.xovyra.invalid", Uuid::new_v4());
    let failed_invitation_key = Uuid::new_v4().to_string();
    let failed_invitation = invitations::create(
        database,
        &mut cache,
        &signer,
        invitations::InvitationCreation {
            workspace_id,
            actor: user_b,
            email: &failed_email,
            invited_user_id: None,
            role_key: "editor",
            idempotency_key: &failed_invitation_key,
            correlation_id: "team-invitation-failed-created",
        },
    )
    .await?;
    for attempt in 0..5 {
        invitations::mark_delivery(
            database,
            failed_invitation.invitation_id,
            false,
            &format!("team-invitation-failed-attempt-{attempt}"),
        )
        .await?;
    }
    invitations::mark_delivery(
        database,
        failed_invitation.invitation_id,
        false,
        "team-invitation-failed-replay",
    )
    .await?;
    let failed_delivery: (String, i32) = sqlx::query_as(
        "SELECT delivery_status, delivery_attempts FROM tenancy_access.workspace_invitations WHERE invitation_id=$1",
    )
    .bind(failed_invitation.invitation_id)
    .fetch_one(database)
    .await?;
    assert_eq!(failed_delivery, ("failed".to_owned(), 5));
    assert!(matches!(
        invitations::delivery(database, &signer, failed_invitation.invitation_id).await,
        Err(TeamError::DeliveryUnavailable)
    ));
    assert!(matches!(
        invitations::accept(
            database,
            &signer,
            user_a,
            &format!("wrong-{email_a}"),
            &delivery.token,
            "team-wrong-email"
        )
        .await,
        Err(TeamError::InvalidInvitation)
    ));
    let accepted = invitations::accept(
        database,
        &signer,
        user_a,
        email_a,
        &delivery.token,
        "team-invitation-accepted",
    )
    .await?;
    assert_eq!(accepted.role_key, "editor");
    assert_eq!(
        invitations::accept(
            database,
            &signer,
            user_a,
            email_a,
            &delivery.token,
            "team-invitation-replay"
        )
        .await?
        .user_id,
        user_a
    );
    let (listed, _) = members::list_members(database, workspace_id, user_b, None, 100).await?;
    assert_eq!(listed.len(), 2);
    let owner_preferences = workspaces::update_workspace(
        database,
        user_b,
        workspace_id,
        workspaces::WorkspaceUpdate {
            expected_row_version: expected_workspace_version,
            name: None,
            locale: Some("fr-FR"),
            timezone: Some("Europe/Paris"),
            correlation_id: "team-workspace-preferences-owner",
        },
    )
    .await?;
    expected_workspace_version = owner_preferences.row_version;
    assert_eq!(owner_preferences.role, "owner");
    let visible_preferences = workspaces::get_workspace(database, user_a, workspace_id).await?;
    assert_eq!(visible_preferences.locale, "fr-FR");
    assert_eq!(visible_preferences.timezone, "Europe/Paris");
    assert!(matches!(
        workspaces::update_workspace(
            database,
            user_a,
            workspace_id,
            workspaces::WorkspaceUpdate {
                expected_row_version: expected_workspace_version,
                name: None,
                locale: Some("en-GB"),
                timezone: Some("Europe/London"),
                correlation_id: "team-workspace-preferences-editor-denied",
            },
        )
        .await,
        Err(WorkspaceError::Forbidden)
    ));
    let custom_role = roles::create_role(
        database,
        workspace_id,
        user_b,
        "analyst",
        "Analista",
        &["tasks.read".to_owned(), "comments.read".to_owned()],
        "team-role-create",
    )
    .await?;
    assert!(!custom_role.system_role);
    let forbidden_task_key = Uuid::new_v4().to_string();
    assert!(matches!(
        tasks::create_task(
            database,
            tasks::TaskCreation {
                workspace_id,
                actor: user_a,
                title: "No puede crear como editor",
                description: "",
                assignee: Some(user_a),
                idempotency_key: &forbidden_task_key,
                correlation_id: "team-editor-task-create-forbidden",
            }
        )
        .await,
        Err(TeamError::Forbidden)
    ));
    let task_key = Uuid::new_v4().to_string();
    let created_task = tasks::create_task(
        database,
        tasks::TaskCreation {
            workspace_id,
            actor: user_b,
            title: "Revisar proyecto",
            description: "Tarea compartida",
            assignee: Some(user_a),
            idempotency_key: &task_key,
            correlation_id: "team-task-create",
        },
    )
    .await?;
    assert_eq!(
        tasks::create_task(
            database,
            tasks::TaskCreation {
                workspace_id,
                actor: user_b,
                title: "Revisar proyecto",
                description: "Tarea compartida",
                assignee: Some(user_a),
                idempotency_key: &task_key,
                correlation_id: "team-task-replay",
            }
        )
        .await?
        .task_id,
        created_task.task_id
    );
    let progress = tasks::update_task(
        database,
        tasks::TaskUpdate {
            workspace_id,
            actor: user_a,
            task_id: created_task.task_id,
            title: &created_task.title,
            description: &created_task.description,
            assignee: created_task.assignee_user_id,
            status: "in_progress",
            expected_version: created_task.row_version,
            correlation_id: "team-assignee-progress",
        },
    )
    .await?;
    assert_eq!(progress.status, "in_progress");
    assert!(matches!(
        tasks::update_task(
            database,
            tasks::TaskUpdate {
                workspace_id,
                actor: user_a,
                task_id: progress.task_id,
                title: "Cambiar título sin permiso",
                description: &progress.description,
                assignee: progress.assignee_user_id,
                status: "in_progress",
                expected_version: progress.row_version,
                correlation_id: "team-assignee-content-forbidden",
            }
        )
        .await,
        Err(TeamError::Forbidden)
    ));
    let comment_key = Uuid::new_v4().to_string();
    let comment = tasks::create_comment(
        database,
        tasks::CommentCreation {
            workspace_id,
            actor: user_a,
            task_id: created_task.task_id,
            body: "Estoy revisando el material",
            mentions: &[user_b],
            idempotency_key: &comment_key,
            correlation_id: "team-comment-create",
        },
    )
    .await?;
    assert_eq!(
        tasks::create_comment(
            database,
            tasks::CommentCreation {
                workspace_id,
                actor: user_a,
                task_id: created_task.task_id,
                body: "Estoy revisando el material",
                mentions: &[user_b],
                idempotency_key: &comment_key,
                correlation_id: "team-comment-replay",
            }
        )
        .await?
        .comment_id,
        comment.comment_id
    );
    assert_eq!(
        tasks::list_comments(database, workspace_id, user_b, created_task.task_id)
            .await?
            .len(),
        1
    );
    assert!(matches!(
        tasks::delete_comment(
            database,
            workspace_id,
            user_b,
            created_task.task_id,
            comment.comment_id,
            "team-comment-delete-forbidden"
        )
        .await,
        Err(TeamError::Forbidden)
    ));
    let deleted_comment = tasks::delete_comment(
        database,
        workspace_id,
        user_a,
        created_task.task_id,
        comment.comment_id,
        "team-comment-delete",
    )
    .await?;
    assert!(deleted_comment.deleted);
    assert_eq!(deleted_comment.body, "[Comentario eliminado]");
    assert!(
        tasks::delete_comment(
            database,
            workspace_id,
            user_a,
            created_task.task_id,
            comment.comment_id,
            "team-comment-delete-replay",
        )
        .await?
        .deleted
    );
    let changed = members::change_member(
        database,
        members::MemberChange {
            workspace_id,
            actor_user_id: user_b,
            member_user_id: user_a,
            role_key: "viewer",
            status: "active",
            expected_row_version: accepted.row_version,
            correlation_id: "team-member-demote",
        },
    )
    .await?;
    assert!(matches!(
        tasks::create_task(
            database,
            tasks::TaskCreation {
                workspace_id,
                actor: user_a,
                title: "Sin permiso",
                description: "",
                assignee: None,
                idempotency_key: &Uuid::new_v4().to_string(),
                correlation_id: "team-task-forbidden",
            }
        )
        .await,
        Err(TeamError::Forbidden)
    ));
    let reactivated = members::change_member(
        database,
        members::MemberChange {
            workspace_id,
            actor_user_id: user_b,
            member_user_id: user_a,
            role_key: "editor",
            status: "active",
            expected_row_version: changed.row_version,
            correlation_id: "team-member-restore-role",
        },
    )
    .await?;
    assert_eq!(reactivated.role_key, "editor");
    let administrator = members::change_member(
        database,
        members::MemberChange {
            workspace_id,
            actor_user_id: user_b,
            member_user_id: user_a,
            role_key: "admin",
            status: "active",
            expected_row_version: reactivated.row_version,
            correlation_id: "team-member-promote-admin",
        },
    )
    .await?;
    assert_eq!(administrator.role_key, "admin");
    let admin_preferences = workspaces::update_workspace(
        database,
        user_a,
        workspace_id,
        workspaces::WorkspaceUpdate {
            expected_row_version: expected_workspace_version,
            name: None,
            locale: Some("de-DE"),
            timezone: Some("Europe/Berlin"),
            correlation_id: "team-workspace-preferences-admin",
        },
    )
    .await?;
    expected_workspace_version = admin_preferences.row_version;
    assert_eq!(admin_preferences.role, "admin");
    let owner_visible_preferences =
        workspaces::get_workspace(database, user_b, workspace_id).await?;
    assert_eq!(owner_visible_preferences.locale, "de-DE");
    assert_eq!(owner_visible_preferences.timezone, "Europe/Berlin");
    let next_version = ownership::transfer(
        database,
        workspace_id,
        user_b,
        user_a,
        "TRANSFER Integration second tenant",
        expected_workspace_version,
        "team-ownership-transfer",
    )
    .await?;
    assert_eq!(next_version, expected_workspace_version + 1);
    assert!(matches!(
        ownership::transfer(
            database,
            workspace_id,
            user_b,
            user_a,
            "TRANSFER Integration second tenant",
            expected_workspace_version,
            "team-stale-transfer"
        )
        .await,
        Err(TeamError::NotFound) | Err(TeamError::Forbidden) | Err(TeamError::VersionConflict)
    ));
    members::revoke_member(database, workspace_id, user_a, user_b, "team-member-revoke").await?;
    assert!(matches!(
        roles::list_roles(database, workspace_id, user_b).await,
        Err(TeamError::NotFound)
    ));
    let outsider = Uuid::new_v4();
    assert!(matches!(
        tasks::list_tasks(database, workspace_id, outsider).await,
        Err(TeamError::NotFound)
    ));
    let limiter_workspace = Uuid::new_v4();
    let limiter_actor = Uuid::new_v4();
    let limiter_email_prefix = Uuid::new_v4();
    for index in 0..5 {
        rate_limit::enforce_invitation_rate(
            &mut cache,
            &signer,
            limiter_workspace,
            limiter_actor,
            &format!("{limiter_email_prefix}-limiter-{index}@integration.xovyra.invalid"),
        )
        .await?;
    }
    assert!(matches!(
        rate_limit::enforce_invitation_rate(
            &mut cache,
            &signer,
            limiter_workspace,
            limiter_actor,
            &format!("{limiter_email_prefix}-limiter-final@integration.xovyra.invalid")
        )
        .await,
        Err(TeamError::RateLimited { .. })
    ));
    let payloads:Vec<serde_json::Value>=sqlx::query_scalar("SELECT payload FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=$1 AND event_type LIKE 'tenancy_access.%' AND event_type NOT LIKE 'tenancy_access.workspace.%'").bind(workspace_id.to_string()).fetch_all(database).await?;
    assert!(payloads.len() >= 7);
    for payload in payloads {
        assert!(payload.get("email").is_none());
        assert!(payload.get("recipient").is_none());
        assert!(payload.get("token").is_none());
    }
    verify_invitation_event_delivery(database, workspace_id).await?;
    Ok(())
}

fn chrono_free_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

async fn verify_invitation_event_delivery(
    database: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let nats_url = std::env::var("NATS_URL")?;
    let username = std::env::var("NATS_USERNAME")?;
    let password = std::env::var("NATS_PASSWORD")?;
    let messaging = async_nats::ConnectOptions::with_user_and_password(username, password)
        .connect(nats_url)
        .await?;
    let jetstream = jetstream::new(messaging.clone());
    let stream = jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: "TENANCY_ACCESS_EVENTS".to_owned(),
            subjects: vec!["tenancy_access.>".to_owned()],
            ..Default::default()
        })
        .await?;
    let relay = OutboxRelay::new(TENANCY_ACCESS)?;
    let publisher = JetStreamEventPublisher::new(jetstream);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let pending:i64=sqlx::query_scalar("SELECT count(*)::bigint FROM tenancy_access.outbox_events WHERE event_type='tenancy_access.invitation.requested' AND payload->>'workspace_id'=$1 AND published_at IS NULL").bind(workspace_id.to_string()).fetch_one(database).await?;
        if pending == 0 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("invitation outbox did not publish before timeout".into());
        }
        let mut connection = database.acquire().await?;
        relay.relay_one(&mut connection, &publisher).await?;
    }
    let consumer = stream
        .create_consumer(Config {
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
            filter_subject: "tenancy_access.invitation.requested.v1".to_owned(),
            ..Default::default()
        })
        .await?;
    let mut messages = consumer.messages().await?;
    while tokio::time::Instant::now() < deadline {
        let item = tokio::time::timeout(Duration::from_millis(500), messages.next()).await;
        if let Ok(Some(Ok(message))) = item {
            let event: serde_json::Value = serde_json::from_slice(&message.payload)?;
            message.ack().await?;
            if event
                .pointer("/payload/workspace_id")
                .and_then(serde_json::Value::as_str)
                == Some(workspace_id.to_string().as_str())
            {
                assert!(event["payload"].get("email").is_none());
                assert!(event["payload"].get("token").is_none());
                return Ok(());
            }
        }
    }
    Err("JetStream did not deliver invitation event before timeout".into())
}

async fn verify_workspace_outbox(
    database: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let event_types = [
        "tenancy_access.workspace.created",
        "tenancy_access.workspace.updated",
        "tenancy_access.workspace.deletion_requested",
        "tenancy_access.workspace.restored",
        "tenancy_access.workspace.purged",
    ];
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM tenancy_access.outbox_events WHERE event_type = 'tenancy_access.workspace.created' AND payload->>'workspace_id' = $1",
    )
    .bind(workspace_id.to_string())
    .fetch_one(database)
    .await?;
    assert_eq!(event_count, 1);
    let payload: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM tenancy_access.outbox_events WHERE event_type = 'tenancy_access.workspace.created' AND payload->>'workspace_id' = $1",
    )
    .bind(workspace_id.to_string())
    .fetch_one(database)
    .await?;
    assert_eq!(payload["workspace_id"], workspace_id.to_string());
    assert!(payload.get("email").is_none());
    assert!(payload.get("token").is_none());
    assert!(payload.get("secret").is_none());

    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await?;
    let jetstream = jetstream::new(messaging.clone());
    let stream = jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: "TENANCY_ACCESS_EVENTS".to_owned(),
            subjects: vec!["tenancy_access.>".to_owned()],
            ..Default::default()
        })
        .await?;
    let publisher = JetStreamEventPublisher::new(jetstream);
    let relay = OutboxRelay::new(TENANCY_ACCESS)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let unpublished_count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM tenancy_access.outbox_events WHERE event_type = ANY($2) AND payload->>'workspace_id' = $1 AND published_at IS NULL")
            .bind(workspace_id.to_string())
            .bind(&event_types[..])
            .fetch_one(database)
            .await?;
        if unpublished_count == 0 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("workspace Outbox event was not published before timeout".into());
        }
        let mut connection = database.acquire().await?;
        relay.relay_one(&mut connection, &publisher).await?;
        drop(connection);
    }

    for expected_type in event_types {
        let consumer = stream
            .create_consumer(Config {
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                filter_subject: format!("{expected_type}.v1"),
                ..Default::default()
            })
            .await?;
        let mut messages = consumer.messages().await?;
        let mut delivered = false;
        while tokio::time::Instant::now() < deadline {
            let message = tokio::time::timeout(Duration::from_millis(500), messages.next()).await;
            let Ok(Some(Ok(message))) = message else {
                continue;
            };
            let envelope: serde_json::Value = serde_json::from_slice(&message.payload)?;
            let is_target = envelope["payload"]["workspace_id"] == workspace_id.to_string();
            message.ack().await?;
            if is_target {
                assert!(envelope["payload"].get("email").is_none());
                assert!(envelope["payload"].get("secret").is_none());
                delivered = true;
            }
            if delivered {
                break;
            }
        }
        if !delivered {
            return Err(format!("JetStream did not deliver {expected_type}.v1").into());
        }
    }
    messaging.flush().await?;
    Ok(())
}

async fn cleanup_workspace_rows(
    database: &sqlx::PgPool,
    users: &[Uuid],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tenancy_access.workspace_purge_holds WHERE workspace_id IN (SELECT workspace_id FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = ANY($1))")
        .bind(users)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id' IN (SELECT workspace_id::text FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1)) OR payload->>'workspace_id' IN (SELECT workspace_id::text FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = ANY($1))")
        .bind(users)
        .execute(database)
        .await?;
    sqlx::query("DELETE FROM tenancy_access.workspace_audit_events WHERE actor_user_id = ANY($1) OR workspace_id IN (SELECT workspace_id FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1)) OR workspace_id IN (SELECT workspace_id FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = ANY($1))")
        .bind(users)
        .execute(database)
        .await?;
    sqlx::query(
        "DELETE FROM tenancy_access.workspace_deletion_requests WHERE requester_user_id = ANY($1)",
    )
    .bind(users)
    .execute(database)
    .await?;
    sqlx::query("DELETE FROM tenancy_access.workspaces WHERE created_by_user_id = ANY($1)")
        .bind(users)
        .execute(database)
        .await?;
    sqlx::query(
        "DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id = ANY($1)",
    )
    .bind(users)
    .execute(database)
    .await?;
    Ok(())
}

fn load_local_environment() -> Result<(), Box<dyn Error + Send + Sync>> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}
