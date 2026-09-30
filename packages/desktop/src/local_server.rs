//! The server the app talks to, started by the app itself.
//!
//! Installed builds carry the server as `akhsakov-finance-server` next to the
//! app's own executable. On start the app uses a server of the same build
//! already answering on 127.0.0.1:8080 (a second window, or
//! `akhsakov-finance run server`), and otherwise starts that bundled one.
//! An older server left running is not used: it lacks newer endpoints.
//!
//! The started server exits with the app, however the app ends: the app
//! holds the server's stdin open, the system closes it when the app's
//! process goes away, and the server stops at end-of-file (see
//! `exit_with_parent` in main.rs).
//!
//! Under `dx serve` the CLI runs the server, so nothing is started here.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_PORT: u16 = 8080;

/// The started server's stdin, held until the app exits.
static SERVER_STDIN: Mutex<Option<ChildStdin>> = Mutex::new(None);

/// Starts the bundled server when needed and returns the URL to talk to.
pub fn start() -> String {
    let url = |port: u16| format!("http://127.0.0.1:{port}");
    if std::env::var_os("DIOXUS_CLI_ENABLED").is_some() {
        return url(DEFAULT_PORT);
    }
    let Some(exe) = bundled_server() else {
        return url(DEFAULT_PORT);
    };
    if is_our_server(DEFAULT_PORT) {
        return url(DEFAULT_PORT);
    }
    // Something else owns 8080: run ours on any free port instead.
    let port = if answers(DEFAULT_PORT) { free_port().unwrap_or(DEFAULT_PORT) } else { DEFAULT_PORT };
    match spawn(&exe, port) {
        Ok(()) => url(port),
        Err(e) => {
            eprintln!("[server] couldn't start {}: {e}", exe.display());
            url(DEFAULT_PORT)
        }
    }
}

/// `akhsakov-finance-server` next to this executable, if it was bundled.
fn bundled_server() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = if cfg!(windows) { "akhsakov-finance-server.exe" } else { "akhsakov-finance-server" };
    Some(dir.join(name)).filter(|path| path.is_file())
}

/// Where the database and the server's log live, unless `AKHSAKOV_DB` says
/// otherwise: ~/.local/share/akhsakov-finance on Linux,
/// ~/Library/Application Support/akhsakov-finance on macOS and
/// %LOCALAPPDATA%\akhsakov-finance on Windows. The installers use the same.
fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share")))
    };
    Some(base?.join("akhsakov-finance"))
}

fn spawn(exe: &std::path::Path, port: u16) -> std::io::Result<()> {
    let dir = data_dir().ok_or_else(|| std::io::Error::other("no home folder"))?;
    // The server serves static files from here and won't start without it.
    let public = dir.join("public");
    std::fs::create_dir_all(&public)?;
    let log_path = dir.join("server.log");
    let log = std::fs::File::create(&log_path)?;

    let mut command = Command::new(exe);
    command
        .current_dir(&dir)
        .env("IP", "127.0.0.1")
        .env("PORT", port.to_string())
        .env("DIOXUS_PUBLIC_PATH", &public)
        .env("AKHSAKOV_EXIT_WITH_PARENT", "1")
        .stdin(Stdio::piped())
        .stdout(log.try_clone()?)
        .stderr(log);
    if std::env::var_os("AKHSAKOV_DB").is_none() {
        command.env("AKHSAKOV_DB", dir.join("akhsakov_finance.db"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    *SERVER_STDIN.lock().unwrap_or_else(|e| e.into_inner()) = child.stdin.take();

    // Wait for it, so the first screen doesn't open offline. The first start
    // creates the database, which takes a moment.
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if is_our_server(port) {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(std::io::Error::other(format!(
                "it stopped ({status}); see {}",
                log_path.display()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(std::io::Error::other(format!("no answer after 30 seconds; see {}", log_path.display())))
}

fn addr(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

fn answers(port: u16) -> bool {
    TcpStream::connect_timeout(&addr(port), Duration::from_millis(300)).is_ok()
}

/// Whether this build's server answers on `port`: it reports the same
/// build at /api/build. An older server (left running from before an
/// update) doesn't, and gets a server of our own beside it.
fn is_our_server(port: u16) -> bool {
    let check = || -> std::io::Result<bool> {
        let mut stream = TcpStream::connect_timeout(&addr(port), Duration::from_millis(300))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n",
            crate::build_id::ROUTE
        )?;
        let mut response = String::new();
        stream.take(64 * 1024).read_to_string(&mut response)?;
        Ok(crate::build_id::is_build(&response, crate::build_id::BUILD))
    };
    check().unwrap_or(false)
}

fn free_port() -> Option<u16> {
    TcpListener::bind(addr(0)).ok()?.local_addr().ok().map(|a| a.port())
}

