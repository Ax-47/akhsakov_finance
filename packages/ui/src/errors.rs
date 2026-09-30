//! Server errors in the user's words.

use crate::i18n::tr;
use dioxus::prelude::*;

/// The message to show for a failed server call. A 404 or 405 means the
/// server has no such endpoint: it's an older version than the app (for
/// example one left running from before an update).
pub fn error_message(e: ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { code: 404 | 405, .. } => tr(OUTDATED).to_string(),
        ServerFnError::ServerError { message, .. } => message,
        e => e.to_string(),
    }
}

const OUTDATED: &str = "The app's server is out of date. Close the app completely and open it again.";

#[cfg(test)]
mod tests {
    use super::*;

    fn server_error(code: u16, message: &str) -> ServerFnError {
        ServerFnError::ServerError { message: message.into(), code, details: None }
    }

    #[test]
    fn missing_endpoints_say_the_server_is_old() {
        assert_eq!(error_message(server_error(405, "HTTP 405: null")), OUTDATED);
        assert_eq!(error_message(server_error(404, "HTTP 404: ")), OUTDATED);
        assert_eq!(error_message(server_error(400, "That image is too large.")), "That image is too large.");
    }
}
