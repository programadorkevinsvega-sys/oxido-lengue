pub mod invitations;
pub mod members;
pub mod models;
pub mod ownership;
pub mod permissions;
pub mod rate_limit;
pub mod roles;
pub mod tasks;
pub mod token;

#[macro_export]
macro_rules! append_team_event {
    ($transaction:expr, $workspace_id:expr, $actor_user_id:expr, $action:expr, $row_version:expr, $correlation_id:expr, $event_type:expr, $payload:expr $(,)?) => {
        $crate::teams::permissions::append_team_event(
            $transaction,
            $crate::teams::permissions::TeamEvent {
                workspace_id: $workspace_id,
                actor_user_id: $actor_user_id,
                action: $action,
                row_version: $row_version,
                correlation_id: $correlation_id,
                event_type: $event_type,
                payload: $payload,
            },
        )
    };
}
