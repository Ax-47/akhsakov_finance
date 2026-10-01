//! MCP over Streamable HTTP, stateless: each JSON-RPC message is POSTed and
//! answered with JSON. There's no server-to-client stream, so GET gets 405
//! as the transport allows.

use crate::{
    auth::controller::bearer,
    mcp::{services::Access, McpService},
};
use dioxus::server::axum::{
    body::Bytes,
    extract::{Extension, Path},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};

pub fn router(service: McpService) -> Router {
    Router::new()
        .route("/mcp", any(with_header))
        .route("/mcp/{key}", any(with_path))
        // There's no OAuth here. Otherwise the app's page fallback would
        // answer clients probing for OAuth metadata with HTML and a 200.
        .route("/.well-known/{*rest}", any(|| async { StatusCode::NOT_FOUND }))
        .layer(Extension(service))
}

async fn with_header(
    Extension(mcp): Extension<McpService>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    serve(&mcp, bearer(&headers), method, &body).await
}

async fn with_path(
    Extension(mcp): Extension<McpService>,
    Path(key): Path<String>,
    method: Method,
    body: Bytes,
) -> Response {
    serve(&mcp, Some(&key), method, &body).await
}

async fn serve(mcp: &McpService, key: Option<&str>, method: Method, body: &[u8]) -> Response {
    match mcp.authorize(key) {
        Access::Granted(access) => {
            if !mcp.allow(access.connection_id) {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, "60")],
                    "Too many requests from this connection; try again in a minute.",
                )
                    .into_response();
            }
            if method != Method::POST {
                return (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response();
            }
            let (status, reply) = match serde_json::from_slice(body) {
                Ok(message) => (StatusCode::OK, mcp.handle_scoped(message, &access).await),
                Err(_) => (StatusCode::BAD_REQUEST, Some(McpService::parse_error())),
            };
            return match reply {
                Some(reply) => (status, [(header::CONTENT_TYPE, "application/json")], reply.to_string()).into_response(),
                None => StatusCode::ACCEPTED.into_response(),
            };
        }
        Access::Denied => {
            return (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Bearer")],
                "Wrong or missing connector key",
            )
                .into_response()
        }
        Access::Error(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
