sintaxis llaves
#[path = "account_sessions/list.ox"]
pub mod list;
#[path = "account_sessions/revoke.ox"]
pub mod revoke;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::openapi::AppState;

pub use list::{SessionListBody, SessionSummaryBody};
pub use revoke::{RevokeOthersBody, SessionRevocationBody};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me/sessions", get(list::list_sessions))
        .route(
            "/api/v1/me/sessions/{session_id}",
            delete(revoke::revoke_session),
        )
        .route(
            "/api/v1/me/sessions/revoke-others",
            post(revoke::revoke_others),
        )
}
