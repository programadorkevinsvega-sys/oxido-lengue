#[path = "platform_auth/login.rs"]
pub mod login;
#[path = "platform_auth/permissions.rs"]
pub mod permissions;
#[path = "platform_auth/session.rs"]
pub mod session;

use axum::Router;
use axum::routing::{get, post};

use crate::openapi::AppState;

pub use login::{PlatformLoginBody, PlatformLoginResponseBody};
pub use permissions::PlatformPermissionsBody;
pub use session::{PlatformRefreshResponseBody, PlatformSessionResponseBody};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/platform/auth/login", post(login::login))
        .route("/platform/auth/session", get(session::current))
        .route("/platform/auth/refresh", post(session::refresh))
        .route("/platform/auth/logout", post(session::logout))
        .route(
            "/platform/me/permissions",
            get(permissions::get_permissions),
        )
}
