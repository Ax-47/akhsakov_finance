//! Which build a server is. The app only uses a server that answers
//! [`ROUTE`] with its own [`BUILD`], so an older server left running (from
//! before an update) isn't used by a newer app whose endpoints it lacks.

// The server answers ROUTE; only the app on its own (desktop without
// server) checks the answer.
#![cfg_attr(any(not(feature = "desktop"), feature = "server"), allow(dead_code))]

/// The commit this was built from (see build.rs).
pub const BUILD: &str = env!("AKHSAKOV_BUILD");
pub const ROUTE: &str = "/api/build";

/// Whether an HTTP `response` is a 200 whose body is exactly `build`.
pub fn is_build(response: &str, build: &str) -> bool {
    let status = response.lines().next().and_then(|line| line.split(' ').nth(1));
    let body = response.split_once("\r\n\r\n").map(|(_, body)| body.trim());
    status == Some("200") && body == Some(build)
}

#[cfg(test)]
mod tests {
    use super::is_build;

    #[test]
    fn only_the_same_build_counts() {
        let ok = "HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nabc1234";
        assert!(is_build(ok, "abc1234"));
        assert!(!is_build(ok, "def5678"), "another build");
        // A server from before /api/build existed.
        assert!(!is_build("HTTP/1.1 405 Method Not Allowed\r\n\r\n", "abc1234"));
        assert!(!is_build("HTTP/1.1 404 Not Found\r\n\r\nabc1234", "abc1234"));
    }
}
