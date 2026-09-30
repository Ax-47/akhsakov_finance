//! Stamps the build with its git commit (`AKHSAKOV_BUILD`), so the app only
//! uses a server from the same build (see `local_server.rs`).

use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn main() {
    let build = git(&["rev-parse", "HEAD"])
        .unwrap_or_else(|| format!("v{}", std::env::var("CARGO_PKG_VERSION").unwrap_or_default()));
    println!("cargo:rustc-env=AKHSAKOV_BUILD={build}");
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
        for file in ["HEAD", "refs/heads", "packed-refs"] {
            let path = Path::new(&dir).join(file);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
