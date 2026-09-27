//! Refuse to run the server-driving integration tests against a live sccache server.
//!
//! The integration tests start and stop real sccache servers. `tests/harness`
//! strips every `SCCACHE_*` variable from the commands it runs, including
//! `SCCACHE_SERVER_PORT` and `SCCACHE_SERVER_UDS`, so those tests always use
//! the default port and cannot be isolated from a developer's own server by
//! environment variables. Run them on a machine that has one and they stop it,
//! serve its clients from a test daemon with a temporary cache, and leave it
//! for whoever compiles next to restart.
//!
//! Before the first server command of each test binary, the harnesses call
//! [`refuse_live_server`] (the harness's default port) or
//! [`refuse_live_inherited_server`] (the endpoint an inherited environment
//! selects). If a server already answers there, they panic with instructions
//! instead. Set `SCCACHE_TEST_ALLOW_LIVE=1` to override (for example in a CI
//! container where a leftover server from an earlier step is expected).

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// The port an sccache client uses when `SCCACHE_SERVER_PORT` is unset. Mirrors
/// `DEFAULT_PORT` in src/commands.rs (private to the crate);
/// tests/live_server_guard.rs fails if the two drift apart.
#[allow(dead_code)]
pub const DEFAULT_PORT: u16 = 4226;

/// Set to `1` to run the tests even though a server already listens.
pub const ALLOW_VAR: &str = "SCCACHE_TEST_ALLOW_LIVE";

const HOW_TO_ISOLATE: &str = "Run them in a VM, a container with its own network namespace, or \
     CI; a separate user account on the same host still shares the loopback port.";

/// Where an sccache client connects, resolved like `get_addr` in src/commands.rs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Endpoint {
    Tcp(u16),
    Unix(PathBuf),
    /// A Linux abstract socket (`\x00name`), which this guard cannot probe portably.
    UnixAbstract(String),
}

/// The endpoint a command that inherits this process's environment will use:
/// `SCCACHE_SERVER_UDS` first (on Unix), then `SCCACHE_SERVER_PORT`, then the
/// default port.
#[allow(dead_code)]
pub fn inherited_endpoint() -> Endpoint {
    #[cfg(unix)]
    if let Ok(uds) = std::env::var("SCCACHE_SERVER_UDS") {
        if let Some(name) = uds.strip_prefix("\\x00") {
            return Endpoint::UnixAbstract(name.to_owned());
        }
        return Endpoint::Unix(PathBuf::from(uds));
    }
    Endpoint::Tcp(inherited_port())
}

/// The port a command that inherits this process's environment will use, when
/// it uses TCP.
#[allow(dead_code)]
pub fn inherited_port() -> u16 {
    std::env::var("SCCACHE_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// `Err` with an explanation when something listens on `port` and `allow` is false.
#[allow(dead_code)]
pub fn check(port: u16, allow: bool) -> Result<(), String> {
    check_endpoint(&Endpoint::Tcp(port), allow)
}

/// `Err` with an explanation when a server answers at `endpoint` (or when it
/// cannot be probed) and `allow` is false.
pub fn check_endpoint(endpoint: &Endpoint, allow: bool) -> Result<(), String> {
    if allow {
        return Ok(());
    }
    let refuse = |what: String| {
        Err(format!(
            "{what}. These tests stop and start sccache servers there, so running them would \
             stop it (tests/harness also strips SCCACHE_SERVER_PORT and SCCACHE_SERVER_UDS, so an \
             environment variable cannot move them elsewhere). {HOW_TO_ISOLATE} Set \
             {ALLOW_VAR}=1 if stopping that server is fine."
        ))
    };
    match endpoint {
        Endpoint::Tcp(port) => {
            let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, *port));
            match TcpStream::connect_timeout(&addr, Duration::from_millis(250)) {
                Ok(_) => refuse(format!("a server is already listening on 127.0.0.1:{port}")),
                Err(_) => Ok(()),
            }
        }
        Endpoint::Unix(path) => {
            #[cfg(unix)]
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return refuse(format!(
                    "a server is already listening on the socket {}",
                    path.display()
                ));
            }
            Ok(())
        }
        Endpoint::UnixAbstract(name) => refuse(format!(
            "SCCACHE_SERVER_UDS names the abstract socket \\x00{name}, which this guard cannot probe"
        )),
    }
}

fn refuse_once(endpoint: Endpoint) {
    static CHECKED: Mutex<Vec<Endpoint>> = Mutex::new(Vec::new());
    let mut checked = CHECKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if checked.contains(&endpoint) {
        return;
    }
    let allow = std::env::var(ALLOW_VAR).is_ok_and(|v| v == "1");
    if let Err(message) = check_endpoint(&endpoint, allow) {
        panic!("{message}");
    }
    checked.push(endpoint);
}

/// Panic unless nothing listens on `port` or `SCCACHE_TEST_ALLOW_LIVE=1` is set.
///
/// Checked once per endpoint per test binary: after the first check passes, the
/// tests' own servers are expected to come and go there, so a later listener
/// cannot be told apart from them. A server that some other program starts
/// while the tests run is therefore not detected; that is why the README says
/// to run these tests in a sandbox rather than rely on this.
#[allow(dead_code)]
pub fn refuse_live_server(port: u16) {
    refuse_once(Endpoint::Tcp(port));
}

/// [`refuse_live_server`] for commands that inherit this process's environment,
/// at the endpoint that environment selects (see [`inherited_endpoint`]).
#[allow(dead_code)]
pub fn refuse_live_inherited_server() {
    refuse_once(inherited_endpoint());
}
